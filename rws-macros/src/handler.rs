//! `#[get]`/`#[post]`/… on a typed handler (18.28.0, #86): the function
//! becomes a unit struct implementing `rust_web_server::handler::Handler`
//! and `AsyncHandler`, with the route's parameters checked against its
//! `Path` argument here, at compile time.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{FnArg, GenericArgument, ItemFn, LitStr, Pat, PathArguments, ReturnType, Type};

/// The attribute's output for `func`: a doc comment on a classic handler,
/// the generated adapter on a typed one.
pub(crate) fn expand(method: &str, path: LitStr, func: ItemFn) -> TokenStream {
    let doc = format!("Route: `{} {}`", method, path.value());
    if is_classic(&func) {
        return quote! {
            #[doc = #doc]
            #func
        };
    }
    typed(method, &path, func, &doc).unwrap_or_else(|e| e.to_compile_error())
}

/// `fn(&Request, &PathParams, &ConnectionInfo[, state])`, by reference or
/// by value (`AsyncAppWithState`'s): registered with `.get(pattern, f)`.
fn is_classic(func: &ItemFn) -> bool {
    let inputs: Vec<&FnArg> = func.sig.inputs.iter().collect();
    if inputs.len() != 3 && inputs.len() != 4 {
        return false;
    }
    let expected = ["Request", "PathParams", "ConnectionInfo"];
    inputs.iter().zip(expected).all(|(arg, name)| match arg {
        FnArg::Typed(pt) => {
            let ty = match &*pt.ty {
                Type::Reference(r) => &*r.elem,
                other => other,
            };
            last_ident(ty).as_deref() == Some(name)
        }
        FnArg::Receiver(_) => false,
    })
}

fn last_ident(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(tp) if tp.qself.is_none() => tp.path.segments.last().map(|s| s.ident.to_string()),
        Type::Group(g) => last_ident(&g.elem),
        Type::Paren(p) => last_ident(&p.elem),
        _ => None,
    }
}

/// `Wrapper<T>`'s `T`, when `ty`'s last segment is `wrapper`.
fn single_type_arg<'a>(ty: &'a Type, wrapper: &str) -> Option<&'a Type> {
    let Type::Path(tp) = ty else { return None };
    let seg = tp.path.segments.last()?;
    if seg.ident != wrapper {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &seg.arguments else { return None };
    let mut types = args.args.iter().filter_map(|a| match a {
        GenericArgument::Type(t) => Some(t),
        _ => None,
    });
    let first = types.next()?;
    types.next().is_none().then_some(first)
}

/// The parameter names of a route pattern, in order (`:id`, `{id}`,
/// `*rest`, `{*rest}`; `{{`/`}}` are literal braces).
fn route_params(pattern: &str) -> Vec<String> {
    pattern
        .split('/')
        .filter(|s| !s.is_empty())
        .filter_map(|seg| {
            if let Some(name) = seg.strip_prefix(':').or_else(|| seg.strip_prefix('*')) {
                return Some(name.to_string());
            }
            let inner = seg.strip_prefix('{')?.strip_suffix('}')?;
            let valid = !inner.is_empty() && !inner.starts_with('{') && !inner.contains(['{', '}', '/']);
            valid.then(|| inner.trim_start_matches('*').to_string())
        })
        .collect()
}

const SCALARS: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "f32", "f64", "bool",
    "char", "String", "IpAddr", "Ipv4Addr", "Ipv6Addr",
];

enum Source {
    Request,
    Connection,
    Params,
    StateRef,
    Extract(Type),
}

/// The name a binding gives a route parameter: `id` and `_id` name `id`;
/// `_` and anything else name nothing.
fn binding_name(pat: &Pat) -> Option<(String, Span)> {
    match pat {
        Pat::Ident(pi) if pi.subpat.is_none() => {
            let name = pi.ident.to_string();
            let name = name.strip_prefix("r#").unwrap_or(&name);
            let name = name.strip_prefix('_').unwrap_or(name);
            (!name.is_empty()).then(|| (name.to_string(), pi.ident.span()))
        }
        _ => None,
    }
}

fn list(names: &[String]) -> String {
    names.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ")
}

fn typed(method: &str, path: &LitStr, func: ItemFn, doc: &str) -> syn::Result<TokenStream> {
    let sig = &func.sig;
    let name = &sig.ident;
    let route = format!("{} {}", method, path.value());
    if let Some(receiver) = sig.receiver() {
        return Err(syn::Error::new(receiver.span(), "a route handler can't take `self`; make it a free function"));
    }
    if !sig.generics.params.is_empty() || sig.generics.where_clause.is_some() {
        return Err(syn::Error::new(sig.generics.span(), "a typed route handler can't be generic"));
    }
    if let Some(v) = &sig.variadic {
        return Err(syn::Error::new(v.span(), "a route handler can't be variadic"));
    }
    if let ReturnType::Default = sig.output {
        return Err(syn::Error::new(
            sig.ident.span(),
            "a typed route handler returns a value that implements `IntoResponse` (a `Response`, `Json<T>`, `String`, `Result<T, E>`, …)",
        ));
    }

    let params = route_params(&path.value());
    for (i, p) in params.iter().enumerate() {
        if params[..i].contains(p) {
            return Err(syn::Error::new(path.span(), format!("route `{route}` names the parameter `{p}` twice")));
        }
    }

    let mut sources = Vec::new();
    let mut state_ty: Option<Type> = None;
    let mut takes_params = false;
    let mut path_arg: Option<(Span, Vec<String>, Option<Type>)> = None;

    for arg in &sig.inputs {
        let FnArg::Typed(pt) = arg else { unreachable!("receiver rejected above") };
        let ty = &*pt.ty;
        if let Type::Reference(r) = ty {
            let source = match last_ident(&r.elem).as_deref() {
                Some("Request") => Source::Request,
                Some("ConnectionInfo") => Source::Connection,
                Some("PathParams") => {
                    takes_params = true;
                    Source::Params
                }
                _ => {
                    if r.mutability.is_some() {
                        return Err(syn::Error::new(ty.span(), "handler arguments can't be `&mut`; the state is shared (use a `Mutex` inside it)"));
                    }
                    state_ty.get_or_insert_with(|| (*r.elem).clone());
                    Source::StateRef
                }
            };
            sources.push(source);
            continue;
        }
        if let Some(inner) = single_type_arg(ty, "State") {
            state_ty.get_or_insert_with(|| inner.clone());
        }
        if let Some(inner) = single_type_arg(ty, "Path") {
            if path_arg.is_some() {
                return Err(syn::Error::new(ty.span(), "a handler takes at most one `Path<…>` argument; use a tuple or a #[derive(FromPath)] struct for several parameters"));
            }
            let names = check_path(&route, &params, &pt.pat, ty, inner)?;
            let needs_const_check = names.is_none();
            let names = names.unwrap_or_else(|| params.clone());
            path_arg = Some((ty.span(), names, needs_const_check.then(|| inner.clone())));
        }
        sources.push(Source::Extract(ty.clone()));
    }

    if path_arg.is_none() && !takes_params && !params.is_empty() {
        let noun = if params.len() == 1 { "parameter" } else { "parameters" };
        return Err(syn::Error::new(
            name.span(),
            format!(
                "route `{route}` has the {noun} {}, but `{name}` takes no `Path<…>` (or `&PathParams`) argument",
                list(&params)
            ),
        ));
    }

    let vis = &func.vis;
    let asyncness = &sig.asyncness;
    let inputs = &sig.inputs;
    let output = &sig.output;
    let block = &func.block;
    let (docs, rest): (Vec<_>, Vec<_>) = func.attrs.iter().partition(|a| a.path().is_ident("doc"));
    let cfgs: Vec<_> = rest.iter().filter(|a| a.path().is_ident("cfg")).collect();
    let fn_attrs: Vec<_> = rest.iter().filter(|a| !a.path().is_ident("cfg")).collect();

    let rws = quote!(::rust_web_server);
    let (impl_generics, state) = match &state_ty {
        Some(t) => (quote!(), quote!(#t)),
        None => (
            quote!(<__S: ::core::marker::Send + ::core::marker::Sync + 'static>),
            quote!(__S),
        ),
    };
    let path_names: Vec<&String> = path_arg.as_ref().map(|(_, n, _)| n.iter().collect()).unwrap_or_default();

    let arg_idents: Vec<_> = (0..sources.len()).map(|i| format_ident!("__rws_arg{}", i)).collect();
    let extractions = sources.iter().zip(&arg_idents).map(|(source, ident)| {
        let value = match source {
            Source::Request => quote!(request),
            Source::Connection => quote!(connection),
            Source::Params => quote!(params),
            Source::StateRef => quote!(&**state),
            Source::Extract(ty) => quote_spanned! {ty.span()=>
                match <#ty as #rws::handler::FromRequestParts<#state>>::from_request_parts(&__parts) {
                    ::core::result::Result::Ok(value) => value,
                    ::core::result::Result::Err(rejection) => return rejection,
                }
            },
        };
        quote!(let #ident = #value;)
    });
    let extractions: Vec<_> = extractions.collect();
    let parts = quote! {
        #[allow(unused_variables)]
        let __parts = #rws::handler::RequestParts {
            request, params, connection, state,
            path_names: &[#(#path_names),*],
        };
    };

    let (handler_impl, async_body) = if asyncness.is_some() {
        (
            quote!(),
            quote! {
                let request = &request;
                let params = &params;
                let connection = &connection;
                let state = &state;
                #parts
                #(#extractions)*
                #rws::handler::__private::respond(#name::call(#(#arg_idents),*).await)
            },
        )
    } else {
        (
            quote! {
                #(#cfgs)*
                impl #impl_generics #rws::handler::Handler<#state> for #name {
                    #[allow(unused_variables)]
                    fn handle(
                        &self,
                        request: &#rws::request::Request,
                        params: &#rws::router::PathParams,
                        connection: &#rws::server::ConnectionInfo,
                        state: &::std::sync::Arc<#state>,
                    ) -> #rws::response::Response {
                        #parts
                        #(#extractions)*
                        #rws::handler::__private::respond(#name::call(#(#arg_idents),*))
                    }
                }
            },
            quote! {
                <#name as #rws::handler::Handler<#state>>::handle(&#name, &request, &params, &connection, &state)
            },
        )
    };

    let const_check = path_arg.as_ref().and_then(|(span, _, ty)| {
        let ty = ty.as_ref()?;
        let names = &params;
        let message = format!(
            "route `{route}`: this `Path<…>` type doesn't take exactly the route's parameters ({}); a #[derive(FromPath)] struct needs one field per parameter, named after it",
            list(&params)
        )
        // A format string to `assert!`.
        .replace('{', "{{")
        .replace('}', "}}");
        Some(quote_spanned! {*span=>
            #(#cfgs)*
            const _: () = ::core::assert!(#rws::handler::__private::path_fits::<#ty>(&[#(#names),*]), #message);
        })
    });

    let method_lit = method;
    let pattern = path.value();
    Ok(quote! {
        #[doc = #doc]
        #(#docs)*
        #(#cfgs)*
        #[allow(non_camel_case_types)]
        #[derive(Clone, Copy, Debug, Default)]
        #vis struct #name;

        #(#cfgs)*
        impl #name {
            /// The handler as written, callable directly (in tests, say).
            #(#fn_attrs)*
            #vis #asyncness fn call(#inputs) #output #block
        }

        #(#cfgs)*
        impl #rws::handler::RouteSpec for #name {
            const METHOD: &'static str = #method_lit;
            const PATTERN: &'static str = #pattern;
        }

        #handler_impl

        #(#cfgs)*
        impl #impl_generics #rws::handler::AsyncHandler<#state> for #name {
            #[allow(unused_variables)]
            fn handle_async(
                &self,
                request: #rws::request::Request,
                params: #rws::router::PathParams,
                connection: #rws::server::ConnectionInfo,
                state: ::std::sync::Arc<#state>,
            ) -> #rws::handler::HandlerFuture {
                ::std::boxed::Box::pin(async move { #async_body })
            }
        }

        #const_check
    })
}

/// Check a `Path<inner>` argument (bound by `pat`) against the route's
/// parameters. `Some(names)` gives the parameters it takes, in its order,
/// when that's known here; `None` leaves the check to a `const` assertion
/// on the type's `FromPath` impl (a `#[derive(FromPath)]` struct).
fn check_path(route: &str, params: &[String], pat: &Pat, ty: &Type, inner: &Type) -> syn::Result<Option<Vec<String>>> {
    if params.is_empty() {
        return Err(syn::Error::new(ty.span(), format!("route `{route}` has no parameters for this `Path<…>` argument")));
    }
    let noun = |n: usize| if n == 1 { "parameter" } else { "parameters" };
    // The bindings inside `Path(…)`.
    let bound: Option<Vec<&Pat>> = match pat {
        Pat::TupleStruct(ts) if ts.elems.len() == 1 => match &ts.elems[0] {
            Pat::Tuple(t) => Some(t.elems.iter().collect()),
            single => Some(vec![single]),
        },
        _ => None,
    };
    let check_names = |pats: &[&Pat]| -> syn::Result<Option<Vec<String>>> {
        let mut names = Vec::new();
        for p in pats {
            let Some((n, span)) = binding_name(p) else { return Ok(None) };
            if !params.contains(&n) {
                return Err(syn::Error::new(
                    span,
                    format!("`{n}` is not a parameter of route `{route}` (its {}: {})", noun(params.len()), list(params)),
                ));
            }
            if names.contains(&n) {
                return Err(syn::Error::new(span, format!("route parameter `{n}` is bound twice")));
            }
            names.push(n);
        }
        Ok(Some(names))
    };

    if let Type::Tuple(tuple) = inner {
        let arity = tuple.elems.len();
        if arity != params.len() {
            return Err(syn::Error::new(
                ty.span(),
                format!(
                    "route `{route}` has {} {} ({}), but this `Path<…>` tuple takes {arity}",
                    params.len(),
                    noun(params.len()),
                    list(params)
                ),
            ));
        }
        let by_name = match bound.as_deref() {
            Some(pats) if pats.len() == arity => check_names(pats)?,
            _ => None,
        };
        return Ok(Some(by_name.unwrap_or_else(|| params.to_vec())));
    }

    let scalar = last_ident(inner).is_some_and(|i| SCALARS.contains(&i.as_str()));
    if scalar {
        if params.len() != 1 {
            return Err(syn::Error::new(
                ty.span(),
                format!(
                    "route `{route}` has {} parameters ({}), but this `Path<…>` takes one; use a tuple or a #[derive(FromPath)] struct",
                    params.len(),
                    list(params)
                ),
            ));
        }
        if let Some(pats) = bound.as_deref() {
            check_names(pats)?;
        }
        return Ok(Some(params.to_vec()));
    }
    Ok(None)
}

/// `#[derive(FromPath)]` on a struct with named fields.
pub(crate) fn derive_from_path(ast: syn::DeriveInput) -> syn::Result<TokenStream> {
    let name = &ast.ident;
    let syn::Data::Struct(data) = &ast.data else {
        return Err(syn::Error::new(name.span(), "#[derive(FromPath)] needs a struct with named fields"));
    };
    let syn::Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new(name.span(), "#[derive(FromPath)] needs a struct with named fields, one per route parameter"));
    };
    let idents: Vec<_> = fields.named.iter().filter_map(|f| f.ident.as_ref()).collect();
    let names: Vec<String> = idents
        .iter()
        .map(|i| {
            let s = i.to_string();
            s.strip_prefix("r#").unwrap_or(&s).to_string()
        })
        .collect();
    let arity = names.len();
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics ::rust_web_server::handler::FromPath for #name #ty_generics #where_clause {
            const ARITY: usize = #arity;
            const NAMES: &'static [&'static str] = &[#(#names),*];
            fn from_path(
                params: &::rust_web_server::router::PathParams,
                _names: &[&str],
            ) -> ::core::result::Result<Self, ::std::string::String> {
                ::core::result::Result::Ok(#name {
                    #(#idents: ::rust_web_server::handler::__private::parse_param(params, #names)?,)*
                })
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn func(src: &str) -> ItemFn {
        syn::parse_str(src).unwrap()
    }

    fn expand_str(pattern: &str, src: &str) -> String {
        let path: LitStr = syn::parse_str(&format!("{pattern:?}")).unwrap();
        expand("GET", path, func(src)).to_string()
    }

    #[test]
    fn route_params_reads_every_syntax() {
        assert_eq!(vec!["id", "rest"], route_params("/a/:id/*rest"));
        assert_eq!(vec!["id", "rest"], route_params("/a/{id}/{*rest}"));
        assert_eq!(Vec::<String>::new(), route_params("/a/{{lit}}/b"));
        assert_eq!(Vec::<String>::new(), route_params("/"));
    }

    #[test]
    fn classic_handlers_keep_their_function() {
        let out = expand_str("/x/:id", "fn h(_: &Request, p: &PathParams, c: &ConnectionInfo, s: &Db) -> Response { todo!() }");
        assert!(out.contains("fn h"), "{out}");
        assert!(!out.contains("struct h"), "{out}");
        // AsyncAppWithState's owned form, and the 3-argument Router form.
        assert!(is_classic(&func("async fn h(r: Request, p: PathParams, c: ConnectionInfo, s: Arc<S>) -> Response { todo!() }")));
        assert!(is_classic(&func("fn h(r: &rws::request::Request, p: &PathParams, c: &ConnectionInfo) -> Response { todo!() }")));
        assert!(!is_classic(&func("fn h(r: &Request, c: &ConnectionInfo) -> Response { todo!() }")));
        assert!(!is_classic(&func("fn h(Path(id): Path<u64>) -> String { todo!() }")));
    }

    #[test]
    fn typed_handlers_become_a_struct_with_the_route() {
        let out = expand_str("/posts/{id}", "fn show(Path(id): Path<u64>, State(db): State<Db>) -> String { todo!() }");
        assert!(out.contains("struct show"), "{out}");
        assert!(out.contains("Handler < Db >"), "{out}");
        assert!(out.contains("\"/posts/{id}\""), "{out}");
        assert!(out.contains("path_names : & [\"id\"]"), "{out}");
        // No state argument: generic over the app's state.
        let out = expand_str("/ping", "fn ping() -> String { todo!() }");
        assert!(out.contains("__S"), "{out}");
    }

    #[test]
    fn tuple_bindings_choose_the_order_and_structs_get_a_const_check() {
        let out = expand_str("/a/{x}/{y}", "fn h(Path((y, x)): Path<(u8, u8)>) -> String { todo!() }");
        assert!(out.contains("path_names : & [\"y\" , \"x\"]"), "{out}");
        let out = expand_str("/a/{x}/{y}", "fn h(Path(key): Path<Key>) -> String { todo!() }");
        assert!(out.contains("path_fits :: < Key >"), "{out}");
        // Braces in the message are escaped for `assert!`'s format string.
        assert!(out.contains("{{x}}"), "{out}");
    }

    #[test]
    fn mismatches_are_compile_errors() {
        for (pattern, src) in [
            ("/p/{id}", "fn h(Path(other): Path<u64>) -> String { todo!() }"),
            ("/p/{id}", "fn h(Path((a, b)): Path<(u8, u8)>) -> String { todo!() }"),
            ("/p/{id}", "fn h() -> String { todo!() }"),
            ("/p", "fn h(p: Path<u8>) -> String { todo!() }"),
            ("/p/{a}/{b}", "fn h(p: Path<u8>) -> String { todo!() }"),
            ("/p/{a}", "fn h(p: Path<u8>, q: Path<u8>) -> String { todo!() }"),
            ("/p", "fn h<T>() -> String { todo!() }"),
            ("/p", "fn h(db: &mut Db) -> String { todo!() }"),
        ] {
            let out = expand_str(pattern, src);
            assert!(out.contains("compile_error"), "{pattern} {src}: {out}");
        }
        // `&PathParams` takes the parameters too; `_` bindings aren't checked.
        assert!(!expand_str("/p/{id}", "fn h(p: &PathParams) -> String { todo!() }").contains("compile_error"));
        assert!(!expand_str("/p/{id}", "fn h(Path(_): Path<u64>) -> String { todo!() }").contains("compile_error"));
        assert!(!expand_str("/p/{id}", "fn h(Path(_id): Path<u64>) -> String { todo!() }").contains("compile_error"));
    }

    #[test]
    fn derive_from_path_names_the_fields() {
        let ast: syn::DeriveInput = syn::parse_str("struct Key { id: u64, slug: String }").unwrap();
        let out = derive_from_path(ast).unwrap().to_string();
        assert!(out.contains("NAMES : & 'static [& 'static str] = & [\"id\" , \"slug\"]"), "{out}");
        let tuple: syn::DeriveInput = syn::parse_str("struct Key(u64);").unwrap();
        assert!(derive_from_path(tuple).is_err());
    }
}
