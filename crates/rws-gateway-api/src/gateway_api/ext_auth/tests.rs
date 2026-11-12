use super::*;
use crate::gateway_api::types::{BackendObjectReference, ForwardBodyConfig, GrpcAuthConfig, HttpAuthConfig};

fn request(uri: &str, headers: &[(&str, &str)], body: &[u8]) -> Request {
    Request {
        method: "POST".to_string(),
        request_uri: uri.to_string(),
        http_version: "HTTP/1.1".to_string(),
        headers: headers.iter().map(|(n, v)| Header::new(n.to_string(), v.to_string())).collect(),
        body: body.to_vec().into(),
    }
}

fn http_filter(path: &str, allowed: &[&str], response_headers: &[&str], max_body: i32) -> HttpExternalAuthFilter {
    HttpExternalAuthFilter {
        protocol: "HTTP".to_string(),
        backend_ref: BackendObjectReference { name: "auth".to_string(), port: Some(80), ..Default::default() },
        http: Some(HttpAuthConfig {
            path: path.to_string(),
            allowed_headers: allowed.iter().map(|s| s.to_string()).collect(),
            allowed_response_headers: response_headers.iter().map(|s| s.to_string()).collect(),
        }),
        forward_body: Some(ForwardBodyConfig { max_size: max_body }),
        grpc: None,
    }
}

fn header<'a>(r: &'a Request, name: &str) -> Option<&'a str> {
    r.headers.iter().find(|h| h.name.eq_ignore_ascii_case(name)).map(|h| h.value.as_ref())
}

#[test]
fn the_http_authorization_request_carries_what_the_filter_allows() {
    let r = request("/orders?id=1", &[("Host", "shop.example"), ("Authorization", "Bearer t"), ("X-Tenant", "a"), ("Cookie", "c")], b"0123456789");
    let none = http_request(&http_filter("/check/", &[], &[], 0), &r);
    assert_eq!("/check/orders?id=1", none.request_uri, "the prefix, then the client's target");
    assert_eq!("POST", none.method);
    assert_eq!(Some("shop.example"), header(&none, "host"));
    assert_eq!(Some("Bearer t"), header(&none, "authorization"));
    assert_eq!(None, header(&none, "x-tenant"), "only the listed extra headers");
    assert_eq!(Some("0"), header(&none, "content-length"), "no body forwarded");
    let some = http_request(&http_filter("", &["x-tenant"], &[], 4), &r);
    assert_eq!(Some("a"), header(&some, "x-tenant"));
    assert_eq!(None, header(&some, "cookie"));
    assert_eq!(b"0123".to_vec(), some.body.to_vec(), "truncated to maxSize");
    assert_eq!(Some("4"), header(&some, "content-length"));
}

#[test]
fn an_http_answer_allows_with_its_headers_or_is_the_answer() {
    let mut ok = Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "");
    ok.headers.push(Header::new("X-User", "ann"));
    ok.headers.push(Header::new("Host", "evil"));
    ok.headers.push(Header::new("X-Other", "o"));
    match http_decision(&http_filter("", &[], &[], 0), ok) {
        Decision::Allow { set, .. } => {
            assert!(set.contains(&("X-User".to_string(), "ann".to_string())));
            assert!(set.contains(&("X-Other".to_string(), "o".to_string())));
            assert!(!set.iter().any(|(n, _)| n.eq_ignore_ascii_case("host") || n.eq_ignore_ascii_case("content-length")));
        }
        other => panic!("{other:?}"),
    }
    let mut ok = Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "");
    ok.headers.push(Header::new("X-User", "ann"));
    ok.headers.push(Header::new("X-Other", "o"));
    match http_decision(&http_filter("", &[], &["x-user"], 0), ok) {
        Decision::Allow { set, .. } => assert_eq!(vec![("X-User".to_string(), "ann".to_string())], set),
        other => panic!("{other:?}"),
    }
    let denied = Response::text(STATUS_CODE_REASON_PHRASE.n401_unauthorized, "who are you");
    assert!(matches!(http_decision(&http_filter("", &[], &[], 0), denied), Decision::Deny(r) if r.status_code == 401));
}

#[test]
fn check_requests_and_responses_round_trip() {
    let headers = vec![("authorization".to_string(), "Bearer t".to_string()), ("x-tenant".to_string(), "a".to_string())];
    let message = proto::check_request(&proto::HttpAttributes { method: "GET", path: "/a?b=1", host: "shop.example", scheme: "http", headers: &headers, body: b"", source_ip: "10.0.0.9" });
    let (method, path, host, read) = proto::decode_check_request(&message).unwrap();
    assert_eq!(("GET", "/a?b=1", "shop.example"), (method.as_str(), path.as_str(), host.as_str()));
    assert_eq!(headers, read);

    let allow = proto::Verdict { code: 0, headers: vec![("x-user".to_string(), "ann".to_string())], headers_to_remove: vec!["cookie".to_string()], ..Default::default() };
    assert_eq!(allow, proto::check_response(&proto::encode_check_response(&allow)).unwrap());
    let deny = proto::Verdict { code: 7, headers: vec![("www-authenticate".to_string(), "Bearer".to_string())], denied_status: 401, denied_body: "no".to_string(), ..Default::default() };
    assert_eq!(deny, proto::check_response(&proto::encode_check_response(&deny)).unwrap());
    assert!(proto::check_response(&[0x0a, 0xff]).is_err(), "truncated");
    assert!(proto::check_response(&[0x0b]).is_err(), "an unknown wire type");
}

#[test]
fn a_grpc_denial_answers_with_its_status_or_403() {
    match grpc_decision(proto::Verdict { code: 7, denied_status: 401, denied_body: "no".to_string(), headers: vec![("www-authenticate".to_string(), "Bearer".to_string())], ..Default::default() }) {
        Decision::Deny(r) => {
            assert_eq!(401, r.status_code);
            assert!(r.headers.iter().any(|h| h.name == "www-authenticate"));
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(grpc_decision(proto::Verdict { code: 16, ..Default::default() }), Decision::Deny(r) if r.status_code == 403));
    assert!(matches!(grpc_decision(proto::Verdict::default()), Decision::Allow { .. }));
}

/// An HTTP authorization server: `Authorization: Bearer ok` passes (with
/// `X-User: ann` and an attempt to change `Host`), anything else is `401`.
fn http_auth_server() -> String {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut stream = stream;
                let mut buffer = [0u8; 4096];
                let n = stream.read(&mut buffer).unwrap_or(0);
                let head = String::from_utf8_lossy(&buffer[..n]).to_ascii_lowercase();
                let reply = if head.contains("authorization: bearer ok") {
                    "HTTP/1.1 200 OK\r\nX-User: ann\r\nHost: evil.example\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
                } else {
                    "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Bearer\r\nContent-Length: 2\r\nConnection: close\r\n\r\nno".to_string()
                };
                let _ = stream.write_all(reply.as_bytes());
            });
        }
    });
    address
}

fn compiled(filter: HttpExternalAuthFilter, endpoint: &str) -> CompiledAuth {
    CompiledAuth { filter, endpoints: Ok(vec![endpoint.to_string()]), tls: None }
}

fn peer() -> crate::proxy::Peer<'static> {
    crate::proxy::Peer { ip: "10.0.0.9", tls: false }
}

#[test]
fn http_filters_allow_deny_and_never_change_the_host() {
    let server = http_auth_server();
    let counter = AtomicUsize::new(0);
    let filters = vec![compiled(http_filter("", &[], &[], 0), &server)];
    let allowed = run(&filters, &request("/", &[("Host", "shop.example"), ("Authorization", "Bearer ok")], b""), peer(), &counter).unwrap().unwrap();
    assert_eq!(Some("ann"), header(&allowed, "x-user"), "the server's header reaches the route");
    assert_eq!(Some("shop.example"), header(&allowed, "host"), "Host can't be changed");
    let denied = run(&filters, &request("/", &[("Host", "shop.example")], b""), peer(), &counter).unwrap_err();
    assert_eq!(401, denied.status_code, "the server's answer");
    // Unreachable: denied, never let through.
    let gone = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().to_string()
    };
    let unreachable = run(&[compiled(http_filter("", &[], &[], 0), &gone)], &request("/", &[("Authorization", "Bearer ok")], b""), peer(), &counter).unwrap_err();
    assert_eq!(403, unreachable.status_code);
    let unresolved = CompiledAuth { filter: http_filter("", &[], &[], 0), endpoints: Err("no such Service".to_string()), tls: None };
    assert_eq!(403, run(&[unresolved], &request("/", &[], b""), peer(), &counter).unwrap_err().status_code);
    // In order: a denial stops the rest.
    let two = vec![compiled(http_filter("", &[], &[], 0), &server), compiled(http_filter("", &[], &[], 0), &gone)];
    assert_eq!(401, run(&two, &request("/", &[], b""), peer(), &counter).unwrap_err().status_code, "the first one's answer");
}

/// An ext_authz gRPC server over h2c: `authorization: Bearer ok` is
/// allowed (adding `x-user`, removing `cookie`), anything else denied 401.
async fn grpc_auth_server() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    tokio::spawn(async move {
        loop {
            let Ok((tcp, _)) = listener.accept().await else { return };
            tokio::spawn(async move {
                let Ok(mut connection) = h2::server::handshake(tcp).await else { return };
                while let Some(Ok((request, mut respond))) = connection.accept().await {
                    tokio::spawn(async move {
                        assert_eq!("/envoy.service.auth.v3.Authorization/Check", request.uri().path());
                        let mut body = request.into_body();
                        let mut message = Vec::new();
                        while let Some(Ok(chunk)) = body.data().await {
                            let _ = body.flow_control().release_capacity(chunk.len());
                            message.extend_from_slice(&chunk);
                        }
                        let (_, _, _, headers) = proto::decode_check_request(&message[5..]).unwrap();
                        let ok = headers.iter().any(|(k, v)| k == "authorization" && v == "Bearer ok");
                        let verdict = if ok {
                            proto::Verdict { code: 0, headers: vec![("x-user".to_string(), "ann".to_string())], headers_to_remove: vec!["cookie".to_string()], ..Default::default() }
                        } else {
                            proto::Verdict { code: 16, denied_status: 401, denied_body: "no".to_string(), ..Default::default() }
                        };
                        let encoded = proto::encode_check_response(&verdict);
                        let mut frame = vec![0u8];
                        frame.extend_from_slice(&(encoded.len() as u32).to_be_bytes());
                        frame.extend_from_slice(&encoded);
                        let response = http::Response::builder().status(200).header("content-type", "application/grpc").body(()).unwrap();
                        let mut send = respond.send_response(response, false).unwrap();
                        send.send_data(bytes::Bytes::from(frame), false).unwrap();
                        let mut trailers = http::HeaderMap::new();
                        trailers.insert("grpc-status", "0".parse().unwrap());
                        send.send_trailers(trailers).unwrap();
                    });
                }
            });
        }
    });
    address
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grpc_filters_ask_ext_authz() {
    let server = grpc_auth_server().await;
    let filter = HttpExternalAuthFilter { protocol: "GRPC".to_string(), grpc: Some(GrpcAuthConfig::default()), ..http_filter("", &[], &[], 0) };
    let filters = vec![compiled(filter, &server)];
    let (allowed, denied) = tokio::task::spawn_blocking(move || {
        let counter = AtomicUsize::new(0);
        let allowed = run(&filters, &request("/a", &[("Host", "shop.example"), ("Authorization", "Bearer ok"), ("Cookie", "c")], b""), peer(), &counter);
        let denied = run(&filters, &request("/a", &[("Host", "shop.example")], b""), peer(), &counter);
        (allowed, denied)
    })
    .await
    .unwrap();
    let allowed = allowed.unwrap().unwrap();
    assert_eq!(Some("ann"), header(&allowed, "x-user"));
    assert_eq!(None, header(&allowed, "cookie"), "removed as the server asked");
    let denied = denied.unwrap_err();
    assert_eq!(401, denied.status_code);
}

#[test]
fn check_response_decoding_never_panics_on_damaged_input() {
    // A deterministic stand-in for a fuzz target (the fuzz crate doesn't
    // build the gateway-api feature): truncations and byte flips of real
    // answers, and noise.
    let samples = [
        proto::encode_check_response(&proto::Verdict { code: 0, headers: vec![("x-user".to_string(), "ann".to_string())], headers_to_remove: vec!["cookie".to_string()], ..Default::default() }),
        proto::encode_check_response(&proto::Verdict { code: 7, denied_status: 401, denied_body: "no".to_string(), headers: vec![("a".to_string(), "b".to_string())], ..Default::default() }),
    ];
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for sample in &samples {
        for cut in 0..=sample.len() {
            let _ = proto::check_response(&sample[..cut]);
        }
        for _ in 0..2000 {
            let mut damaged = sample.clone();
            let at = (next() as usize) % damaged.len();
            damaged[at] = next() as u8;
            let _ = proto::check_response(&damaged);
        }
    }
    for _ in 0..2000 {
        let noise: Vec<u8> = (0..(next() % 64)).map(|_| next() as u8).collect();
        let _ = proto::check_response(&noise);
    }
}
