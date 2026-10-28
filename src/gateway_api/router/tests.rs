use super::*;
use crate::gateway_api::routes::Backend;
use crate::gateway_api::types::{HttpHeaderMatch, HttpPathMatch};

fn request(method: &str, uri: &str, headers: &[(&str, &str)]) -> Request {
    Request {
        method: method.to_string(),
        request_uri: uri.to_string(),
        http_version: "HTTP/1.1".to_string(),
        headers: headers.iter().map(|(n, v)| crate::header::Header::new(n.to_string(), v.to_string())).collect(),
        body: Default::default(),
    }
}

fn path(kind: &str, value: &str) -> HttpRouteMatch {
    HttpRouteMatch { path: Some(HttpPathMatch { match_type: Some(kind.to_string()), value: Some(value.to_string()) }), ..Default::default() }
}

fn rule(name: &str, matches: Vec<HttpRouteMatch>, backends: Vec<Backend>) -> CompiledRule {
    CompiledRule { name: Some(name.to_string()), matches, filters: vec![], backends, timeouts: None, retry: None, mirrors: vec![] }
}

fn backend(weight: u32, endpoints: Result<Vec<String>, String>) -> Backend {
    Backend { weight, endpoints, app_protocol: None, tls: None, filters: vec![] }
}

fn route(name: &str, created: &str, hostnames: &[&str], rules: Vec<CompiledRule>) -> CompiledHttpRoute {
    CompiledHttpRoute {
        key: ("ns".to_string(), name.to_string()),
        hostnames: hostnames.iter().map(|h| h.to_string()).collect(),
        created: created.to_string(),
        rules,
    }
}

fn listener(hostname: Option<&str>, routes: Vec<CompiledHttpRoute>) -> ListenerConfig {
    ListenerConfig {
        gateway: ("ns".to_string(), "gw".to_string()),
        name: hostname.unwrap_or("any").to_string(),
        port: 80,
        protocol: "HTTP".to_string(),
        hostname: hostname.map(str::to_string),
        container_port: 10000,
        tls: None,
        client_ca: None,
        backend_client_cert: None,
        http_routes: routes,
    }
}

fn picked(l: &ListenerConfig, r: &Request) -> Option<String> {
    select(l, r, &request_host(r)).and_then(|(_, rule, _)| rule.name.clone())
}

#[test]
fn prefixes_match_whole_segments() {
    assert!(prefix_matches("/", "/anything"));
    assert!(prefix_matches("/foo", "/foo"));
    assert!(prefix_matches("/foo", "/foo/bar"));
    assert!(prefix_matches("/foo/", "/foo"), "a trailing slash on the prefix is ignored");
    assert!(!prefix_matches("/foo", "/foobar"));
    assert!(!prefix_matches("/foo/bar", "/foo"));
}

#[test]
fn hosts_score_exact_above_wildcard_and_long_above_short() {
    assert_eq!(Some((7, 7)), host_score("foo.com", "foo.com"));
    assert_eq!(None, host_score("foo.com", "bar.com"));
    assert!(host_score("*.bar.com", "a.bar.com").unwrap() < host_score("a.bar.com", "a.bar.com").unwrap());
    assert!(host_score("*.bar.com", "x.a.bar.com").unwrap() < host_score("*.a.bar.com", "x.a.bar.com").unwrap());
    assert_eq!(None, host_score("*.bar.com", "bar.com"));
    assert_eq!("example.com", request_host(&request("GET", "/", &[("Host", "Example.COM:8080")])));
    assert_eq!("[::1]", request_host(&request("GET", "/", &[("Host", "[::1]:80")])));
}

#[test]
fn exact_beats_prefix_and_longer_prefixes_beat_shorter() {
    let l = listener(None, vec![route("r", "2026-01-01", &[], vec![
        rule("root", vec![path("PathPrefix", "/")], vec![]),
        rule("v1", vec![path("PathPrefix", "/v1")], vec![]),
        rule("exact", vec![path("Exact", "/v1/status")], vec![]),
    ])]);
    assert_eq!(Some("exact".to_string()), picked(&l, &request("GET", "/v1/status", &[])));
    assert_eq!(Some("v1".to_string()), picked(&l, &request("GET", "/v1/users?x=1", &[])));
    assert_eq!(Some("root".to_string()), picked(&l, &request("GET", "/v2", &[])));
}

#[test]
fn methods_headers_and_query_params_narrow_and_rank() {
    let with = |method: Option<&str>, headers: &[(&str, &str)], query: &[(&str, &str)]| HttpRouteMatch {
        path: Some(HttpPathMatch { match_type: Some("PathPrefix".to_string()), value: Some("/".to_string()) }),
        method: method.map(str::to_string),
        headers: headers.iter().map(|(n, v)| HttpHeaderMatch { match_type: None, name: n.to_string(), value: v.to_string() }).collect(),
        query_params: query.iter().map(|(n, v)| HttpHeaderMatch { match_type: None, name: n.to_string(), value: v.to_string() }).collect(),
    };
    let l = listener(None, vec![route("r", "2026-01-01", &[], vec![
        rule("plain", vec![with(None, &[], &[])], vec![]),
        rule("post", vec![with(Some("POST"), &[], &[])], vec![]),
        rule("one-header", vec![with(None, &[("version", "one")], &[])], vec![]),
        rule("two-headers", vec![with(None, &[("version", "one"), ("color", "red")], &[])], vec![]),
        rule("query", vec![with(None, &[], &[("animal", "whale")])], vec![]),
    ])]);
    assert_eq!(Some("post".to_string()), picked(&l, &request("POST", "/", &[])));
    assert_eq!(Some("two-headers".to_string()), picked(&l, &request("GET", "/", &[("version", "one"), ("color", "red")])));
    assert_eq!(Some("one-header".to_string()), picked(&l, &request("GET", "/", &[("version", "one")])));
    assert_eq!(Some("query".to_string()), picked(&l, &request("GET", "/?animal=whale", &[])));
    assert_eq!(Some("plain".to_string()), picked(&l, &request("GET", "/?animal=dolphin", &[])));
}

#[test]
fn hostnames_then_age_then_name_break_ties() {
    let l = listener(None, vec![
        route("young", "2026-02-01", &[], vec![rule("young", vec![path("PathPrefix", "/")], vec![])]),
        route("old", "2026-01-01", &[], vec![rule("old", vec![path("PathPrefix", "/")], vec![])]),
        route("specific", "2026-03-01", &["foo.example.com"], vec![rule("specific", vec![path("PathPrefix", "/")], vec![])]),
        route("wild", "2026-01-01", &["*.example.com"], vec![rule("wild", vec![path("PathPrefix", "/")], vec![])]),
    ]);
    assert_eq!(Some("specific".to_string()), picked(&l, &request("GET", "/", &[("Host", "foo.example.com")])), "exact hostname first");
    assert_eq!(Some("wild".to_string()), picked(&l, &request("GET", "/", &[("Host", "bar.example.com")])));
    assert_eq!(Some("old".to_string()), picked(&l, &request("GET", "/", &[("Host", "other.org")])), "then the oldest route");
}

#[test]
fn the_most_specific_listener_hostname_wins() {
    let any = listener(None, vec![route("a", "1", &[], vec![rule("any", vec![path("PathPrefix", "/")], vec![])])]);
    let wild = listener(Some("*.bar.com"), vec![route("b", "1", &[], vec![rule("wild", vec![path("PathPrefix", "/")], vec![])])]);
    let exact = listener(Some("foo.bar.com"), vec![route("c", "1", &[], vec![rule("exact", vec![path("PathPrefix", "/")], vec![])])]);
    let all = vec![any, wild, exact];
    let name = |host: &str| pick_listener(&all, host).map(|l| l.name.clone());
    assert_eq!(Some("foo.bar.com".to_string()), name("foo.bar.com"));
    assert_eq!(Some("*.bar.com".to_string()), name("x.bar.com"));
    assert_eq!(Some("any".to_string()), name("elsewhere.org"));
}

#[test]
fn unresolvable_or_empty_backends_answer_500_and_503() {
    let router = PortRouter::new(Arc::new(RwLock::new(Arc::new(vec![]))));
    let connection = ConnectionInfo {
        client: crate::server::Address { ip: "127.0.0.1".to_string(), port: 1 },
        server: crate::server::Address { ip: "127.0.0.1".to_string(), port: 2 },
        request_size: 0,
        sni_hostname: None,
    };
    let r = request("GET", "/", &[]);
    let code = |backends: Vec<Backend>| router.forward(&rule("x", vec![], backends), &r, &connection).status_code;
    assert_eq!(500, code(vec![]), "no backends");
    assert_eq!(500, code(vec![backend(0, Ok(vec!["10.0.0.1:80".to_string()]))]), "all weights 0");
    assert_eq!(500, code(vec![backend(1, Err("not found".to_string()))]));
    assert_eq!(503, code(vec![backend(1, Ok(vec![]))]), "no ready endpoints");
    assert_eq!(404, router.execute(&r, &connection).unwrap().status_code, "no listener");
}

#[test]
fn rule_filters_answer_redirects_preflights_and_unknown_filters_here() {
    use crate::gateway_api::types::{HttpCorsFilter, HttpRequestRedirectFilter, HttpRouteFilter};
    let filtered = |filter: HttpRouteFilter| {
        let mut r = rule("x", vec![path("PathPrefix", "/")], vec![backend(1, Ok(vec![]))]);
        r.filters = vec![filter];
        listener(None, vec![route("r", "1", &[], vec![r])])
    };
    let router = PortRouter::new(Arc::new(RwLock::new(Arc::new(vec![]))));
    let connection = ConnectionInfo {
        client: crate::server::Address { ip: "127.0.0.1".to_string(), port: 1 },
        server: crate::server::Address { ip: "127.0.0.1".to_string(), port: 2 },
        request_size: 0,
        sni_hostname: None,
    };
    let run = |l: &ListenerConfig, r: &Request| {
        let (_, rule, matched) = select(l, r, "example.org").expect("a rule");
        router.route(l, rule, matched, r, "example.org", &connection)
    };
    let redirect = filtered(HttpRouteFilter {
        filter_type: "RequestRedirect".to_string(),
        request_redirect: Some(HttpRequestRedirectFilter { scheme: Some("https".to_string()), ..Default::default() }),
        ..Default::default()
    });
    let response = run(&redirect, &request("GET", "/a?b", &[("host", "example.org")]));
    assert_eq!(302, response.status_code);
    assert_eq!(Some("https://example.org/a?b"), response.headers.iter().find(|h| h.name == "Location").map(|h| h.value.as_ref()));

    let cors = filtered(HttpRouteFilter {
        filter_type: "CORS".to_string(),
        cors: Some(HttpCorsFilter { allow_origins: vec!["*".to_string()], ..Default::default() }),
        ..Default::default()
    });
    let preflight = request("OPTIONS", "/", &[("origin", "https://a.com"), ("access-control-request-method", "GET")]);
    assert_eq!(204, run(&cors, &preflight).status_code, "answered without the backend");
    assert_eq!(503, run(&cors, &request("GET", "/", &[])).status_code, "others reach the (empty) backend");

    let unknown = filtered(HttpRouteFilter { filter_type: "ExtensionRef".to_string(), ..Default::default() });
    assert_eq!(500, run(&unknown, &request("GET", "/", &[])).status_code);
}

/// A backend answering each connection's first request: the first `fail`
/// requests with `code`, the rest with 200 — after `delay`.
fn flaky_backend(fail: usize, code: u16, delay: std::time::Duration) -> (String, Arc<std::sync::atomic::AtomicUsize>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let seen = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = Arc::clone(&seen);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let n = counter.fetch_add(1, Ordering::SeqCst);
            std::thread::spawn(move || {
                let mut buffer = [0u8; 4096];
                let _ = stream.read(&mut buffer);
                std::thread::sleep(delay);
                let code = if n < fail { code } else { 200 };
                let _ = stream.write_all(format!("HTTP/1.1 {code} X\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok").as_bytes());
            });
        }
    });
    (address, seen)
}

fn connection() -> ConnectionInfo {
    ConnectionInfo {
        client: crate::server::Address { ip: "127.0.0.1".to_string(), port: 1 },
        server: crate::server::Address { ip: "127.0.0.1".to_string(), port: 2 },
        request_size: 0,
        sni_hostname: None,
    }
}

fn with_policy(address: &str, timeouts: Option<(&str, &str)>, retry: Option<(i32, Vec<i32>)>) -> CompiledRule {
    use crate::gateway_api::types::{HttpRouteRetry, HttpRouteTimeouts};
    let mut r = rule("x", vec![path("PathPrefix", "/")], vec![backend(1, Ok(vec![address.to_string()]))]);
    r.timeouts = timeouts.map(|(request, backend_request)| HttpRouteTimeouts {
        request: Some(request.to_string()).filter(|s| !s.is_empty()),
        backend_request: Some(backend_request.to_string()).filter(|s| !s.is_empty()),
    });
    r.retry = retry.map(|(attempts, codes)| HttpRouteRetry { attempts: Some(attempts), codes, backoff: Some("1ms".to_string()) });
    r
}

#[test]
fn retries_listed_codes_up_to_attempts() {
    let router = PortRouter::new(Arc::new(RwLock::new(Arc::new(vec![]))));
    let r = request("GET", "/", &[("host", "example.org")]);
    let (address, seen) = flaky_backend(2, 500, std::time::Duration::ZERO);
    assert_eq!(200, router.forward(&with_policy(&address, None, Some((3, vec![500]))), &r, &connection()).status_code);
    assert_eq!(3, seen.load(Ordering::SeqCst), "two retries");

    let (address, seen) = flaky_backend(4, 500, std::time::Duration::ZERO);
    assert_eq!(500, router.forward(&with_policy(&address, None, Some((3, vec![500]))), &r, &connection()).status_code);
    assert_eq!(4, seen.load(Ordering::SeqCst), "the first try and three retries");

    let (address, seen) = flaky_backend(2, 503, std::time::Duration::ZERO);
    assert_eq!(503, router.forward(&with_policy(&address, None, Some((3, vec![500]))), &r, &connection()).status_code, "an unlisted code isn't retried");
    assert_eq!(1, seen.load(Ordering::SeqCst));

    let (address, seen) = flaky_backend(1, 500, std::time::Duration::ZERO);
    assert_eq!(500, router.forward(&with_policy(&address, None, None), &r, &connection()).status_code, "no retry policy");
    assert_eq!(1, seen.load(Ordering::SeqCst));
}

#[test]
fn timeouts_answer_504_and_zero_disables_them() {
    let router = PortRouter::new(Arc::new(RwLock::new(Arc::new(vec![]))));
    let r = request("GET", "/", &[("host", "example.org")]);
    let (slow, _) = flaky_backend(0, 200, std::time::Duration::from_millis(400));
    assert_eq!(504, router.forward(&with_policy(&slow, Some(("100ms", "")), None), &r, &connection()).status_code, "request timeout");
    assert_eq!(504, router.forward(&with_policy(&slow, Some(("", "100ms")), None), &r, &connection()).status_code, "backend timeout");
    assert_eq!(200, router.forward(&with_policy(&slow, Some(("0s", "0s")), None), &r, &connection()).status_code, "0s disables");
    assert_eq!(200, router.forward(&with_policy(&slow, Some(("2s", "")), None), &r, &connection()).status_code);
}

#[test]
fn h2c_backends_get_http2_with_prior_knowledge() {
    use std::io::Read;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let preface = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0u8; 24];
        stream.read_exact(&mut buffer).unwrap();
        buffer.to_vec()
    });
    let router = PortRouter::new(Arc::new(RwLock::new(Arc::new(vec![]))));
    let mut b = backend(1, Ok(vec![address]));
    b.app_protocol = Some("kubernetes.io/h2c".to_string());
    let mut r = rule("x", vec![path("PathPrefix", "/")], vec![b]);
    r.timeouts = Some(crate::gateway_api::types::HttpRouteTimeouts { request: Some("300ms".to_string()), backend_request: None });
    let _ = router.forward(&r, &request("GET", "/", &[("host", "example.org")]), &connection());
    assert_eq!(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n".to_vec(), preface.join().unwrap());
}

#[test]
fn sampling_follows_the_share() {
    let hits = (0..20_000).filter(|_| sample(0.35)).count();
    assert!((6_400..7_600).contains(&hits), "{hits}");
    assert!((0..1000).all(|_| sample(1.0)));
    assert!((0..1000).all(|_| !sample(0.0)));
}

#[test]
fn mirrors_get_a_copy_after_the_request_filters() {
    use crate::gateway_api::routes::Mirror;
    use crate::gateway_api::types::{HttpHeader, HttpHeaderFilter, HttpRouteFilter};
    use std::io::{Read, Write};
    let mirror = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mirror_address = mirror.local_addr().unwrap().to_string();
    let copy = std::thread::spawn(move || {
        let (mut stream, _) = mirror.accept().unwrap();
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap_or(0) == 1 {
            head.push(byte[0]);
        }
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
        String::from_utf8_lossy(&head).to_string()
    });
    let (primary, seen) = flaky_backend(0, 200, std::time::Duration::ZERO);
    let mut r = rule("x", vec![path("PathPrefix", "/")], vec![backend(1, Ok(vec![primary]))]);
    r.filters = vec![
        HttpRouteFilter { filter_type: "RequestMirror".to_string(), request_mirror: Some(Default::default()), ..Default::default() },
        HttpRouteFilter {
            filter_type: "RequestHeaderModifier".to_string(),
            request_header_modifier: Some(HttpHeaderFilter { set: vec![HttpHeader { name: "X-Set".to_string(), value: "yes".to_string() }], ..Default::default() }),
            ..Default::default()
        },
    ];
    r.mirrors = vec![Mirror { endpoints: Ok(vec![mirror_address]), share: 1.0 }];
    let l = listener(None, vec![route("r", "1", &[], vec![r])]);
    let router = PortRouter::new(Arc::new(RwLock::new(Arc::new(vec![]))));
    let req = request("GET", "/m", &[("host", "example.org")]);
    let (_, rule, matched) = select(&l, &req, "example.org").unwrap();
    assert_eq!(200, router.route(&l, rule, matched, &req, "example.org", &connection()).status_code);
    assert_eq!(1, seen.load(Ordering::SeqCst));
    let head = copy.join().unwrap();
    assert!(head.starts_with("GET /m HTTP/1.1") && head.contains("X-Set: yes"), "{head}");
}

#[test]
fn grpc_method_matches_take_the_second_path_segment() {
    assert_eq!(Some("Echo"), grpc_method("/pkg.Service/Echo"));
    assert_eq!(None, grpc_method("/pkg.Service/"));
    assert_eq!(None, grpc_method("/Echo"));
    assert_eq!(None, grpc_method("/a/b/Echo"));
    let l = listener(None, vec![route("r", "1", &[], vec![rule("method", vec![path(crate::gateway_api::routes::GRPC_METHOD_MATCH, "Echo")], vec![])])]);
    assert_eq!(Some("method".to_string()), picked(&l, &request("POST", "/any.Service/Echo", &[])));
    assert_eq!(None, picked(&l, &request("POST", "/any.Service/Other", &[])));
    let unsupported = listener(None, vec![route("r", "1", &[], vec![rule("re", vec![path("rws.Unsupported", "")], vec![])])]);
    assert_eq!(None, picked(&unsupported, &request("POST", "/a/b", &[])));
}

#[test]
fn a_host_of_another_listener_than_the_sni_chose_is_misdirected() {
    let listeners = vec![
        listener(None, vec![]),
        listener(Some("second-example.org"), vec![]),
        listener(Some("*.wildcard.org"), vec![]),
        listener(Some("fourth-example.wildcard.org"), vec![]),
    ];
    let over_tls = |sni: &str| ConnectionInfo { sni_hostname: Some(sni.to_string()), ..connection() };
    let check = |sni: &str, host: &str| {
        let by_host = pick_listener(&listeners, host).unwrap();
        misdirected(&listeners, by_host, &over_tls(sni))
    };
    // The suite's cases (HTTPRouteHTTPSListenerDetectMisdirectedRequests).
    assert!(!check("example.org", "example.org"));
    assert!(check("example.org", "second-example.org"));
    assert!(!check("example.org", "unknown-example.org"));
    assert!(check("second-example.org", "example.org"));
    assert!(check("second-example.org", "unknown-example.org"));
    assert!(!check("third-example.wildcard.org", "fith-example.wildcard.org"));
    assert!(check("third-example.wildcard.org", "fourth-example.wildcard.org"));
    assert!(check("fourth-example.wildcard.org", "fith-example.wildcard.org"));
    assert!(!check("unknown-example.org", "example.org"));
    assert!(!misdirected(&listeners, &listeners[1], &connection()), "plain HTTP has no SNI");
}
