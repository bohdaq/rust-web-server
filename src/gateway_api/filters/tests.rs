use super::*;
use crate::gateway_api::types::HttpPathMatch;

fn request(method: &str, uri: &str, headers: &[(&str, &str)]) -> Request {
    Request {
        method: method.to_string(),
        request_uri: uri.to_string(),
        http_version: "HTTP/1.1".to_string(),
        headers: headers.iter().map(|(n, v)| Header::new(n.to_string(), v.to_string())).collect(),
        body: Default::default(),
    }
}

fn prefix(value: &str) -> HttpRouteMatch {
    HttpRouteMatch { path: Some(HttpPathMatch { match_type: Some("PathPrefix".to_string()), value: Some(value.to_string()) }), ..Default::default() }
}

fn replace_prefix(value: &str) -> HttpPathModifier {
    HttpPathModifier { modifier_type: "ReplacePrefixMatch".to_string(), replace_prefix_match: Some(value.to_string()), ..Default::default() }
}

fn header(name: &str, value: &str) -> HttpHeader {
    HttpHeader { name: name.to_string(), value: value.to_string() }
}

fn location(response: &Response) -> (i16, String) {
    (response.status_code, response.headers.iter().find(|h| h.name == "Location").map(|h| h.value.to_string()).unwrap_or_default())
}

fn names(headers: &[Header]) -> Vec<(String, String)> {
    headers.iter().map(|h| (h.name.to_string(), h.value.to_string())).collect()
}

#[test]
fn header_modifier_sets_adds_and_removes_case_insensitively() {
    let mut headers = vec![Header::new("x-header-set", "old"), Header::new("x-header-add", "a"), Header::new("x-header-remove", "r"), Header::new("Keep", "k")];
    let filter = HttpHeaderFilter { set: vec![header("X-Header-Set", "new")], add: vec![header("X-Header-Add", "b")], remove: vec!["X-Header-Remove".to_string()] };
    modify_headers(&mut headers, &filter);
    assert_eq!(
        vec![("x-header-add".into(), "a".into()), ("Keep".into(), "k".into()), ("X-Header-Set".into(), "new".into()), ("X-Header-Add".into(), "b".into())],
        names(&headers)
    );
    modify_headers(&mut headers, &HttpHeaderFilter::default());
    assert_eq!(4, headers.len(), "an empty filter changes nothing");
}

#[test]
fn prefix_replacement_follows_the_suites_cases() {
    let cases = [
        ("/prefix/one", "/one", "/prefix/one/two", "/one/two"),
        ("/strip-prefix", "/", "/strip-prefix/three", "/three"),
        ("/strip-prefix", "/", "/strip-prefix", "/"),
        ("/prefix/rewrite-path-and-modify-headers", "/prefix", "/prefix/rewrite-path-and-modify-headers/one", "/prefix/one"),
        ("/original-prefix", "/replacement-prefix", "/original-prefix/lemon", "/replacement-prefix/lemon"),
        ("/path-and-host", "/replacement-prefix", "/path-and-host", "/replacement-prefix"),
        ("/trailing/", "/x", "/trailing/a", "/x/a"),
        ("/", "/api", "/a/b", "/api/a/b"),
    ];
    for (matched, replacement, path, expected) in cases {
        assert_eq!(expected, modified_path(path, &prefix(matched), &replace_prefix(replacement)), "{matched} → {replacement} on {path}");
    }
    let full = HttpPathModifier { modifier_type: "ReplaceFullPath".to_string(), replace_full_path: Some("/one".to_string()), ..Default::default() };
    assert_eq!("/one", modified_path("/full/one/two", &prefix("/full/one"), &full));
    assert_eq!("/other", modified_path("/other", &prefix("/x"), &replace_prefix("/y")), "a path the prefix doesn't match is kept");
}

#[test]
fn redirects_derive_scheme_host_and_port() {
    let req = request("GET", "/p?q=1", &[("host", "example.org")]);
    let m = prefix("/");
    let redirect_with = |scheme: Option<&str>, port: Option<i32>| HttpRequestRedirectFilter { scheme: scheme.map(str::to_string), port, ..Default::default() };
    assert_eq!((302, "http://example.org/p?q=1".to_string()), location(&redirect(&req, "example.org", false, 80, &m, &redirect_with(None, None))));
    assert_eq!((302, "http://example.org:8080/p?q=1".to_string()), location(&redirect(&req, "example.org", false, 80, &m, &redirect_with(None, Some(8080)))));
    assert_eq!((302, "https://example.org/p?q=1".to_string()), location(&redirect(&req, "example.org", false, 80, &m, &redirect_with(Some("https"), None))));
    assert_eq!((302, "https://example.org:8443/p?q=1".to_string()), location(&redirect(&req, "example.org", false, 80, &m, &redirect_with(Some("https"), Some(8443)))));
    assert_eq!((302, "http://example.org:8080/p?q=1".to_string()), location(&redirect(&req, "example.org", false, 8080, &m, &redirect_with(None, None))), "the listener's port");
    assert_eq!((302, "https://example.org/p?q=1".to_string()), location(&redirect(&req, "example.org", true, 443, &m, &redirect_with(None, None))), "the listener's scheme");
    let moved = HttpRequestRedirectFilter {
        hostname: Some("example.net".to_string()),
        status_code: Some(301),
        path: Some(replace_prefix("/new")),
        ..Default::default()
    };
    assert_eq!((301, "http://example.net/new/x".to_string()), location(&redirect(&request("GET", "/old/x", &[]), "example.org", false, 80, &prefix("/old"), &moved)));
    for code in [303, 307, 308] {
        let filter = HttpRequestRedirectFilter { status_code: Some(code), ..Default::default() };
        assert_eq!(code as i16, redirect(&req, "example.org", false, 80, &m, &filter).status_code);
    }
}

fn cors_filter() -> HttpCorsFilter {
    HttpCorsFilter {
        allow_origins: vec!["https://www.foo.com".to_string(), "https://*.bar.com".to_string()],
        allow_credentials: Some(true),
        allow_methods: vec!["GET".to_string(), "OPTIONS".to_string()],
        allow_headers: vec!["x-header-1".to_string(), "x-header-2".to_string()],
        expose_headers: vec!["x-header-3".to_string()],
        max_age: 3600,
    }
}

fn value(headers: &[Header], name: &str) -> Option<String> {
    headers.iter().find(|h| h.name.eq_ignore_ascii_case(name)).map(|h| h.value.to_string())
}

#[test]
fn cors_answers_preflights_and_decorates_actual_requests() {
    let preflight = |origin: &str| request("OPTIONS", "/cors-1", &[("origin", origin), ("access-control-request-method", "GET")]);
    let Cors::Preflight(headers) = cors(&preflight("https://xpto.www.bar.com"), &cors_filter()) else { panic!("a preflight") };
    assert_eq!(Some("https://xpto.www.bar.com".to_string()), value(&headers, "access-control-allow-origin"), "a wildcard covers several labels");
    assert_eq!(Some("GET, OPTIONS".to_string()), value(&headers, "access-control-allow-methods"));
    assert_eq!(Some("x-header-1, x-header-2".to_string()), value(&headers, "access-control-allow-headers"));
    assert_eq!(Some("3600".to_string()), value(&headers, "access-control-max-age"));
    assert_eq!(Some("true".to_string()), value(&headers, "access-control-allow-credentials"));
    assert_eq!(Cors::Preflight(vec![]), cors(&preflight("https://foobar.com"), &cors_filter()), "an origin that isn't allowed gets nothing");
    assert_eq!(Cors::Preflight(vec![]), cors(&preflight("https://bar.com"), &cors_filter()), "`*.` needs a label");

    let Cors::Actual(headers) = cors(&request("GET", "/cors-1", &[("origin", "https://www.foo.com")]), &cors_filter()) else { panic!("actual") };
    assert_eq!(Some("https://www.foo.com".to_string()), value(&headers, "access-control-allow-origin"));
    assert_eq!(Some("x-header-3".to_string()), value(&headers, "access-control-expose-headers"));
    assert_eq!(Cors::Pass, cors(&request("GET", "/cors-1", &[("origin", "https://foobar.com")]), &cors_filter()));
    assert_eq!(Cors::Pass, cors(&request("GET", "/cors-1", &[]), &cors_filter()), "not cross-origin");
}

#[test]
fn cors_wildcards_without_credentials() {
    let filter = HttpCorsFilter { allow_origins: vec!["*".to_string()], allow_methods: vec!["*".to_string()], ..Default::default() };
    let Cors::Preflight(headers) = cors(&request("OPTIONS", "/", &[("origin", "https://a.com"), ("access-control-request-method", "POST")]), &filter) else { panic!() };
    assert_eq!(Some("*".to_string()), value(&headers, "access-control-allow-origin"));
    assert_eq!(Some("*".to_string()), value(&headers, "access-control-allow-methods"));
    assert_eq!(None, value(&headers, "access-control-allow-credentials"));
    let with_credentials = HttpCorsFilter { allow_credentials: Some(true), ..filter };
    let Cors::Preflight(headers) = cors(&request("OPTIONS", "/", &[("origin", "https://a.com"), ("access-control-request-method", "POST")]), &with_credentials) else { panic!() };
    assert_eq!(Some("https://a.com".to_string()), value(&headers, "access-control-allow-origin"), "credentials echo the origin");
    assert_eq!(Some("POST".to_string()), value(&headers, "access-control-allow-methods"));
}
