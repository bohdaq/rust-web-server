//! Procedural macro attributes for [rust-web-server](https://crates.io/crates/rust-web-server).
//!
//! Import via the main crate with `features = ["macros"]`:
//!
//! ```toml
//! [dependencies]
//! rust-web-server = { version = "17", features = ["macros"] }
//! ```
//!
//! # Attributes
//!
//! | Attribute | Equivalent |
//! |-----------|------------|
//! | `#[route(GET, "/path")]` | generic; any method |
//! | `#[get("/path")]` | shorthand for GET |
//! | `#[post("/path")]` | shorthand for POST |
//! | `#[put("/path")]` | shorthand for PUT |
//! | `#[patch("/path")]` | shorthand for PATCH |
//! | `#[delete("/path")]` | shorthand for DELETE |
//!
//! All attributes add a `Route: METHOD /path` doc-comment and leave the
//! function body completely unchanged. They work with named functions used
//! as handlers in `routes!` or registered directly with `.get()`, `.post()`, etc.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Data, DeriveInput, Fields, Ident, ItemFn, LitStr, Token,
};

// ── Shared helper ─────────────────────────────────────────────────────────────

fn annotate(method: &str, path: LitStr, func: ItemFn) -> TokenStream {
    let doc = format!("Route: `{} {}`", method, path.value());
    quote! {
        #[doc = #doc]
        #func
    }
    .into()
}

// ── #[route(METHOD, "/path")] ─────────────────────────────────────────────────

struct RouteArgs {
    method: Ident,
    path: LitStr,
}

impl Parse for RouteArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let method: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let path: LitStr = input.parse()?;
        Ok(RouteArgs { method, path })
    }
}

/// Annotate a handler function with its HTTP method and path.
///
/// Adds a `Route: METHOD /path` doc-comment to the function and otherwise
/// leaves it completely unchanged. Use [`crate::routes!`] (from the main
/// crate) to register the handler with the router.
///
/// # Example
///
/// ```ignore
/// use rust_web_server::route;
/// use rust_web_server::request::Request;
/// use rust_web_server::router::PathParams;
/// use rust_web_server::server::ConnectionInfo;
/// use rust_web_server::response::Response;
/// use std::sync::Arc;
///
/// struct Db;
///
/// #[route(GET, "/users/:id")]
/// fn get_user(
///     req: &Request,
///     params: &PathParams,
///     conn: &ConnectionInfo,
///     state: &Arc<Db>,
/// ) -> Response {
///     let id = params.get("id").unwrap_or("0");
///     Response::new()
/// }
/// ```
#[proc_macro_attribute]
pub fn route(args: TokenStream, input: TokenStream) -> TokenStream {
    let RouteArgs { method, path } = parse_macro_input!(args as RouteArgs);
    let func = parse_macro_input!(input as ItemFn);
    annotate(&method.to_string(), path, func)
}

// ── #[get("/path")], #[post("/path")], … ─────────────────────────────────────

/// Shorthand for `#[route(GET, "/path")]`.
///
/// # Example
///
/// ```ignore
/// use rust_web_server::get;
/// # use rust_web_server::request::Request;
/// # use rust_web_server::router::PathParams;
/// # use rust_web_server::server::ConnectionInfo;
/// # use rust_web_server::response::Response;
///
/// #[get("/healthz")]
/// fn health(_: &Request, _: &PathParams, _: &ConnectionInfo) -> Response {
///     Response::new()
/// }
/// ```
#[proc_macro_attribute]
pub fn get(args: TokenStream, input: TokenStream) -> TokenStream {
    let path = parse_macro_input!(args as LitStr);
    let func = parse_macro_input!(input as ItemFn);
    annotate("GET", path, func)
}

/// Shorthand for `#[route(POST, "/path")]`.
#[proc_macro_attribute]
pub fn post(args: TokenStream, input: TokenStream) -> TokenStream {
    let path = parse_macro_input!(args as LitStr);
    let func = parse_macro_input!(input as ItemFn);
    annotate("POST", path, func)
}

/// Shorthand for `#[route(PUT, "/path")]`.
#[proc_macro_attribute]
pub fn put(args: TokenStream, input: TokenStream) -> TokenStream {
    let path = parse_macro_input!(args as LitStr);
    let func = parse_macro_input!(input as ItemFn);
    annotate("PUT", path, func)
}

/// Shorthand for `#[route(PATCH, "/path")]`.
#[proc_macro_attribute]
pub fn patch(args: TokenStream, input: TokenStream) -> TokenStream {
    let path = parse_macro_input!(args as LitStr);
    let func = parse_macro_input!(input as ItemFn);
    annotate("PATCH", path, func)
}

/// Shorthand for `#[route(DELETE, "/path")]`.
#[proc_macro_attribute]
pub fn delete(args: TokenStream, input: TokenStream) -> TokenStream {
    let path = parse_macro_input!(args as LitStr);
    let func = parse_macro_input!(input as ItemFn);
    annotate("DELETE", path, func)
}

// ── #[derive(FromRequest)] ────────────────────────────────────────────────────

/// Derive `FromRequest` for a named-field struct.
///
/// Each field must implement `FromRequest`. Fields are extracted in declaration
/// order; the first failure short-circuits and returns that error response.
///
/// # Example
///
/// ```ignore
/// use rust_web_server::extract::{BodyText, Query};
///
/// #[derive(rust_web_server::FromRequest)]
/// struct Payload {
///     body: BodyText,
///     params: Query,
/// }
/// ```
#[proc_macro_derive(FromRequest)]
pub fn derive_from_request(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_from_request(ast)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn impl_from_request(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let span = input.ident.span();
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            _ => {
                return Err(syn::Error::new(
                    span,
                    "#[derive(FromRequest)] only supports structs with named fields",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new(
                span,
                "#[derive(FromRequest)] can only be derived on structs",
            ))
        }
    };

    let extractions = fields.iter().map(|f| {
        let ident = f.ident.as_ref().unwrap();
        let ty = &f.ty;
        quote! {
            let #ident = <#ty as _rws::extract::FromRequest>::from_request(__req)?;
        }
    });

    let field_names = fields.iter().map(|f| f.ident.as_ref().unwrap());

    Ok(quote! {
        const _: () = {
            use ::rust_web_server as _rws;
            impl _rws::extract::FromRequest for #name {
                fn from_request(__req: &_rws::request::Request) -> ::core::result::Result<Self, _rws::response::Response> {
                    #(#extractions)*
                    ::core::result::Result::Ok(#name { #(#field_names),* })
                }
            }
        };
    })
}
