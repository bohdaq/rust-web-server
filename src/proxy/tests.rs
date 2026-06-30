use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use crate::app::App;
use crate::application::Application;
use crate::core::New;
use crate::http::VERSION;
use crate::middleware::Middleware;
use crate::proxy::{LoadBalancing, ReverseProxy};
use crate::request::{METHOD, Request};
use crate::server::{Address, ConnectionInfo};

// ── test helpers ──────────────────────────────────────────────────────────────

fn conn() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "10.0.0.1".to_string(), port: 1234 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 16000,
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

/// Spawns a mock HTTP/1.1 backend that reads one request and sends `response`.
/// Returns the port the backend is listening on.
fn mock_backend(response: &'static str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock backend");
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(response.as_bytes());
        }
    });
    port
}

/// Backend that accepts a connection and immediately closes without sending anything.
fn silent_backend() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind silent backend");
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            drop(stream);
        }
    });
    port
}

/// Backend that stays listening but never accepts; combined with a very short
/// connect timeout this simulates an unreachable backend.
fn refusing_port() -> u16 {
    // A port that immediately refuses connections (no listener).
    // Bind, extract the port, then drop the listener so the OS rejects incoming
    // SYNs immediately (on most platforms) rather than queuing them.
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    drop(l);
    port
}

const OK_RESPONSE: &str =
    "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello";
const NOT_FOUND_RESPONSE: &str =
    "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: close\r\n\r\nnot found";

// ── Backend parsing ───────────────────────────────────────────────────────────

#[test]
fn proxy_with_no_backends_returns_502() {
    let proxy = ReverseProxy::new(std::iter::empty::<String>());
    let resp = proxy.handle(&get("/"), &conn(), &App::new()).unwrap();
    assert_eq!(502, resp.status_code);
}

// ── Basic forwarding ──────────────────────────────────────────────────────────

#[test]
fn proxy_forwards_and_returns_200() {
    let port = mock_backend(OK_RESPONSE);
    let proxy = ReverseProxy::new([format!("http://127.0.0.1:{}", port)]);
    let resp = proxy.handle(&get("/"), &conn(), &App::new()).unwrap();
    assert_eq!(200, resp.status_code);
}

#[test]
fn proxy_returns_upstream_status_code() {
    let port = mock_backend(NOT_FOUND_RESPONSE);
    let proxy = ReverseProxy::new([format!("http://127.0.0.1:{}", port)]);
    let resp = proxy.handle(&get("/missing"), &conn(), &App::new()).unwrap();
    assert_eq!(404, resp.status_code);
}

#[test]
fn proxy_preserves_upstream_body() {
    let port = mock_backend(OK_RESPONSE);
    let proxy = ReverseProxy::new([format!("http://127.0.0.1:{}", port)]);
    let resp = proxy.handle(&get("/"), &conn(), &App::new()).unwrap();
    assert_eq!(200, resp.status_code);
    let body: Vec<u8> = resp
        .content_range_list
        .iter()
        .flat_map(|cr| cr.body.iter().copied())
        .collect();
    assert_eq!(b"hello", body.as_slice());
}

// ── 502 on backend failure ────────────────────────────────────────────────────

#[test]
fn proxy_returns_502_when_backend_silently_closes() {
    let port = silent_backend();
    // Give the thread a moment to accept
    thread::sleep(Duration::from_millis(20));
    let proxy = ReverseProxy::new([format!("http://127.0.0.1:{}", port)]);
    let resp = proxy.handle(&get("/"), &conn(), &App::new()).unwrap();
    assert_eq!(502, resp.status_code);
}

#[test]
fn proxy_returns_502_when_all_backends_refuse() {
    let port = refusing_port();
    let proxy = ReverseProxy::new([format!("http://127.0.0.1:{}", port)])
        .connect_timeout_ms(100);
    let resp = proxy.handle(&get("/"), &conn(), &App::new()).unwrap();
    assert_eq!(502, resp.status_code);
}

// ── Failover ──────────────────────────────────────────────────────────────────

#[test]
fn proxy_fails_over_to_second_backend_when_first_is_down() {
    let good_port = mock_backend(OK_RESPONSE);
    let bad_port = refusing_port();
    let proxy = ReverseProxy::new([
        format!("http://127.0.0.1:{}", bad_port),
        format!("http://127.0.0.1:{}", good_port),
    ])
    .connect_timeout_ms(200);
    let resp = proxy.handle(&get("/"), &conn(), &App::new()).unwrap();
    assert_eq!(200, resp.status_code);
}

// ── Round-robin ───────────────────────────────────────────────────────────────

#[test]
fn round_robin_distributes_across_backends() {
    fn count_backend(resp_body: &'static str) -> (u16, std::sync::Arc<std::sync::atomic::AtomicU32>) {
        use std::sync::Arc;
        use std::sync::atomic::AtomicU32;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let count = Arc::new(AtomicU32::new(0));
        let count_clone = Arc::clone(&count);
        let body = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", resp_body.len(), resp_body);
        thread::spawn(move || {
            for _ in 0..4 {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buf = [0u8; 4096];
                    let _ = stream.read(&mut buf);
                    let _ = stream.write_all(body.as_bytes());
                    count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
        });
        (port, count)
    }

    let (port_a, count_a) = count_backend("from-a");
    let (port_b, count_b) = count_backend("from-b");

    let proxy = ReverseProxy::new([
        format!("http://127.0.0.1:{}", port_a),
        format!("http://127.0.0.1:{}", port_b),
    ])
    .strategy(LoadBalancing::RoundRobin);

    for _ in 0..4 {
        let _ = proxy.handle(&get("/"), &conn(), &App::new());
    }

    thread::sleep(Duration::from_millis(50));

    let a = count_a.load(std::sync::atomic::Ordering::Relaxed);
    let b = count_b.load(std::sync::atomic::Ordering::Relaxed);
    assert_eq!(4, a + b, "total requests should be 4");
    assert!(a >= 1, "backend A should have received at least one request");
    assert!(b >= 1, "backend B should have received at least one request");
}

// ── Path prefix filtering ─────────────────────────────────────────────────────

#[test]
fn path_prefix_proxies_matching_requests() {
    let port = mock_backend(OK_RESPONSE);
    let proxy = ReverseProxy::new([format!("http://127.0.0.1:{}", port)])
        .path_prefix("/api");
    let app = App::new().wrap(proxy);
    let resp = app.execute(&get("/api/users"), &conn()).unwrap();
    assert_eq!(200, resp.status_code);
}

#[test]
fn path_prefix_passes_non_matching_to_inner_app() {
    let port = mock_backend(OK_RESPONSE);
    let proxy = ReverseProxy::new([format!("http://127.0.0.1:{}", port)])
        .path_prefix("/api");
    let app = App::new().wrap(proxy);
    // /healthz is handled by App, not the proxy
    let resp = app.execute(&get("/healthz"), &conn()).unwrap();
    assert_eq!(200, resp.status_code);
}

// ── Request construction ──────────────────────────────────────────────────────

#[test]
fn forwarded_request_includes_x_forwarded_for() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = vec![0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            buf.truncate(n);
            let _ = stream.write_all(OK_RESPONSE.as_bytes());
            String::from_utf8_lossy(&buf).to_string()
        } else {
            String::new()
        }
    });

    let proxy = ReverseProxy::new([format!("http://127.0.0.1:{}", port)]);
    let _ = proxy.handle(&get("/"), &conn(), &App::new());

    let received = handle.join().unwrap();
    assert!(received.contains("X-Forwarded-For: 10.0.0.1"), "missing X-Forwarded-For");
    assert!(received.contains("Via: 1.1 rws"), "missing Via");
}

#[test]
fn hop_by_hop_headers_are_stripped() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = vec![0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            buf.truncate(n);
            let _ = stream.write_all(OK_RESPONSE.as_bytes());
            String::from_utf8_lossy(&buf).to_string()
        } else {
            String::new()
        }
    });

    let mut req = get("/");
    req.headers.push(crate::header::Header {
        name: "Connection".to_string(),
        value: "keep-alive".to_string(),
    });
    req.headers.push(crate::header::Header {
        name: "Transfer-Encoding".to_string(),
        value: "chunked".to_string(),
    });
    req.headers.push(crate::header::Header {
        name: "X-Custom".to_string(),
        value: "should-pass".to_string(),
    });

    let proxy = ReverseProxy::new([format!("http://127.0.0.1:{}", port)]);
    let _ = proxy.handle(&req, &conn(), &App::new());

    let received = handle.join().unwrap();
    assert!(!received.to_lowercase().contains("transfer-encoding"), "Transfer-Encoding should be stripped");
    assert!(received.contains("X-Custom: should-pass"), "X-Custom header should be forwarded");
}

// ── Middleware integration ────────────────────────────────────────────────────

#[test]
fn proxy_can_be_used_as_middleware_wrap() {
    let port = mock_backend(OK_RESPONSE);
    let app = App::new()
        .wrap(ReverseProxy::new([format!("http://127.0.0.1:{}", port)]));
    let resp = app.execute(&get("/anything"), &conn()).unwrap();
    assert_eq!(200, resp.status_code);
}

#[test]
fn proxy_with_builder_options_compiles_and_works() {
    let port = mock_backend(OK_RESPONSE);
    let proxy = ReverseProxy::new([format!("http://127.0.0.1:{}", port)])
        .strategy(LoadBalancing::RoundRobin)
        .connect_timeout_ms(5000)
        .read_timeout_ms(30000)
        .path_prefix("/");
    let resp = proxy.handle(&get("/"), &conn(), &App::new()).unwrap();
    assert_eq!(200, resp.status_code);
}
