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
//!
//! ## `#[derive(Config)]`
//!
//! Generates `fn load() -> Result<Self, String>` that reads environment variables
//! and parses them into the annotated field types. See `rust_web_server::config_binding`.

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

// ── #[derive(Config)] ─────────────────────────────────────────────────────────

/// Derive `load() -> Result<Self, String>` for a configuration struct.
///
/// Each field is bound to an environment variable. The env var name is derived
/// from the field name (uppercased) plus an optional struct-level prefix.
///
/// # Struct-level attribute
///
/// ```text
/// #[config(prefix = "APP_")]
/// ```
///
/// When set, every field's env var key is `prefix + key`.
///
/// # Field-level attribute
///
/// ```text
/// #[config(env = "PORT", default = "8080")]
/// ```
///
/// | Option | Meaning |
/// |--------|---------|
/// | `env = "KEY"` | explicit env var name (prefix is still prepended) |
/// | `default = "v"` | fallback when the env var is absent |
///
/// If `env` is omitted the field name is uppercased and used as the key.
/// Wrapping the field type in `Option<T>` makes it optional (returns `None` when absent).
///
/// # Example
///
/// ```ignore
/// #[derive(rust_web_server::Config)]
/// #[config(prefix = "APP_")]
/// struct AppConfig {
///     #[config(env = "PORT", default = "8080")]
///     port: u16,
///     #[config(env = "DATABASE_URL")]
///     database_url: String,
///     #[config(env = "DEBUG")]
///     debug: Option<bool>,
/// }
///
/// let cfg = AppConfig::load().unwrap();
/// ```
#[proc_macro_derive(Config, attributes(config))]
pub fn derive_config(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_config(ast)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn impl_config(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let span = input.ident.span();
    let name = &input.ident;

    // Parse optional struct-level prefix: #[config(prefix = "...")]
    let prefix = parse_config_prefix(&input.attrs)?;

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            _ => {
                return Err(syn::Error::new(
                    span,
                    "#[derive(Config)] only supports structs with named fields",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new(
                span,
                "#[derive(Config)] can only be derived on structs",
            ))
        }
    };

    let mut field_loads: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut field_names: Vec<&syn::Ident> = Vec::new();

    for field in fields {
        let ident = field.ident.as_ref().unwrap();
        field_names.push(ident);

        // Parse field-level #[config(env = "...", default = "...")]
        let (env_key, default) = parse_field_config(&field.attrs, ident, &prefix)?;

        let is_option = is_option_type(&field.ty);

        let load_expr = if is_option {
            quote! {
                _rws::config_binding::load_optional(&#env_key)?
            }
        } else if let Some(default_str) = default {
            quote! {
                _rws::config_binding::load_with_default(&#env_key, #default_str)?
            }
        } else {
            quote! {
                _rws::config_binding::load_required(&#env_key)?
            }
        };

        field_loads.push(quote! {
            let #ident = #load_expr;
        });
    }

    Ok(quote! {
        impl #name {
            /// Load configuration from environment variables.
            pub fn load() -> ::core::result::Result<Self, ::std::string::String> {
                use ::rust_web_server as _rws;
                #(#field_loads)*
                ::core::result::Result::Ok(#name {
                    #(#field_names),*
                })
            }
        }
    })
}

/// Return `(env_key_expr, Option<default_str>)` for a field.
/// `env_key_expr` is a `proc_macro2::TokenStream` that evaluates to a `String`.
fn parse_field_config(
    attrs: &[syn::Attribute],
    ident: &syn::Ident,
    prefix: &str,
) -> syn::Result<(proc_macro2::TokenStream, Option<LitStr>)> {
    let mut env_name: Option<String> = None;
    let mut default: Option<LitStr> = None;

    for attr in attrs {
        if !attr.path().is_ident("config") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("env") {
                let lit: LitStr = meta.value()?.parse()?;
                env_name = Some(lit.value());
            } else if meta.path.is_ident("default") {
                default = Some(meta.value()?.parse()?);
            } else {
                return Err(meta.error("unknown config key; expected `env` or `default`"));
            }
            Ok(())
        })?;
    }

    let key = format!(
        "{}{}",
        prefix,
        env_name.unwrap_or_else(|| ident.to_string().to_uppercase())
    );

    Ok((quote! { #key }, default))
}

/// Parse `#[config(prefix = "...")]` from struct attributes, returning the prefix string.
fn parse_config_prefix(attrs: &[syn::Attribute]) -> syn::Result<String> {
    let mut prefix = String::new();
    for attr in attrs {
        if !attr.path().is_ident("config") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("prefix") {
                let lit: LitStr = meta.value()?.parse()?;
                prefix = lit.value();
            } else {
                return Err(meta.error("unknown struct config key; expected `prefix`"));
            }
            Ok(())
        })?;
    }
    Ok(prefix)
}

/// Return true if `ty` is `Option<_>`.
fn is_option_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            return seg.ident == "Option";
        }
    }
    false
}

// ── #[derive(Model)] ──────────────────────────────────────────────────────────

/// Derive `rust_web_server::model::Model` for a struct.
///
/// # Struct attributes
///
/// | Attribute | Meaning |
/// |---|---|
/// | `#[table(name = "tbl")]` | Override table name (default: struct name lowercased) |
///
/// # Field attributes
///
/// | Attribute | Meaning |
/// |---|---|
/// | `#[primary_key]` | Mark as primary key |
/// | `#[primary_key(auto_increment)]` | Auto-increment primary key |
/// | `#[column(name = "col")]` | Override column name |
/// | `#[column(unique)]` | Mark column unique (informational) |
/// | `#[ignore]` | Exclude from DB mapping |
///
/// # Example
///
/// ```ignore
/// #[derive(rust_web_server::Model, Debug, Clone)]
/// #[table(name = "users")]
/// pub struct User {
///     #[primary_key(auto_increment)]
///     pub id: i64,
///     #[column(name = "first_name")]
///     pub name: String,
///     pub email: String,
///     #[ignore]
///     pub display_label: String,
/// }
/// ```
#[proc_macro_derive(Model, attributes(table, column, primary_key))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_model(ast)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn impl_model(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let span = input.ident.span();
    let struct_name = &input.ident;

    // ── Determine table name ──────────────────────────────────────────────────
    let table_name = parse_table_name(&input.attrs, struct_name)?;

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            _ => {
                return Err(syn::Error::new(
                    span,
                    "#[derive(Model)] only supports structs with named fields",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new(
                span,
                "#[derive(Model)] can only be derived on structs",
            ))
        }
    };

    // ── Parse field metadata ──────────────────────────────────────────────────
    struct FieldMeta {
        field_ident: syn::Ident,
        col_name: String,
        #[allow(dead_code)]
        is_pk: bool,
        is_auto_increment: bool,
        is_ignored: bool,
    }

    let mut pk_field: Option<FieldMeta> = None;
    let mut regular_fields: Vec<FieldMeta> = Vec::new();

    for field in fields {
        let ident = field.ident.as_ref().unwrap().clone();
        let mut col_name = ident.to_string();
        let mut is_pk = false;
        let mut is_auto_increment = false;
        let mut is_ignored = false;

        for attr in &field.attrs {
            if attr.path().is_ident("ignore") {
                is_ignored = true;
            } else if attr.path().is_ident("primary_key") {
                is_pk = true;
                // Check for (auto_increment)
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("auto_increment") {
                        is_auto_increment = true;
                    }
                    Ok(())
                });
            } else if attr.path().is_ident("column") {
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("name") {
                        let lit: LitStr = meta.value()?.parse()?;
                        col_name = lit.value();
                    }
                    // unique / nullable are informational — ignore
                    Ok(())
                });
            }
        }

        let meta = FieldMeta {
            field_ident: ident,
            col_name,
            is_pk,
            is_auto_increment,
            is_ignored,
        };

        if is_pk {
            pk_field = Some(meta);
        } else {
            regular_fields.push(meta);
        }
    }

    let pk = pk_field.ok_or_else(|| {
        syn::Error::new(span, "#[derive(Model)] requires exactly one #[primary_key] field")
    })?;

    // ── column_names (all mapped fields, including PK) ────────────────────────
    let mut all_col_names: Vec<String> = Vec::new();
    all_col_names.push(pk.col_name.clone());
    for f in regular_fields.iter().filter(|f| !f.is_ignored) {
        all_col_names.push(f.col_name.clone());
    }

    let pk_col_name = pk.col_name.clone();
    let pk_field_ident = &pk.field_ident;
    let auto_inc = pk.is_auto_increment;

    // ── from_row body ─────────────────────────────────────────────────────────
    let pk_from_row = {
        let col = &pk.col_name;
        let fident = &pk.field_ident;
        quote! { #fident: row.get(#col)? }
    };

    let regular_from_row: Vec<proc_macro2::TokenStream> = fields
        .iter()
        .filter(|f| {
            let ident_str = f.ident.as_ref().unwrap().to_string();
            // Skip the PK field (handled separately) and ignored fields.
            let is_pk_field = ident_str == pk_field_ident.to_string();
            !is_pk_field
        })
        .map(|f| {
            let fident = f.ident.as_ref().unwrap();
            // Check if ignored
            let is_ignored = f.attrs.iter().any(|a| a.path().is_ident("ignore"));
            if is_ignored {
                quote! { #fident: ::core::default::Default::default() }
            } else {
                // Determine column name override
                let mut col = fident.to_string();
                for attr in &f.attrs {
                    if attr.path().is_ident("column") {
                        let _ = attr.parse_nested_meta(|meta| {
                            if meta.path.is_ident("name") {
                                if let Ok(lit) = meta.value()?.parse::<LitStr>() {
                                    col = lit.value();
                                }
                            }
                            Ok(())
                        });
                    }
                }
                quote! { #fident: row.get(#col)? }
            }
        })
        .collect();

    // ── to_values body ────────────────────────────────────────────────────────
    let pk_to_values = {
        let col = &pk.col_name;
        quote! {
            (#col, ::rust_web_server::model::ToColumn::to_column(&self.#pk_field_ident))
        }
    };

    let regular_to_values: Vec<proc_macro2::TokenStream> = fields
        .iter()
        .filter(|f| {
            let ident_str = f.ident.as_ref().unwrap().to_string();
            let is_pk_field = ident_str == pk_field_ident.to_string();
            let is_ignored = f.attrs.iter().any(|a| a.path().is_ident("ignore"));
            !is_pk_field && !is_ignored
        })
        .map(|f| {
            let fident = f.ident.as_ref().unwrap();
            let mut col = fident.to_string();
            for attr in &f.attrs {
                if attr.path().is_ident("column") {
                    let _ = attr.parse_nested_meta(|meta| {
                        if meta.path.is_ident("name") {
                            if let Ok(lit) = meta.value()?.parse::<LitStr>() {
                                col = lit.value();
                            }
                        }
                        Ok(())
                    });
                }
            }
            quote! {
                (#col, ::rust_web_server::model::ToColumn::to_column(&self.#fident))
            }
        })
        .collect();

    // ── Generate impl ─────────────────────────────────────────────────────────
    Ok(quote! {
        impl ::rust_web_server::model::Model for #struct_name {
            fn table_name() -> &'static str {
                #table_name
            }

            fn column_names() -> &'static [&'static str] {
                &[#(#all_col_names),*]
            }

            fn primary_key_name() -> &'static str {
                #pk_col_name
            }

            fn primary_key_value(&self) -> ::rust_web_server::model::Value {
                ::rust_web_server::model::ToColumn::to_column(&self.#pk_field_ident)
            }

            fn primary_key_auto_increment() -> bool {
                #auto_inc
            }

            fn from_row(row: &::rust_web_server::model::ModelRow) -> ::core::result::Result<Self, ::rust_web_server::model::DbError> {
                ::core::result::Result::Ok(#struct_name {
                    #pk_from_row,
                    #(#regular_from_row),*
                })
            }

            fn to_values(&self) -> ::std::vec::Vec<(&'static str, ::rust_web_server::model::Value)> {
                vec![
                    #pk_to_values,
                    #(#regular_to_values),*
                ]
            }
        }

        impl #struct_name {
            /// Create a `ModelRepository` tied to the given connection.
            pub fn repository(conn: &mut ::rust_web_server::model::DbConnection) -> ::rust_web_server::model::ModelRepository<#struct_name, i64> {
                ::rust_web_server::model::ModelRepository::new(conn)
            }

            /// Create a `QueryBuilder` tied to the given connection.
            pub fn query(conn: &mut ::rust_web_server::model::DbConnection) -> ::rust_web_server::model::QueryBuilder<#struct_name> {
                ::rust_web_server::model::QueryBuilder::new(conn)
            }
        }
    })
}

fn parse_table_name(
    attrs: &[syn::Attribute],
    struct_name: &syn::Ident,
) -> syn::Result<String> {
    for attr in attrs {
        if attr.path().is_ident("table") {
            let mut name: Option<String> = None;
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    let lit: LitStr = meta.value()?.parse()?;
                    name = Some(lit.value());
                }
                Ok(())
            })?;
            if let Some(n) = name {
                return Ok(n);
            }
        }
    }
    // Default: lowercased struct name.
    Ok(struct_name.to_string().to_lowercase())
}
