use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_attribute]
pub fn cache(args: TokenStream, annotated_item: TokenStream) -> TokenStream {
    // args should be empty for now
    if !args.is_empty() {
        panic!("cache takes no arguments");
    }

    let func = syn::parse_macro_input!(annotated_item as syn::ItemFn);
    let func_name = func.sig.ident;
    let func_vis = func.vis;
    let func_body = func.block;
    let func_attrs = func.attrs;

    // Generate a cache name for the function by creating a matching uppercase variable name
    // For example, `fn foo()` will generate a `static mut FOO: HashMap<_, _>` cache.
    let cache_name = syn::Ident::new(&func_name.to_string().to_uppercase(), func_name.span());

    // Determine the types and names for all the parameters of the function
    let params = &func.sig.inputs.iter().filter_map(|param| {
        match param {
            syn::FnArg::Receiver(_) => panic!("cache cannot be applied to a method (a function taking `self` as its first parameter)"),
            syn::FnArg::Typed(arg) => Some(arg),
        }
    }).collect::<Vec<_>>();

    let param_types = params
        .iter()
        .map(|param| param.ty.clone())
        .collect::<Vec<_>>();

    let param_names = params
        .iter()
        .map(|param| match *param.pat {
            syn::Pat::Ident(ref name) => name.ident.clone(),
            _ => panic!("cache cannot be applied to a function using a pattern as parameter"),
        })
        .collect::<Vec<_>>();

    // Generate the type of the keys for the cache: a tuple of the function parameters' types
    let param_types_tuple = quote! {
        (#(#param_types),*)
    };

    // Determine the return type of the function
    let return_type = match &func.sig.output {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ref ty) => quote! { #ty },
    };

    let store = quote! {
        std::thread_local! {
            static #cache_name: std::cell::UnsafeCell<
                std::collections::HashMap<
                    #param_types_tuple,
                    #return_type
                >
            > = std::cell::UnsafeCell::new(std::collections::HashMap::new());
        }
    };

    let memoized_func = quote! {
        #(#func_attrs)*
        fn #func_vis #func_name (#(#param_names: #param_types),*) -> #return_type {
            let key = (#(#param_names),*);
            if let Some(value) = #cache_name.with(|cache| {
                let cache = unsafe { &*cache.get() };
                cache.get(&key).cloned()
            }) {
                value
            } else {
                fn compute_result(#(#param_names: #param_types),*) -> #return_type #func_body;
                let result = compute_result(#(#param_names),*);
                #cache_name.with(|cache| {
                    let cache = unsafe { &mut *cache.get() };
                    cache.insert(key, result.clone())
                });
                result
            }
        }
    };

    quote! {
        #store

        #memoized_func
    }
    .into()
}
