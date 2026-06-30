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
    parse_macro_input, Data, DeriveInput, Fields, Ident, ItemFn, Lit, LitInt, LitStr, Token,
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

// ── #[derive(Validate)] ───────────────────────────────────────────────────────

/// Derive `Validate` for a named-field struct.
///
/// Annotate fields with `#[validate(...)]` rules. All failures are collected
/// before returning, so the caller sees every invalid field in one response.
///
/// # Supported validators
///
/// | Syntax | Checks |
/// |--------|--------|
/// | `length(min = N)` | `field.chars().count() >= N` |
/// | `length(max = N)` | `field.chars().count() <= N` |
/// | `length(min = N, max = N)` | both bounds |
/// | `range(min = N)` | `field as f64 >= N` |
/// | `range(max = N)` | `field as f64 <= N` |
/// | `range(min = N, max = N)` | both bounds |
/// | `email` | local part, `@`, domain with `.` |
/// | `required` | `!field.is_empty()` |
/// | `url` | starts with `http://` or `https://` |
///
/// # Example
///
/// ```ignore
/// #[derive(rust_web_server::Validate)]
/// struct CreateUser {
///     #[validate(length(min = 1, max = 50))]
///     name: String,
///     #[validate(email)]
///     email: String,
///     #[validate(range(min = 0, max = 150))]
///     age: u8,
/// }
/// ```
#[proc_macro_derive(Validate, attributes(validate))]
pub fn derive_validate(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_validate(ast)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn impl_validate(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let span = input.ident.span();
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            _ => {
                return Err(syn::Error::new(
                    span,
                    "#[derive(Validate)] only supports structs with named fields",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new(
                span,
                "#[derive(Validate)] can only be derived on structs",
            ))
        }
    };

    let mut all_checks: Vec<proc_macro2::TokenStream> = Vec::new();

    for field in fields {
        let field_ident = field.ident.as_ref().unwrap();
        let field_name = field_ident.to_string();
        for attr in &field.attrs {
            if attr.path().is_ident("validate") {
                let checks = generate_field_checks(attr, field_ident, &field_name)?;
                all_checks.extend(checks);
            }
        }
    }

    Ok(quote! {
        const _: () = {
            use ::rust_web_server as _rws;
            impl _rws::validate::Validate for #name {
                fn validate(&self) -> ::core::result::Result<(), _rws::validate::ValidationErrors> {
                    let mut __errors = _rws::validate::ValidationErrors::new();
                    #(#all_checks)*
                    if __errors.is_empty() {
                        ::core::result::Result::Ok(())
                    } else {
                        ::core::result::Result::Err(__errors)
                    }
                }
            }
        };
    })
}

fn generate_field_checks(
    attr: &syn::Attribute,
    field_ident: &syn::Ident,
    field_name: &str,
) -> syn::Result<Vec<proc_macro2::TokenStream>> {
    let mut checks: Vec<proc_macro2::TokenStream> = Vec::new();

    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("email") {
            let msg = format!("{field_name} must be a valid email address");
            checks.push(quote! {
                if !_rws::validate::is_email(&self.#field_ident) {
                    __errors.add(#field_name, #msg);
                }
            });
        } else if meta.path.is_ident("required") {
            let msg = format!("{field_name} must not be empty");
            checks.push(quote! {
                if self.#field_ident.is_empty() {
                    __errors.add(#field_name, #msg);
                }
            });
        } else if meta.path.is_ident("url") {
            let msg = format!("{field_name} must be a valid URL (http:// or https://)");
            checks.push(quote! {
                if !_rws::validate::is_url(&self.#field_ident) {
                    __errors.add(#field_name, #msg);
                }
            });
        } else if meta.path.is_ident("length") {
            let mut min: Option<u64> = None;
            let mut max: Option<u64> = None;
            meta.parse_nested_meta(|inner| {
                if inner.path.is_ident("min") {
                    let lit: LitInt = inner.value()?.parse()?;
                    min = Some(lit.base10_parse()?);
                } else if inner.path.is_ident("max") {
                    let lit: LitInt = inner.value()?.parse()?;
                    max = Some(lit.base10_parse()?);
                } else {
                    return Err(inner.error("expected `min` or `max`"));
                }
                Ok(())
            })?;

            let mut len_checks: Vec<proc_macro2::TokenStream> = Vec::new();
            if let Some(n) = min {
                let msg = format!("{field_name} must be at least {n} character(s) long");
                let n_lit = proc_macro2::Literal::usize_suffixed(n as usize);
                len_checks.push(quote! {
                    if __len < #n_lit { __errors.add(#field_name, #msg); }
                });
            }
            if let Some(n) = max {
                let msg = format!("{field_name} must be at most {n} character(s) long");
                let n_lit = proc_macro2::Literal::usize_suffixed(n as usize);
                len_checks.push(quote! {
                    if __len > #n_lit { __errors.add(#field_name, #msg); }
                });
            }
            checks.push(quote! {
                {
                    let __len = self.#field_ident.chars().count();
                    #(#len_checks)*
                }
            });
        } else if meta.path.is_ident("range") {
            let mut min: Option<f64> = None;
            let mut max: Option<f64> = None;
            meta.parse_nested_meta(|inner| {
                if inner.path.is_ident("min") {
                    min = Some(lit_to_f64(&inner.value()?.parse::<Lit>()?)?);
                } else if inner.path.is_ident("max") {
                    max = Some(lit_to_f64(&inner.value()?.parse::<Lit>()?)?);
                } else {
                    return Err(inner.error("expected `min` or `max`"));
                }
                Ok(())
            })?;

            let mut rng_checks: Vec<proc_macro2::TokenStream> = Vec::new();
            if let Some(n) = min {
                let msg = format!("{field_name} must be at least {n}");
                let n_lit = proc_macro2::Literal::f64_suffixed(n);
                rng_checks.push(quote! {
                    if __val < #n_lit { __errors.add(#field_name, #msg); }
                });
            }
            if let Some(n) = max {
                let msg = format!("{field_name} must be at most {n}");
                let n_lit = proc_macro2::Literal::f64_suffixed(n);
                rng_checks.push(quote! {
                    if __val > #n_lit { __errors.add(#field_name, #msg); }
                });
            }
            checks.push(quote! {
                {
                    let __val = self.#field_ident as f64;
                    #(#rng_checks)*
                }
            });
        } else {
            return Err(meta.error(
                "unknown validator; expected: email, required, url, length, range",
            ));
        }
        Ok(())
    })?;

    Ok(checks)
}

fn lit_to_f64(lit: &Lit) -> syn::Result<f64> {
    match lit {
        Lit::Float(f) => Ok(f.base10_parse()?),
        Lit::Int(i) => Ok(i.base10_parse::<i64>()? as f64),
        _ => Err(syn::Error::new_spanned(lit, "expected a numeric literal")),
    }
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
