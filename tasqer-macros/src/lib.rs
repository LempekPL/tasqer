use proc_macro::TokenStream;
use quote::__private::Span;
use quote::quote;
use syn::parse::Parser;
use syn::{Ident, ItemFn, LitStr};

fn create_router(path: LitStr, func: ItemFn, method: &str) -> TokenStream {
    let vis = func.vis.clone();
    let name = func.sig.ident.clone();
    let ret = func.sig.output.clone();
    let block = func.block.clone();
    let async_sig = func
        .sig
        .asyncness
        .clone()
        .expect("function needs to be async");
    let input = func.sig.inputs.clone();
    let input_types = input.iter().map(|arg| match arg {
        syn::FnArg::Receiver(_) => panic!("methods with 'self' are not supported"),
        syn::FnArg::Typed(pat_type) => &pat_type.ty,
    });

    let ident = Ident::new(method, Span::call_site());

    quote! {
        #vis fn #name <S>() -> axum::Router<S>
        where
            S: Clone + Send + Sync + 'static,
            #( #input_types: axum::extract::FromRequestParts<S>, )*
        {
            #async_sig fn handler(#input) #ret #block
            axum::Router::new().route(#path, axum::routing:: #ident (handler))
        }
    }
    .into()
}

fn create_router_from_tokens(attr: TokenStream, item: TokenStream, method: &str) -> TokenStream {
    let func = syn::parse_macro_input!(item as ItemFn);
    let path = syn::parse_macro_input!(attr as LitStr);
    create_router(path, func, method)
}

/// Makes using methods easier
/// ```
/// #[get("/foo")]
/// async fn bar() -> &'static str {
///     "Hello, World!"
/// }
/// ```
/// turns into
/// ```
/// fn bar<S>() -> Router<S>
/// where
///     S: Clone + Send + Sync + 'static,
/// {
///     fn handler() -> &'static str {
///         "Hello, World!"
///     }
///
///     Router::new().route("/foo", get(handler))
/// }
/// ```
/// so you can use it as `Router::new().merge(bar);` or `Router::new().nest("/foo", bar());`
#[proc_macro_attribute]
pub fn get(attr: TokenStream, item: TokenStream) -> TokenStream {
    create_router_from_tokens(attr, item, "get")
}

/// Makes using methods easier
/// ```
/// #[post("/foo")]
/// async fn bar() -> &'static str {
///     "Hello, World!"
/// }
/// ```
/// turns into
/// ```
/// fn bar<S>() -> Router<S>
/// where
///     S: Clone + Send + Sync + 'static,
/// {
///     fn handler() -> &'static str {
///         "Hello, World!"
///     }
///
///     Router::new().route("/foo", post(handler))
/// }
/// ```
/// so you can use it as `Router::new().merge(bar());` or `Router::new().nest("/foo", bar());`
#[proc_macro_attribute]
pub fn post(attr: TokenStream, item: TokenStream) -> TokenStream {
    create_router_from_tokens(attr, item, "post")
}

/// Makes using methods easier
/// ```
/// #[put("/foo")]
/// async fn bar() -> &'static str {
///     "Hello, World!"
/// }
/// ```
/// turns into
/// ```
/// fn bar<S>() -> Router<S>
/// where
///     S: Clone + Send + Sync + 'static,
/// {
///     fn handler() -> &'static str {
///         "Hello, World!"
///     }
///
///     Router::new().route("/foo", put(handler))
/// }
/// ```
/// so you can use it as `Router::new().merge(bar());` or `Router::new().nest("/foo", bar());`
#[proc_macro_attribute]
pub fn put(attr: TokenStream, item: TokenStream) -> TokenStream {
    create_router_from_tokens(attr, item, "put")
}

/// Makes using methods easier
/// ```
/// #[delete("/foo")]
/// async fn bar() -> &'static str {
///     "Hello, World!"
/// }
/// ```
/// turns into
/// ```
/// fn bar<S>() -> Router<S>
/// where
///     S: Clone + Send + Sync + 'static,
/// {
///     fn handler() -> &'static str {
///         "Hello, World!"
///     }
///
///     Router::new().route("/foo", delete(handler))
/// }
/// ```
/// so you can use it as `Router::new().merge(bar());` or `Router::new().nest("/foo", bar());`
#[proc_macro_attribute]
pub fn delete(attr: TokenStream, item: TokenStream) -> TokenStream {
    create_router_from_tokens(attr, item, "delete")
}

/// Makes using methods easier
/// ```
/// #[options("/foo")]
/// async fn bar() -> &'static str {
///     "Hello, World!"
/// }
/// ```
/// turns into
/// ```
/// fn bar<S>() -> Router<S>
/// where
///     S: Clone + Send + Sync + 'static,
/// {
///     fn handler() -> &'static str {
///         "Hello, World!"
///     }
///
///     Router::new().route("/foo", options(handler))
/// }
/// ```
/// so you can use it as `Router::new().merge(bar());` or `Router::new().nest("/foo", bar());`
#[proc_macro_attribute]
pub fn options(attr: TokenStream, item: TokenStream) -> TokenStream {
    create_router_from_tokens(attr, item, "options")
}

/// Makes using methods easier
/// ```
/// #[patch("/foo")]
/// async fn bar() -> &'static str {
///     "Hello, World!"
/// }
/// ```
/// turns into
/// ```
/// fn bar<S>() -> Router<S>
/// where
///     S: Clone + Send + Sync + 'static,
/// {
///     fn handler() -> &'static str {
///         "Hello, World!"
///     }
///
///     Router::new().route("/foo", patch(handler))
/// }
/// ```
/// so you can use it as `Router::new().merge(bar());` or `Router::new().nest("/foo", bar());`
#[proc_macro_attribute]
pub fn patch(attr: TokenStream, item: TokenStream) -> TokenStream {
    create_router_from_tokens(attr, item, "patch")
}

/// Allows to easly merge multiple routers
/// ```
/// #[get("/")]
/// async fn foo() -> &'static str {
///     "Get world!"
/// }
///
/// #[post("/")]
/// async fn bar() -> &'static str {
///     "Post world!"
/// }
///
/// Router::new().nest("/baz", routes![foo(), bar()]);
/// ```
#[proc_macro]
pub fn routes(input: TokenStream) -> TokenStream {
    let parser = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
    let exprs = match parser.parse(input) {
        Ok(exprs) => exprs,
        Err(err) => return err.to_compile_error().into(),
    };
    if exprs.is_empty() {
        return quote! { axum::Router::new() }.into();
    }
    let merges = exprs.into_iter().map(|expr| {
        quote! { .merge(#expr) }
    });
    let expanded = quote! {
        axum::Router::new()
            #(#merges)*
    };
    expanded.into()
}
