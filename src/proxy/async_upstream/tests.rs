use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::forward;
use crate::request::Request;

fn get(uri: &str) -> Request {
    Request { method: "GET".to_string(), request_uri: uri.to_string(), http_version: "HTTP/1.1".to_string(), headers: vec![], body: vec![] }
}

fn with_method(method: &str) -> Request {
    let mut r = get("/");
    r.method = method.to_string();
    if method == "POST" {
        r.body = b"x".to_vec();
    }
    r
}

/// Answers every request on a connection with `response`; closes a
/// connection after `per_conn` responses. Returns (port, connections).
fn backend(response: &'static [u8], per_conn: usize) -> (u16, Arc<AtomicU32>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let accepted = Arc::new(AtomicU32::new(0));
    let counter = Arc::clone(&accepted);
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            counter.fetch_add(1, Ordering::SeqCst);
            thread::spawn(move || {
                let mut stream = stream;
                let mut buf = [0u8; 4096];
                for _ in 0..per_conn {
                    let mut head = Vec::new();
                    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                        match stream.read(&mut buf) {
                            Ok(0) | Err(_) => return,
                            Ok(n) => head.extend_from_slice(&buf[..n]),
                        }
                    }
                    if stream.write_all(response).is_err() {
                        return;
                    }
                }
            });
        }
    });
    (port, accepted)
}

const OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\nConnection: keep-alive\r\n\r\nhello";

fn body(response: &crate::response::Response) -> Vec<u8> {
    response.content_range_list.iter().flat_map(|r| r.body.clone()).collect()
}

async fn fwd(port: u16, request: &Request) -> Result<crate::response::Response, String> {
    forward(request, "10.0.0.1", "127.0.0.1", port, None, Duration::from_secs(2), Duration::from_secs(2)).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn keep_alive_connections_are_reused() {
    let (port, accepted) = backend(OK, 100);
    for _ in 0..5 {
        let response = fwd(port, &get("/")).await.unwrap();
        assert_eq!(200, response.status_code);
        assert_eq!(b"hello".to_vec(), body(&response));
    }
    assert_eq!(1, accepted.load(Ordering::SeqCst), "5 requests, 1 connection");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stale_connection_is_retried_for_get_but_not_post() {
    let (port, accepted) = backend(OK, 1);
    assert_eq!(200, fwd(port, &get("/")).await.unwrap().status_code);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(200, fwd(port, &get("/")).await.unwrap().status_code, "stale pooled connection retried");
    assert_eq!(2, accepted.load(Ordering::SeqCst));
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(fwd(port, &with_method("POST")).await.is_err(), "a POST may have been processed: no replay");
    assert_eq!(2, accepted.load(Ordering::SeqCst));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn head_responses_have_no_body_to_wait_for() {
    // Content-Length describes the GET body; a HEAD response carries none.
    const HEAD: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n";
    let (port, _) = backend(HEAD, 100);
    let response = fwd(port, &with_method("HEAD")).await.unwrap();
    assert_eq!(200, response.status_code);
    assert!(body(&response).is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chunked_responses_are_streamed() {
    const CHUNKED: &[u8] = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n";
    let (port, _) = backend(CHUNKED, 1);
    let mut response = fwd(port, &get("/")).await.unwrap();
    let mut pipe = response.stream_pipe.take().expect("streamed");
    let raw = tokio::task::spawn_blocking(move || {
        let mut raw = Vec::new();
        let _ = pipe.read_to_end(&mut raw);
        raw
    })
    .await
    .unwrap();
    assert!(raw.starts_with(b"5\r\nhello\r\n0\r\n"), "{:?}", String::from_utf8_lossy(&raw));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unreachable_backend_is_an_error() {
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port(); // closed again
    assert!(fwd(port, &get("/")).await.is_err());
}

// ── which requests take the async path ────────────────────────────────────────

fn conn() -> crate::server::ConnectionInfo {
    crate::server::ConnectionInfo {
        client: crate::server::Address { ip: "127.0.0.1".to_string(), port: 1 },
        server: crate::server::Address { ip: "127.0.0.1".to_string(), port: 2 },
        request_size: 1024,
        sni_hostname: None,
    }
}

fn proxy_app(extra: &str, backend: &str) -> crate::proxy_config::ConfigDrivenApp {
    let config = crate::proxy_config::ProxyConfig::from_str(&format!(r#"
[[route]]
  [route.match]
  path = "/api/*"
  [route.action]
  type     = "proxy"
  upstream = "{backend}"
{extra}
"#));
    crate::proxy_config::builder::build(config).0
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plain_proxy_routes_are_served_async() {
    use crate::application::Application;
    let (port, _) = backend(OK, 100);
    let app = proxy_app("", &format!("127.0.0.1:{port}"));
    let (request, connection) = (get("/api/x"), conn());
    let future = app.execute_async(&request, &connection).expect("async path");
    let response = future.await.unwrap();
    assert_eq!(b"hello".to_vec(), body(&response));
    assert!(app.execute_async(&get("/other"), &connection).is_none(), "unmatched: fallback app");
}

#[test]
fn routes_with_middleware_keep_the_blocking_path_tls_routes_do_not() {
    use crate::application::Application;
    let with_mw = proxy_app("  [route.middleware]\n  rate_limit = { max_requests = 5 }", "127.0.0.1:9");
    assert!(with_mw.execute_async(&get("/api/x"), &conn()).is_none());
    let tls = proxy_app("", "https://127.0.0.1:9");
    assert!(tls.execute_async(&get("/api/x"), &conn()).is_some());
}

#[test]
fn middleware_stacks_offer_async_only_when_every_layer_passes_through() {
    use crate::application::Application;
    use crate::middleware::{Middleware, WithMiddleware};
    struct Passthrough;
    impl Middleware for Passthrough {
        fn handle(&self, r: &Request, c: &crate::server::ConnectionInfo, next: &dyn Application) -> Result<crate::response::Response, String> {
            next.execute(r, c)
        }
        fn is_passthrough(&self, _: &Request, _: &crate::server::ConnectionInfo) -> bool {
            true
        }
    }
    let app = proxy_app("", "127.0.0.1:9");
    let passing = WithMiddleware::new(app.clone()).wrap(Passthrough);
    assert!(passing.execute_async(&get("/api/x"), &conn()).is_some());
    let limited = WithMiddleware::new(app).wrap(crate::middleware::RateLimitLayer);
    assert!(limited.execute_async(&get("/api/x"), &conn()).is_none());
}


// ── HTTPS backends on the async path ──────────────────────────────────────────

fn tls_to(config: Arc<rustls::ClientConfig>, pool: &'static crate::proxy::Pool<super::AsyncTlsStream>) -> super::AsyncTls {
    super::AsyncTls { config, pool }
}

fn fresh_tls_pool() -> &'static crate::proxy::Pool<super::AsyncTlsStream> {
    Box::leak(Box::new(crate::proxy::Pool::new(8, Duration::from_secs(60))))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tls_connections_are_pooled() {
    let (port, config, accepted, _) = crate::proxy::tests::tls_backend_with_pem(OK, 100);
    let pool = fresh_tls_pool();
    for _ in 0..5 {
        let response = forward(&get("/"), "10.0.0.1", "localhost", port, Some(tls_to(Arc::clone(&config), pool)), Duration::from_secs(2), Duration::from_secs(2))
            .await
            .unwrap();
        assert_eq!(b"hello".to_vec(), body(&response));
    }
    assert_eq!(1, accepted.load(Ordering::SeqCst), "5 HTTPS requests, 1 handshake");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_chunked_tls_response_is_streamed_through_the_channel() {
    const CHUNKED: &[u8] = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n";
    let (port, config, _, _) = crate::proxy::tests::tls_backend_with_pem(CHUNKED, 1);
    let mut response = forward(&get("/"), "10.0.0.1", "localhost", port, Some(tls_to(config, fresh_tls_pool())), Duration::from_secs(2), Duration::from_secs(2))
        .await
        .unwrap();
    let mut pipe = response.stream_pipe.take().expect("streamed");
    let raw = tokio::task::spawn_blocking(move || {
        let mut raw = Vec::new();
        let _ = pipe.read_to_end(&mut raw);
        raw
    })
    .await
    .unwrap();
    assert!(raw.starts_with(b"5\r\nhello\r\n0\r\n"), "{:?}", String::from_utf8_lossy(&raw));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_untrusted_tls_backend_is_an_error() {
    // Self-signed, checked against the WebPKI roots.
    let (port, _, _, _) = crate::proxy::tests::tls_backend_with_pem(OK, 1);
    let webpki = tls_to(crate::proxy::tls_client_config(), fresh_tls_pool());
    assert!(forward(&get("/"), "10.0.0.1", "localhost", port, Some(webpki), Duration::from_secs(2), Duration::from_secs(2)).await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_tls_ca_file_route_is_served_async() {
    use crate::application::Application;
    let (port, _, _, pem) = crate::proxy::tests::tls_backend_with_pem(OK, 100);
    let ca = crate::test_env::TempPath::new("async_private_ca.pem");
    std::fs::write(ca.as_str(), pem).unwrap();
    let config = crate::proxy_config::ProxyConfig::from_str(&format!(r#"
[[upstream]]
name        = "internal"
backends    = ["https://localhost:{port}"]
tls_ca_file = "{}"

[[route]]
  [route.match]
  path = "/api/*"
  [route.action]
  type     = "proxy"
  upstream = "internal"
"#, ca.as_str()));
    let app = crate::proxy_config::builder::build(config).0;
    let (request, connection) = (get("/api/x"), conn());
    let response = app.execute_async(&request, &connection).expect("async path").await.unwrap();
    assert_eq!(b"hello".to_vec(), body(&response));
}
