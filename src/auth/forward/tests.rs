use crate::application::Application;
use crate::auth::forward::ForwardAuthLayer;
use crate::core::New;
use crate::header::Header;
use crate::http::VERSION;
use crate::middleware::Middleware;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::{Address, ConnectionInfo};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn conn() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 16000,
        sni_hostname: None,
    }
}

fn get(uri: &str) -> Request {
    Request {
        method: METHOD.get.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    }
}

fn with_header(mut req: Request, name: &str, value: &str) -> Request {
    req.headers.push(Header { name: name.to_string(), value: value.to_string() });
    req
}

/// Application that always returns 200 with no body — used when the test
/// only cares whether the request was allowed through, not what reached it.
struct OkApp;
impl Application for OkApp {
    fn execute(&self, _: &Request, _: &ConnectionInfo) -> Result<Response, String> {
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        Ok(r)
    }
}

/// Application that reports, via response headers, what it actually
/// received for a given request header name — lets tests assert on the
/// *forwarded* request without needing a real downstream handler.
struct EchoHeaderApp(&'static str);
impl Application for EchoHeaderApp {
    fn execute(&self, request: &Request, _: &ConnectionInfo) -> Result<Response, String> {
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();

        let count = request.headers.iter().filter(|h| h.name.eq_ignore_ascii_case(self.0)).count();
        r.headers.push(Header { name: "X-Echo-Count".to_string(), value: count.to_string() });
        if let Some(h) = request.headers.iter().find(|h| h.name.eq_ignore_ascii_case(self.0)) {
            r.headers.push(Header { name: "X-Echo".to_string(), value: h.value.clone() });
        }
        Ok(r)
    }
}

/// Spawns a one-shot TCP server that accepts a single connection, discards
/// the request, and writes back `raw_response` verbatim. Returns the bound
/// port. Mirrors the mock-backend pattern used in `proxy_config::tests`.
fn spawn_mock_auth_server(raw_response: String) -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind mock auth server");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(raw_response.as_bytes());
        }
    });
    port
}

fn http_response(status: u16, reason: &str, headers: &[(&str, &str)], body: &str) -> String {
    let mut extra_headers = String::new();
    for (name, value) in headers {
        extra_headers.push_str(&format!("{}: {}\r\n", name, value));
    }
    format!(
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\n{}\r\n{}",
        status,
        reason,
        body.len(),
        extra_headers,
        body
    )
}

/// Binds a listener to obtain a free, currently-unused port, then drops it —
/// connecting to this port afterwards fails with "connection refused",
/// simulating an unreachable auth service without relying on privileged or
/// reserved port numbers.
fn unreachable_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    listener.local_addr().unwrap().port()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn allows_request_through_on_2xx_with_no_copy_header_configured() {
    let port = spawn_mock_auth_server(http_response(200, "OK", &[], ""));
    let layer = ForwardAuthLayer::new(format!("http://127.0.0.1:{}/verify", port));
    let resp = layer.handle(&get("/"), &conn(), &OkApp).unwrap();
    assert_eq!(200, resp.status_code);
}

#[test]
fn copies_header_from_auth_response_onto_forwarded_request() {
    let port = spawn_mock_auth_server(http_response(200, "OK", &[("X-User-Id", "alice")], ""));
    let layer = ForwardAuthLayer::new(format!("http://127.0.0.1:{}/verify", port)).copy_header("X-User-Id");
    let resp = layer.handle(&get("/"), &conn(), &EchoHeaderApp("X-User-Id")).unwrap();
    assert_eq!(200, resp.status_code);
    let echoed = resp.headers.iter().find(|h| h.name == "X-Echo").map(|h| h.value.as_str());
    assert_eq!(Some("alice"), echoed);
}

#[test]
fn copied_header_replaces_client_forged_header_not_append() {
    // Security case: the client already sent its own X-User-Id. The auth
    // service's verified value must fully replace it — not sit alongside it,
    // which could let the forged value win depending on lookup order.
    let port = spawn_mock_auth_server(http_response(200, "OK", &[("X-User-Id", "alice")], ""));
    let layer = ForwardAuthLayer::new(format!("http://127.0.0.1:{}/verify", port)).copy_header("X-User-Id");
    let req = with_header(get("/"), "X-User-Id", "mallory");
    let resp = layer.handle(&req, &conn(), &EchoHeaderApp("X-User-Id")).unwrap();

    let count = resp.headers.iter().find(|h| h.name == "X-Echo-Count").map(|h| h.value.as_str());
    assert_eq!(Some("1"), count, "expected exactly one X-User-Id header on the forwarded request");
    let echoed = resp.headers.iter().find(|h| h.name == "X-Echo").map(|h| h.value.as_str());
    assert_eq!(Some("alice"), echoed, "trusted auth-service value must win over the client-forged one");
}

#[test]
fn copy_header_absent_from_auth_response_leaves_original_untouched() {
    let port = spawn_mock_auth_server(http_response(200, "OK", &[], ""));
    let layer = ForwardAuthLayer::new(format!("http://127.0.0.1:{}/verify", port)).copy_header("X-User-Id");
    let req = with_header(get("/"), "X-User-Id", "original");
    let resp = layer.handle(&req, &conn(), &EchoHeaderApp("X-User-Id")).unwrap();
    let echoed = resp.headers.iter().find(|h| h.name == "X-Echo").map(|h| h.value.as_str());
    assert_eq!(Some("original"), echoed);
}

#[test]
fn copies_multiple_headers() {
    let port = spawn_mock_auth_server(http_response(
        200,
        "OK",
        &[("X-User-Id", "alice"), ("X-Roles", "admin,editor")],
        "",
    ));
    let layer = ForwardAuthLayer::new(format!("http://127.0.0.1:{}/verify", port))
        .copy_header("X-User-Id")
        .copy_header("X-Roles");
    let resp = layer.handle(&get("/"), &conn(), &EchoHeaderApp("X-Roles")).unwrap();
    let echoed = resp.headers.iter().find(|h| h.name == "X-Echo").map(|h| h.value.as_str());
    assert_eq!(Some("admin,editor"), echoed);
}

#[test]
fn non_2xx_auth_response_is_returned_verbatim() {
    let port = spawn_mock_auth_server(http_response(
        401,
        "Unauthorized",
        &[("WWW-Authenticate", r#"Basic realm="auth""#)],
        "denied",
    ));
    let layer = ForwardAuthLayer::new(format!("http://127.0.0.1:{}/verify", port));
    let resp = layer.handle(&get("/"), &conn(), &OkApp).unwrap();

    assert_eq!(401, resp.status_code);
    let challenge = resp.headers.iter().find(|h| h.name.eq_ignore_ascii_case("WWW-Authenticate"));
    assert!(challenge.is_some(), "expected WWW-Authenticate to be preserved");
    let body: Vec<u8> = resp.content_range_list.iter().flat_map(|c| c.body.iter().copied()).collect();
    assert_eq!(b"denied".to_vec(), body);
}

#[test]
fn redirect_auth_response_preserves_location_header() {
    let port = spawn_mock_auth_server(http_response(
        302,
        "Found",
        &[("Location", "https://sso.example.com/login")],
        "",
    ));
    let layer = ForwardAuthLayer::new(format!("http://127.0.0.1:{}/verify", port));
    let resp = layer.handle(&get("/"), &conn(), &OkApp).unwrap();

    assert_eq!(302, resp.status_code);
    let location = resp.headers.iter().find(|h| h.name.eq_ignore_ascii_case("Location")).map(|h| h.value.as_str());
    assert_eq!(Some("https://sso.example.com/login"), location);
}

#[test]
fn non_2xx_response_excludes_hop_by_hop_and_framing_headers() {
    let port = spawn_mock_auth_server(http_response(
        403,
        "Forbidden",
        &[("Connection", "close"), ("Content-Type", "text/plain")],
        "nope",
    ));
    let layer = ForwardAuthLayer::new(format!("http://127.0.0.1:{}/verify", port));
    let resp = layer.handle(&get("/"), &conn(), &OkApp).unwrap();

    assert_eq!(403, resp.status_code);
    assert!(resp.headers.iter().all(|h| !h.name.eq_ignore_ascii_case("connection")));
    assert!(resp.headers.iter().all(|h| !h.name.eq_ignore_ascii_case("content-length")));
    // Content-Type is instead derived from content_range_list, not duplicated in headers.
    assert!(resp.headers.iter().all(|h| !h.name.eq_ignore_ascii_case("content-type")));
}

#[test]
fn unreachable_auth_service_returns_502() {
    let port = unreachable_port();
    let layer = ForwardAuthLayer::new(format!("http://127.0.0.1:{}/verify", port)).timeout_ms(500);
    let resp = layer.handle(&get("/"), &conn(), &OkApp).unwrap();
    assert_eq!(502, resp.status_code);
}

#[test]
fn full_request_headers_are_forwarded_to_auth_service() {
    // The auth service needs to see the client's original headers (e.g. a
    // session cookie) to make its decision — confirm they're present on the
    // outbound GET by having the mock auth server echo whether it saw one.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).unwrap();
        let request_text = String::from_utf8_lossy(&buf[..n]);
        let saw_cookie = request_text.to_lowercase().contains("cookie: session=abc123");
        let _ = stream.write_all(http_response(200, "OK", &[], "").as_bytes());
        saw_cookie
    });

    let layer = ForwardAuthLayer::new(format!("http://127.0.0.1:{}/verify", port));
    let req = with_header(get("/"), "Cookie", "session=abc123");
    let resp = layer.handle(&req, &conn(), &OkApp).unwrap();

    assert_eq!(200, resp.status_code);
    assert!(handle.join().unwrap(), "expected the auth service to receive the client's Cookie header");
}
