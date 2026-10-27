use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU32, Ordering as AtOrd};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::app::App;
use crate::application::Application;
use crate::core::New;
use crate::http::VERSION;
use crate::middleware::Middleware;
use crate::proxy::{ConnPool, LoadBalancing, ReverseProxy, read_headers_only, read_response_from_partial};
use crate::request::{METHOD, Request};
use crate::server::{Address, ConnectionInfo};

// ── test helpers ──────────────────────────────────────────────────────────────

fn conn() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "10.0.0.1".to_string(), port: 1234 },
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
        body: Default::default(),
    }
}

/// Spawns a one-shot mock backend: accepts one connection, reads the request,
/// writes `response`, then closes.
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

/// Keep-alive backend: accepts up to `max_conns` TCP connections, serves each
/// with `response`, keeps the socket open (simulates HTTP/1.1 keep-alive).
/// Returns (port, connections_accepted_counter).
fn keepalive_backend(response: &'static str, max_conns: usize) -> (u16, Arc<AtomicU32>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind keepalive backend");
    let port = listener.local_addr().unwrap().port();
    let counter = Arc::new(AtomicU32::new(0));
    let counter2 = Arc::clone(&counter);
    thread::spawn(move || {
        for _ in 0..max_conns {
            if let Ok((mut stream, _)) = listener.accept() {
                counter2.fetch_add(1, AtOrd::Relaxed);
                // Serve multiple requests on the same connection
                loop {
                    let mut buf = [0u8; 4096];
                    match stream.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {
                            if stream.write_all(response.as_bytes()).is_err() {
                                break;
                            }
                        }
                    }
                }
            }
        }
    });
    (port, counter)
}

/// Backend that immediately closes without sending anything.
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

fn refusing_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    drop(l);
    port
}

// Responses with Connection: close (pool will NOT reuse these connections).
const OK_RESPONSE: &str =
    "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello";
const NOT_FOUND_RESPONSE: &str =
    "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: close\r\n\r\nnot found";

// Response with Connection: keep-alive (pool WILL reuse the connection).
const KEEPALIVE_RESPONSE: &str =
    "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: keep-alive\r\n\r\nhello";

// Chunked response with Connection: keep-alive.
const CHUNKED_KEEPALIVE_RESPONSE: &str =
    "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: keep-alive\r\n\r\n5\r\nhello\r\n0\r\n\r\n";

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

// ── Circuit breaker wiring ─────────────────────────────────────────────────────

#[test]
fn circuit_breaker_skips_open_backend_without_dialing() {
    use crate::circuit_breaker::CircuitBreaker;
    use std::sync::Mutex;

    // A "backend" that would panic the test harness's listener thread if
    // ever dialed is overkill — instead, prove it wasn't dialed by using a
    // refusing port (so if the breaker's skip *didn't* work, the proxy would
    // get a connection-refused error instead of ever reaching the healthy
    // second backend below).
    let open_port = refusing_port();
    let key = format!("127.0.0.1:{}", open_port);

    let breaker = Arc::new(Mutex::new(CircuitBreaker::new(1, 30)));
    breaker.lock().unwrap().record_failure(&key); // threshold=1 -> opens immediately

    let good_port = mock_backend(OK_RESPONSE);
    let proxy = ReverseProxy::new([
        format!("http://127.0.0.1:{}", open_port),
        format!("http://127.0.0.1:{}", good_port),
    ])
    .connect_timeout_ms(200)
    .with_circuit_breaker(breaker);

    let resp = proxy.handle(&get("/"), &conn(), &App::new()).unwrap();
    assert_eq!(200, resp.status_code, "should skip the Open backend and reach the healthy one");
}

#[test]
fn circuit_breaker_records_success_on_successful_proxy() {
    use crate::circuit_breaker::{BreakerState, CircuitBreaker};
    use std::sync::Mutex;

    let port = mock_backend(OK_RESPONSE);
    let key = format!("127.0.0.1:{}", port);
    let breaker = Arc::new(Mutex::new(CircuitBreaker::new(3, 30)));
    let proxy = ReverseProxy::new([format!("http://127.0.0.1:{}", port)])
        .with_circuit_breaker(Arc::clone(&breaker) as Arc<dyn crate::circuit_breaker::Breaker>);

    let resp = proxy.handle(&get("/"), &conn(), &App::new()).unwrap();
    assert_eq!(200, resp.status_code);
    assert_eq!(BreakerState::Closed, breaker.lock().unwrap().state(&key));
}

#[test]
fn circuit_breaker_records_failure_and_opens_after_threshold() {
    use crate::circuit_breaker::{BreakerState, CircuitBreaker};
    use std::sync::Mutex;

    let port = refusing_port();
    let key = format!("127.0.0.1:{}", port);
    let breaker = Arc::new(Mutex::new(CircuitBreaker::new(1, 30)));
    let proxy = ReverseProxy::new([format!("http://127.0.0.1:{}", port)])
        .connect_timeout_ms(200)
        .with_circuit_breaker(Arc::clone(&breaker) as Arc<dyn crate::circuit_breaker::Breaker>);

    let resp = proxy.handle(&get("/"), &conn(), &App::new()).unwrap();
    assert_eq!(502, resp.status_code);
    assert_eq!(
        BreakerState::Open,
        breaker.lock().unwrap().state(&key),
        "a single failed dial should open the breaker (threshold=1)"
    );
}

#[test]
fn without_circuit_breaker_behavior_is_completely_unchanged() {
    // No `.with_circuit_breaker(...)` call at all — same as every other test
    // in this file, and covered here explicitly as a regression guard.
    let port = mock_backend(OK_RESPONSE);
    let proxy = ReverseProxy::new([format!("http://127.0.0.1:{}", port)]);
    let resp = proxy.handle(&get("/"), &conn(), &App::new()).unwrap();
    assert_eq!(200, resp.status_code);
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
    fn count_backend(resp_body: &'static str) -> (u16, Arc<AtomicU32>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let count = Arc::new(AtomicU32::new(0));
        let count_clone = Arc::clone(&count);
        let body = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            resp_body.len(),
            resp_body
        );
        thread::spawn(move || {
            for _ in 0..4 {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buf = [0u8; 4096];
                    let _ = stream.read(&mut buf);
                    let _ = stream.write_all(body.as_bytes());
                    count_clone.fetch_add(1, AtOrd::Relaxed);
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

    let a = count_a.load(AtOrd::Relaxed);
    let b = count_b.load(AtOrd::Relaxed);
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
        name: "Connection".into(),
        value: "keep-alive".into(),
    });
    req.headers.push(crate::header::Header {
        name: "Transfer-Encoding".into(),
        value: "chunked".into(),
    });
    req.headers.push(crate::header::Header {
        name: "X-Custom".into(),
        value: "should-pass".into(),
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

// ── Connection pool ───────────────────────────────────────────────────────────

#[test]
fn pool_reuses_connection_for_keepalive_response() {
    let (port, tcp_conn_count) = keepalive_backend(KEEPALIVE_RESPONSE, 1);
    let proxy = ReverseProxy::new([format!("http://127.0.0.1:{}", port)]);

    // First request — opens a TCP connection.
    let resp = proxy.handle(&get("/"), &conn(), &App::new()).unwrap();
    assert_eq!(200, resp.status_code);

    // Give the pool a moment to store the returned stream.
    thread::sleep(Duration::from_millis(20));

    // Pool should hold the idle connection.
    assert_eq!(1, proxy.pool.idle_count(), "pool should hold 1 idle connection after keep-alive response");

    // Second request — should reuse the pooled stream (no new TCP connection).
    let resp2 = proxy.handle(&get("/health"), &conn(), &App::new()).unwrap();
    assert_eq!(200, resp2.status_code);

    // Only one TCP connection should have been established.
    thread::sleep(Duration::from_millis(20));
    assert_eq!(1, tcp_conn_count.load(AtOrd::Relaxed), "only 1 TCP connection should be opened");
}

#[test]
fn pool_does_not_reuse_connection_close_response() {
    // Backend with Connection: close — pool must not keep the stream.
    let port = mock_backend(OK_RESPONSE);
    let proxy = ReverseProxy::new([format!("http://127.0.0.1:{}", port)]);
    let resp = proxy.handle(&get("/"), &conn(), &App::new()).unwrap();
    assert_eq!(200, resp.status_code);
    thread::sleep(Duration::from_millis(20));
    assert_eq!(0, proxy.pool.idle_count(), "pool must be empty after Connection: close");
}

#[test]
fn shared_pool_is_used_across_proxy_instances() {
    let (port, tcp_conn_count) = keepalive_backend(KEEPALIVE_RESPONSE, 1);
    let pool = Arc::new(ConnPool::new_default());
    let proxy1 = ReverseProxy::new([format!("http://127.0.0.1:{}", port)])
        .with_pool(Arc::clone(&pool));
    let proxy2 = ReverseProxy::new([format!("http://127.0.0.1:{}", port)])
        .with_pool(Arc::clone(&pool));

    let resp1 = proxy1.handle(&get("/"), &conn(), &App::new()).unwrap();
    assert_eq!(200, resp1.status_code);
    thread::sleep(Duration::from_millis(20));

    let resp2 = proxy2.handle(&get("/"), &conn(), &App::new()).unwrap();
    assert_eq!(200, resp2.status_code);
    thread::sleep(Duration::from_millis(20));

    assert_eq!(1, tcp_conn_count.load(AtOrd::Relaxed), "shared pool: only 1 TCP connection");
}

#[test]
fn pool_evicts_stale_connections() {
    use std::time::Duration;
    let pool = ConnPool::new(8, Duration::from_millis(1));

    // Bind a dummy socket so we can get a TcpStream.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || { let _ = listener.accept(); });
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();

    pool.release("127.0.0.1:9999", stream);
    assert_eq!(1, pool.idle_count());

    // Wait for the 1ms timeout to expire.
    thread::sleep(Duration::from_millis(5));
    assert!(pool.acquire("127.0.0.1:9999").is_none(), "stale connection should be evicted");
    assert_eq!(0, pool.idle_count());
}

#[test]
fn pool_respects_max_idle_limit() {
    let pool = ConnPool::new(2, Duration::from_secs(60));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    // Allow up to 3 connections
    thread::spawn(move || {
        for _ in 0..3 {
            let _ = listener.accept();
        }
    });
    for _ in 0..3 {
        let s = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        pool.release("h:1", s);
    }
    // max_idle is 2, so the 3rd should be dropped
    assert_eq!(2, pool.idle_count());
}

// ── Chunked decoding (production path: read_headers_only + read_response_from_partial) ──

/// Reads a full response the same way `ReverseProxy::try_backend`'s buffered
/// path does — split header/body-prefix read followed by the pooling-aware
/// body reader — rather than through a standalone single-buffer reader, so
/// these tests exercise the exact code path production traffic takes.
fn read_full_response(stream: &mut std::net::TcpStream) -> Result<(Vec<u8>, bool), String> {
    let mut tmp = [0u8; 4096];
    let (header_bytes, body_prefix) = read_headers_only(stream, &mut tmp)?;
    read_response_from_partial(stream, header_bytes, body_prefix, &mut tmp)
}

#[test]
fn chunked_response_is_decoded_and_poolable() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((mut s, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            let _ = s.write_all(CHUNKED_KEEPALIVE_RESPONSE.as_bytes());
            // Keep the connection open
            thread::sleep(Duration::from_millis(200));
        }
    });

    let mut stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    // Send a dummy request
    let _ = stream.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");

    let (resp_bytes, can_reuse) = read_full_response(&mut stream).unwrap();
    let resp = crate::response::Response::parse(&resp_bytes).unwrap();

    assert_eq!(200, resp.status_code);
    let body: Vec<u8> = resp.content_range_list.iter().flat_map(|c| c.body.iter().copied()).collect();
    assert_eq!(b"hello", body.as_slice(), "decoded body should be 'hello'");
    assert!(can_reuse, "chunked keep-alive response should be reusable");
}

#[test]
fn chunked_multi_chunk_response_is_decoded_correctly() {
    // Multi-chunk: "hel" + "lo" = "hello"
    let chunked = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: keep-alive\r\n\r\n3\r\nhel\r\n2\r\nlo\r\n0\r\n\r\n";
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((mut s, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            let _ = s.write_all(chunked.as_bytes());
            thread::sleep(Duration::from_millis(200));
        }
    });

    let mut stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let _ = stream.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
    let (resp_bytes, can_reuse) = read_full_response(&mut stream).unwrap();
    let resp = crate::response::Response::parse(&resp_bytes).unwrap();
    let body: Vec<u8> = resp.content_range_list.iter().flat_map(|c| c.body.iter().copied()).collect();
    assert_eq!(b"hello", body.as_slice());
    assert!(can_reuse);
}

#[test]
fn connection_close_response_is_not_reusable() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((mut s, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            let _ = s.write_all(OK_RESPONSE.as_bytes()); // has Connection: close
        }
    });

    let mut stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let _ = stream.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
    let (_resp_bytes, can_reuse) = read_full_response(&mut stream).unwrap();
    assert!(!can_reuse, "Connection: close responses must not be pooled");
}

#[test]
fn proxy_streams_chunked_response() {
    // Chunked responses are now forwarded via stream_pipe (not buffered into
    // content_range_list) so the client receives bytes as they arrive.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((mut s, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            let _ = s.write_all(CHUNKED_KEEPALIVE_RESPONSE.as_bytes());
            thread::sleep(Duration::from_millis(200));
        }
    });

    let proxy = ReverseProxy::new([format!("http://127.0.0.1:{}", port)]);
    let mut resp = proxy.handle(&get("/"), &conn(), &App::new()).unwrap();
    assert_eq!(200, resp.status_code);
    assert!(resp.stream_pipe.is_some(), "chunked response must use stream_pipe");
    assert!(resp.content_range_list.is_empty(), "body must not be buffered");

    // Drain the pipe — the raw bytes contain the chunk framing from the backend.
    let mut raw = Vec::new();
    resp.stream_pipe.as_mut().unwrap().read_to_end(&mut raw).ok();
    assert!(raw.contains(&b'h'), "chunk data must be present in raw bytes");
}

#[test]
fn proxy_streams_sse_response() {
    // SSE (text/event-stream) responses are forwarded via stream_pipe so the
    // client receives each event as soon as the backend sends it.
    let sse_response = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Content-Type: text/event-stream\r\n",
        "Cache-Control: no-cache\r\n",
        "Connection: keep-alive\r\n",
        "\r\n",
        "data: hello\n\n",
        "data: world\n\n",
    );

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((mut s, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            let _ = s.write_all(sse_response.as_bytes());
            thread::sleep(Duration::from_millis(200));
        }
    });

    let proxy = ReverseProxy::new([format!("http://127.0.0.1:{}", port)]);
    let mut resp = proxy.handle(&get("/"), &conn(), &App::new()).unwrap();
    assert_eq!(200, resp.status_code);
    assert!(resp.stream_pipe.is_some(), "SSE response must use stream_pipe");
    assert!(resp.content_range_list.is_empty(), "body must not be buffered");

    let mut raw = Vec::new();
    resp.stream_pipe.as_mut().unwrap().read_to_end(&mut raw).ok();
    let body = String::from_utf8_lossy(&raw);
    assert!(body.contains("data: hello"), "SSE event must be forwarded: {}", body);
}

#[test]
fn proxy_buffers_small_content_length_response() {
    // Small fixed-size responses still go through the buffered path so the
    // connection can be returned to the pool.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((mut s, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            let _ = s.write_all(KEEPALIVE_RESPONSE.as_bytes());
            thread::sleep(Duration::from_millis(200));
        }
    });

    let proxy = ReverseProxy::new([format!("http://127.0.0.1:{}", port)]);
    let resp = proxy.handle(&get("/"), &conn(), &App::new()).unwrap();
    assert_eq!(200, resp.status_code);
    assert!(resp.stream_pipe.is_none(), "small responses must be buffered");
    let body: Vec<u8> = resp.content_range_list.iter()
        .flat_map(|c| c.body.iter().copied())
        .collect();
    assert_eq!(b"hello", body.as_slice());
}

#[test]
fn concat_reader_drains_prefix_then_inner() {
    use super::ConcatReader;
    use std::io::Read;

    let prefix = b"hello".to_vec();
    let inner = std::io::Cursor::new(b" world".to_vec());
    let mut r = ConcatReader::new(prefix, inner);

    let mut out = Vec::new();
    r.read_to_end(&mut out).unwrap();
    assert_eq!(b"hello world", out.as_slice());
}

#[test]
fn should_stream_detects_sse() {
    use super::should_stream_response;
    let h = "http/1.1 200 ok\r\ncontent-type: text/event-stream\r\n\r\n";
    assert!(should_stream_response(h));
}

#[test]
fn should_stream_detects_chunked() {
    use super::should_stream_response;
    let h = "http/1.1 200 ok\r\ntransfer-encoding: chunked\r\n\r\n";
    assert!(should_stream_response(h));
}

#[test]
fn should_not_stream_small_content_length() {
    use super::should_stream_response;
    let h = "http/1.1 200 ok\r\ncontent-length: 5\r\n\r\n";
    assert!(!should_stream_response(h));
}

#[test]
fn should_stream_large_content_length() {
    use super::should_stream_response;
    // 2 MB > 1 MB threshold
    let h = "http/1.1 200 ok\r\ncontent-length: 2097152\r\n\r\n";
    assert!(should_stream_response(h));
}

// ── H2ReverseProxy async bridging ────────────────────────────────────────────
//
// `H2ReverseProxy::handle` used to bridge into its async H2 client code via
// `tokio::task::block_in_place`, which panics on a `current_thread` runtime.
// `#[tokio::test]` defaults to exactly that flavor, so this test alone would
// have failed (panicked) under the old implementation — the connection
// itself is expected to fail (nothing is listening on port 1), but the
// `Middleware::handle` call must not panic getting there.

#[cfg(feature = "http2")]
#[tokio::test]
async fn h2_reverse_proxy_does_not_panic_under_current_thread_runtime() {
    use crate::proxy::H2ReverseProxy;

    let proxy = H2ReverseProxy::new(vec!["h2://127.0.0.1:1".to_string()]).connect_timeout_ms(200);
    let app = App::new().wrap(proxy);
    let response = app.execute(&get("/"), &conn()).unwrap();
    assert_eq!(502, response.status_code, "unreachable backend should yield 502, not a panic");
}

/// A backend that accepts and then says nothing used to hang the request
/// forever: `read_timeout_ms` was stored but never applied to the h2 client.
#[cfg(feature = "http2")]
#[test]
fn h2_reverse_proxy_applies_read_timeout_to_a_silent_backend() {
    use crate::proxy::H2ReverseProxy;

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        // Hold the accepted socket open without writing a byte.
        let _held: Vec<_> = listener.incoming().take(1).collect();
        std::thread::sleep(std::time::Duration::from_secs(10));
    });

    let proxy = H2ReverseProxy::new(vec![format!("h2://127.0.0.1:{}", port)]).read_timeout_ms(200);
    let started = std::time::Instant::now();
    let response = proxy.handle(&get("/"), &conn(), &App::new()).unwrap();
    assert_eq!(502, response.status_code);
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "took {:?}", started.elapsed());
}

// ── upstream response normalization ───────────────────────────────────────────

/// Python's http.server shape: HTTP/1.0, lowercase "Content-type".
const PYTHON_STYLE_RESPONSE: &str =
    "HTTP/1.0 200 OK\r\nServer: SimpleHTTP\r\nContent-type: text/html\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello";

fn header_lines<'a>(raw: &'a str, name: &str) -> Vec<&'a str> {
    raw.split("\r\n\r\n").next().unwrap()
        .lines()
        .filter(|l| l.to_ascii_lowercase().starts_with(&format!("{}:", name)))
        .collect()
}

#[test]
fn proxied_response_has_one_content_type_and_length_with_the_upstream_type() {
    let port = mock_backend(PYTHON_STYLE_RESPONSE);
    let proxy = ReverseProxy::new([format!("http://127.0.0.1:{}", port)]);
    let resp = proxy.handle(&get("/"), &conn(), &App::new()).unwrap();
    // What Server::process writes to the socket for HTTP/1.1.
    let raw = String::from_utf8_lossy(&crate::response::Response::generate_response(resp, get("/"))).to_string();

    assert!(raw.starts_with("HTTP/1.1 200"), "status line: {}", raw.lines().next().unwrap());
    assert_eq!(vec!["Content-Type: text/html"], header_lines(&raw, "content-type"));
    assert_eq!(vec!["Content-Length: 5"], header_lines(&raw, "content-length"));
    assert!(header_lines(&raw, "connection").is_empty(), "hop-by-hop header forwarded");
    assert_eq!(vec!["Server: SimpleHTTP"], header_lines(&raw, "server"), "end-to-end headers kept");
    assert!(raw.ends_with("hello"));
}

#[test]
fn normalize_keeps_content_type_header_when_there_is_no_body() {
    use crate::header::Header;
    use crate::response::Response;
    let mut resp = Response::new();
    resp.http_version = "HTTP/1.0".to_string();
    resp.headers = vec![
        Header { name: "content-type".into(), value: "text/html".into() },
        Header { name: "Transfer-Encoding".into(), value: "chunked".into() },
        Header { name: "ETag".into(), value: "\"abc\"".into() },
    ];
    let resp = crate::proxy::normalize_upstream_response(resp);
    assert_eq!("HTTP/1.1", resp.http_version);
    let names: Vec<&str> = resp.headers.iter().map(|h| h.name.as_ref()).collect();
    assert_eq!(vec!["content-type", "ETag"], names);
}

#[test]
fn proxy_relays_upstream_404_with_a_non_standard_reason_phrase() {
    // Python's http.server: "404 File not found" — was turned into 502.
    let port = mock_backend("HTTP/1.0 404 File not found\r\nContent-Type: text/html\r\nContent-Length: 9\r\nConnection: close\r\n\r\nnot found");
    let proxy = ReverseProxy::new([format!("http://127.0.0.1:{}", port)]);
    let resp = proxy.handle(&get("/missing"), &conn(), &App::new()).unwrap();
    assert_eq!(404, resp.status_code);
}

#[test]
fn proxied_request_has_no_empty_header_line() {
    // Strict backends (nginx) answer 400 to a ": " header line.
    let req = crate::request::Request::parse(b"GET /api/1k.bin HTTP/1.1\r\nHost: a\r\nAccept: */*\r\n\r\n").unwrap();
    assert!(req.headers.iter().all(|h| !h.name.is_empty()), "request line parsed as a header");
    let out = String::from_utf8(crate::proxy::build_request(&req, "127.0.0.1:9000", "10.0.0.1", true)).unwrap();
    assert!(!out.contains("\r\n: "), "{out:?}");
    assert!(out.starts_with("GET /api/1k.bin HTTP/1.1\r\nHost: 127.0.0.1:9000\r\nAccept: */*\r\n"), "{out:?}");
}

// ── forward_pooled: reuse, stale-connection retry, no replay of POST ──────────

/// Serves one keep-alive response per accepted connection, then closes it —
/// like a backend whose idle timeout fires right after responding. Returns
/// (port, connections accepted).
fn closes_after_each_response_backend(max_conns: usize) -> (u16, Arc<AtomicU32>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let accepted = Arc::new(AtomicU32::new(0));
    let accepted2 = Arc::clone(&accepted);
    thread::spawn(move || {
        for _ in 0..max_conns {
            if let Ok((mut stream, _)) = listener.accept() {
                accepted2.fetch_add(1, AtOrd::Relaxed);
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: keep-alive\r\n\r\nok");
                // Closed without Connection: close — the pooled copy is now stale.
            }
        }
    });
    (port, accepted)
}

fn post(uri: &str) -> Request {
    let mut r = get(uri);
    r.method = METHOD.post.to_string();
    r.body = b"x".to_vec().into();
    r
}

#[test]
fn forward_pooled_reuses_one_backend_connection() {
    let (port, accepted) = keepalive_backend(KEEPALIVE_RESPONSE, 3);
    let pool = ConnPool::new(8, Duration::from_secs(60));
    for _ in 0..5 {
        let resp = crate::proxy::forward_pooled(&pool, &get("/"), "10.0.0.1", "127.0.0.1", port, Duration::from_secs(2), Duration::from_secs(2)).unwrap();
        assert_eq!(200, resp.status_code);
    }
    assert_eq!(1, accepted.load(AtOrd::Relaxed), "5 sequential requests, 1 TCP connection");
}

#[test]
fn forward_pooled_retries_a_stale_connection_for_get() {
    let (port, accepted) = closes_after_each_response_backend(4);
    let pool = ConnPool::new(8, Duration::from_secs(60));
    let fwd = |r: &Request| crate::proxy::forward_pooled(&pool, r, "10.0.0.1", "127.0.0.1", port, Duration::from_secs(2), Duration::from_secs(2));
    assert_eq!(200, fwd(&get("/")).unwrap().status_code);
    thread::sleep(Duration::from_millis(50)); // let the close land
    // The pooled connection is dead; a GET is retried on a fresh one.
    assert_eq!(200, fwd(&get("/")).unwrap().status_code);
    assert_eq!(2, accepted.load(AtOrd::Relaxed));
}

#[test]
fn forward_pooled_does_not_replay_a_post_on_a_stale_connection() {
    let (port, accepted) = closes_after_each_response_backend(4);
    let pool = ConnPool::new(8, Duration::from_secs(60));
    let fwd = |r: &Request| crate::proxy::forward_pooled(&pool, r, "10.0.0.1", "127.0.0.1", port, Duration::from_secs(2), Duration::from_secs(2));
    assert_eq!(200, fwd(&get("/")).unwrap().status_code);
    thread::sleep(Duration::from_millis(50));
    // The backend might have received it: a non-idempotent request fails
    // rather than being sent twice.
    assert!(fwd(&post("/")).is_err());
    assert_eq!(1, accepted.load(AtOrd::Relaxed), "no second connection for the POST");
}

// ── HTTPS backends: pooled TLS connections ────────────────────────────────────

/// A keep-alive HTTPS backend with a fresh self-signed certificate for
/// `localhost`. Each request gets `response`; `responses_per_conn` requests
/// are served per connection before it is closed. Returns (port, client TLS
/// config trusting the certificate, connections accepted).
#[cfg(feature = "http2")]
fn tls_backend(response: &'static [u8], responses_per_conn: usize) -> (u16, Arc<rustls::ClientConfig>, Arc<AtomicU32>) {
    let (port, config, accepted, _) = tls_backend_with_pem(response, responses_per_conn);
    (port, config, accepted)
}

/// [`tls_backend`] that also returns the backend certificate as PEM.
#[cfg(feature = "http2")]
pub(crate) fn tls_backend_with_pem(response: &'static [u8], responses_per_conn: usize) -> (u16, Arc<rustls::ClientConfig>, Arc<AtomicU32>, String) {
    let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let cert_pem = certified.cert.pem();
    let cert_der = rustls::pki_types::CertificateDer::from(certified.cert.der().to_vec());
    let key_der = rustls::pki_types::PrivateKeyDer::try_from(certified.signing_key.serialize_der()).unwrap();
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let server_config = Arc::new(
        rustls::ServerConfig::builder_with_provider(Arc::clone(&provider))
            .with_safe_default_protocol_versions().unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der.clone()], key_der)
            .unwrap(),
    );
    let mut roots = rustls::RootCertStore::empty();
    roots.add(cert_der).unwrap();
    let client_config = Arc::new(
        rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions().unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    );

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let accepted = Arc::new(AtomicU32::new(0));
    let accepted2 = Arc::clone(&accepted);
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            accepted2.fetch_add(1, AtOrd::Relaxed);
            let server_config = Arc::clone(&server_config);
            thread::spawn(move || {
                let conn = rustls::ServerConnection::new(server_config).unwrap();
                let mut tls = rustls::StreamOwned::new(conn, stream);
                let mut buf = [0u8; 4096];
                for _ in 0..responses_per_conn {
                    let mut head = Vec::new();
                    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                        match tls.read(&mut buf) {
                            Ok(0) | Err(_) => return,
                            Ok(n) => head.extend_from_slice(&buf[..n]),
                        }
                    }
                    if tls.write_all(response).and_then(|_| tls.flush()).is_err() {
                        return;
                    }
                }
                tls.conn.send_close_notify();
                let _ = tls.flush();
            });
        }
    });
    (port, client_config, accepted, cert_pem)
}

#[cfg(feature = "http2")]
fn forward_tls(pool: &crate::proxy::Pool<super::TlsStream>, config: &Arc<rustls::ClientConfig>, port: u16, request: &Request) -> Result<crate::response::Response, String> {
    super::forward_via(pool, request, "10.0.0.1", "localhost", port, "localhost", Duration::from_secs(5), || {
        super::connect_tls_with(Arc::clone(config), "localhost", port, Duration::from_secs(2))
    })
}

#[cfg(feature = "http2")]
#[test]
fn https_backend_connections_are_pooled() {
    let (port, config, accepted) = tls_backend(KEEPALIVE_RESPONSE.as_bytes(), 100);
    let pool = crate::proxy::Pool::new(8, Duration::from_secs(60));
    for _ in 0..5 {
        let response = forward_tls(&pool, &config, port, &get("/")).unwrap();
        assert_eq!(200, response.status_code);
    }
    assert_eq!(1, accepted.load(AtOrd::Relaxed), "5 sequential HTTPS requests, 1 TLS handshake");
    assert_eq!(1, pool.idle_count());
}

#[cfg(feature = "http2")]
#[test]
fn https_backend_stale_connection_is_retried_for_get_only() {
    let (port, config, accepted) = tls_backend(KEEPALIVE_RESPONSE.as_bytes(), 1);
    let pool = crate::proxy::Pool::new(8, Duration::from_secs(60));
    assert_eq!(200, forward_tls(&pool, &config, port, &get("/")).unwrap().status_code);
    thread::sleep(Duration::from_millis(50));
    assert_eq!(200, forward_tls(&pool, &config, port, &get("/")).unwrap().status_code);
    assert_eq!(2, accepted.load(AtOrd::Relaxed));
    thread::sleep(Duration::from_millis(50));
    assert!(forward_tls(&pool, &config, port, &post("/")).is_err());
    assert_eq!(2, accepted.load(AtOrd::Relaxed), "no new connection for the POST");
}

#[cfg(feature = "http2")]
#[test]
fn https_backend_chunked_response_is_streamed() {
    const CHUNKED: &[u8] = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: keep-alive\r\n\r\n5\r\nhello\r\n0\r\n\r\n";
    let (port, config, _) = tls_backend(CHUNKED, 1);
    let pool = crate::proxy::Pool::new(8, Duration::from_secs(60));
    let mut response = forward_tls(&pool, &config, port, &get("/")).unwrap();
    let mut pipe = response.stream_pipe.take().expect("chunked responses are streamed");
    let mut raw = Vec::new();
    let _ = pipe.read_to_end(&mut raw);
    assert!(raw.starts_with(b"5\r\nhello\r\n0\r\n"), "{:?}", String::from_utf8_lossy(&raw));
}

#[cfg(feature = "http2")]
#[test]
fn reverse_proxy_uses_tls_for_https_backends() {
    // Used to send plain HTTP to https:// backends. Record the first byte the
    // backend receives: 0x16 is a TLS handshake record, b'G' would be "GET".
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut first = [0u8; 1];
            let _ = stream.read_exact(&mut first);
            let _ = tx.send(first[0]);
        }
    });
    let proxy = ReverseProxy::new([format!("https://127.0.0.1:{}", port)]);
    let _ = proxy.handle(&get("/"), &conn(), &App::new());
    assert_eq!(0x16, rx.recv_timeout(Duration::from_secs(5)).unwrap());
}

#[test]
fn head_response_with_content_length_is_not_waited_on() {
    // A HEAD response's Content-Length describes the GET body; none follows.
    let (port, accepted) = keepalive_backend("HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n", 2);
    let pool = ConnPool::new(8, Duration::from_secs(60));
    let mut head = get("/");
    head.method = METHOD.head.to_string();
    let started = std::time::Instant::now();
    let response = crate::proxy::forward_pooled(&pool, &head, "10.0.0.1", "127.0.0.1", port, Duration::from_secs(2), Duration::from_secs(2)).unwrap();
    assert_eq!(200, response.status_code);
    assert!(started.elapsed() < Duration::from_secs(1), "waited {:?} for a body", started.elapsed());
    // The connection is still usable for the next request.
    let again = crate::proxy::forward_pooled(&pool, &head, "10.0.0.1", "127.0.0.1", port, Duration::from_secs(2), Duration::from_secs(2)).unwrap();
    assert_eq!(200, again.status_code);
    assert_eq!(1, accepted.load(AtOrd::Relaxed));
}


// ── tls_ca_file: trusting a private CA ────────────────────────────────────────

#[cfg(feature = "http2")]
#[test]
fn reverse_proxy_trusts_a_private_ca_from_tls_ca_file() {
    let (port, _, _, pem) = tls_backend_with_pem(KEEPALIVE_RESPONSE.as_bytes(), 100);
    let ca = crate::test_env::TempPath::new("rp_private_ca.pem");
    std::fs::write(ca.as_str(), pem).unwrap();

    // Self-signed: not trusted by the WebPKI roots.
    let public = ReverseProxy::new([format!("https://localhost:{port}")]);
    let refused = public.handle(&get("/"), &conn(), &App::new());
    assert!(refused.map_or(true, |r| r.status_code == 502));

    let private = ReverseProxy::new([format!("https://localhost:{port}")]).tls_ca_file(ca.as_str()).unwrap();
    let response = private.handle(&get("/"), &conn(), &App::new()).unwrap();
    assert_eq!(200, response.status_code);
}

#[cfg(feature = "http2")]
#[test]
fn tls_ca_file_errors_are_reported() {
    assert!(crate::proxy::UpstreamTls::from_ca_file("/nonexistent/ca.pem").is_err());
    let empty = crate::test_env::TempPath::new("rp_empty_ca.pem");
    std::fs::write(empty.as_str(), "not a certificate").unwrap();
    let err = crate::proxy::UpstreamTls::from_ca_file(empty.as_str()).err().unwrap();
    assert!(err.contains("no certificates"), "{err}");
}

#[test]
fn proxied_request_drops_hop_by_hop_and_host_in_any_case() {
    let req = crate::request::Request::parse(
        b"POST /a?b=1 HTTP/1.1\r\nHOST: client\r\nConnection: x\r\nKEEP-ALIVE: 5\r\nTe: trailers\r\n\xe2\x84\xaaeep-Alive: 1\r\nX-Kept: v\r\nContent-Length: 3\r\n\r\nabc",
    ).unwrap();
    let out = crate::proxy::build_request(&req, "b:9", "10.0.0.1", false);
    // The client's Content-Length is regenerated, not forwarded twice.
    let expected = "POST /a?b=1 HTTP/1.1\r\nHost: b:9\r\nX-Kept: v\r\n\
        X-Forwarded-For: 10.0.0.1\r\nVia: 1.1 rws\r\nConnection: close\r\nContent-Length: 3\r\n\r\nabc";
    assert_eq!(expected, String::from_utf8(out.clone()).unwrap());
    assert!(out.capacity() - out.len() < 16, "sized up front, not grown: {} of {}", out.len(), out.capacity());
}

#[test]
fn proxied_request_has_one_content_length_and_drops_connection_listed_headers() {
    let req = crate::request::Request::parse(
        b"POST / HTTP/1.1\r\nHost: a\r\nconnection: X-Secret, keep-alive\r\nX-Secret: s\r\nTrailer: X-Sum\r\n\
          Proxy-Connection: keep-alive\r\ncontent-length: 2\r\nX-Kept: v\r\n\r\nhi",
    ).unwrap();
    let out = String::from_utf8(crate::proxy::build_request(&req, "b:9", "10.0.0.1", true)).unwrap();
    assert_eq!(1, out.to_ascii_lowercase().matches("content-length:").count(), "{out:?}");
    assert!(out.contains("Content-Length: 2\r\n"), "{out:?}");
    for gone in ["X-Secret", "Trailer", "Proxy-Connection"] {
        assert!(!out.contains(gone), "{gone} forwarded: {out:?}");
    }
    assert!(out.contains("X-Kept: v\r\n"), "{out:?}");
}

#[test]
fn proxied_request_extends_an_incoming_x_forwarded_for() {
    let req = crate::request::Request::parse(
        b"GET / HTTP/1.1\r\nHost: a\r\nx-forwarded-for: 203.0.113.7, 198.51.100.2\r\n\r\n",
    ).unwrap();
    let out = crate::proxy::build_request(&req, "b:9", "10.0.0.1", true);
    let text = String::from_utf8(out.clone()).unwrap();
    assert_eq!(1, text.to_ascii_lowercase().matches("x-forwarded-for:").count(), "{text:?}");
    assert!(text.contains("X-Forwarded-For: 203.0.113.7, 198.51.100.2, 10.0.0.1\r\n"), "{text:?}");
    assert!(out.capacity() - out.len() < 16, "sized up front, not grown: {} of {}", out.len(), out.capacity());
}

#[test]
fn normalize_upstream_response_moves_content_type_and_drops_regenerated_headers() {
    let raw = b"HTTP/1.0 200 OK\r\nCONTENT-TYPE: text/x\r\nContent-Length: 2\r\nTransfer-Encoding: chunked\r\nX-Up: 1\r\n\r\nhi";
    let response = crate::proxy::normalize_upstream_response(crate::response::Response::parse(raw).unwrap());
    assert_eq!("HTTP/1.1", response.http_version);
    assert_eq!("text/x", response.content_range_list[0].content_type);
    let names: Vec<&str> = response.headers.iter().map(|h| h.name.as_ref()).collect();
    assert_eq!(vec!["X-Up"], names);
    // No body: Content-Type stays as a header.
    let head = crate::response::Response::parse(b"HTTP/1.1 304 Not Modified\r\nContent-Type: text/x\r\n\r\n").unwrap();
    let head = crate::proxy::normalize_upstream_response(crate::response::Response { content_range_list: vec![], ..head });
    assert_eq!(1, head.headers.len());
}

#[test]
fn pool_release_respects_max_idle_per_key_and_acquire_returns_streams() {
    use crate::proxy::pool::Pool;
    let pool: Pool<u32> = Pool::new(2, Duration::from_secs(30));
    assert!(pool.acquire("a:1").is_none());
    pool.release("a:1", 1);
    pool.release("a:1", 2);
    pool.release("a:1", 3); // over max_idle for this key: dropped
    pool.release("b:2", 4);
    assert_eq!(3, pool.idle_count());
    assert_eq!(Some(1), pool.acquire("a:1"));
    assert_eq!(Some(2), pool.acquire("a:1"));
    assert_eq!(None, pool.acquire("a:1"));
    assert_eq!(Some(4), pool.acquire("b:2"));
}

// HeadFacts (one pass over the raw head) must agree with the helpers that
// scan a lower-cased copy.
#[test]
fn head_facts_agree_with_the_lowercase_scans() {
    use crate::proxy::{response_has_no_body, should_stream_response, HeadFacts};
    let heads: Vec<&[u8]> = vec![
        b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: keep-alive\r\n\r\n",
        b"HTTP/1.1 200 OK\r\nCONTENT-LENGTH: 2000000\r\n\r\n",
        b"HTTP/1.1 200 OK\r\ncontent-length: nope\r\nContent-Length: 7\r\n\r\n",
        b"HTTP/1.1 204 No Content\r\nConnection: Close\r\n\r\n",
        b"HTTP/1.1 304 Not Modified\r\n\r\n",
        b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\n\r\n",
        b"HTTP/1.1 200 OK\r\nContent-Type: Text/Event-Stream; charset=utf-8\r\n\r\n",
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip, CHUNKED\r\n\r\n",
        b"HTTP/1.1 200 OK\r\nconnection: keep-alive, close\r\nX-Connection: close\r\n\r\n",
        b"HTTP/1.1 200 OK\nContent-Length: 3\nConnection:close\n\n",
        b"HTTP/1.1\r\n204 x\r\n\r\n",
        b"HTTP/1.1 200 OK\r\nX: \xc3\xa9t\xc3\xa9\r\nContent-Length: 1\r\n\r\n",
        b"HTTP/1.1 200 OK\r\nX: \xff\r\nContent-Length: 1\r\n\r\n",
        b"",
    ];
    for head in heads {
        let lower = std::str::from_utf8(head).unwrap_or("").to_ascii_lowercase();
        let facts = HeadFacts::scan(head);
        let label = String::from_utf8_lossy(head);
        for method in ["GET", "HEAD", "head"] {
            assert_eq!(response_has_no_body(method, &lower), facts.no_body(method), "{label:?} {method}");
        }
        assert_eq!(should_stream_response(&lower), facts.should_stream(), "{label:?}");
        assert_eq!(lower.lines().any(|l| l.starts_with("connection:") && l.contains("close")), facts.connection_close, "{label:?}");
        let content_length: Option<usize> = lower.lines().find_map(|l| l.strip_prefix("content-length:")?.trim().parse().ok());
        assert_eq!(content_length, facts.content_length, "{label:?}");
    }
}

/// `Response::parse_upstream` must build exactly what the two-step path —
/// `Response::parse` then `normalize_upstream_response` — builds, on the
/// response parser's reference corpus and seeded mutations of it.
#[test]
fn parse_upstream_matches_parse_then_normalize() {
    fn summary(result: &Result<crate::response::Response, String>) -> String {
        match result {
            Err(e) => format!("Err({e})"),
            Ok(r) => {
                let mut s = format!("{:?} {} {:?}\n", r.http_version, r.status_code, r.reason_phrase);
                for h in &r.headers {
                    s.push_str(&format!("{:?}={:?}\n", h.name, h.value));
                }
                for cr in &r.content_range_list {
                    s.push_str(&format!("[{} {}-{} {} {:?} {:?}]\n", cr.unit, cr.range.start, cr.range.end, cr.size, cr.content_type, cr.body));
                }
                s
            }
        }
    }
    let corpus: Vec<&[u8]> = vec![
        b"HTTP/1.1 200 OK\r\nServer: nginx/1.24.0\r\nDate: Sun, 25 Oct 2026 16:00:00 GMT\r\nContent-Type: application/octet-stream\r\nContent-Length: 5\r\nLast-Modified: x\r\nConnection: keep-alive\r\nETag: \"a\"\r\nAccept-Ranges: bytes\r\n\r\nhello",
        b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nhello",
        b"HTTP/1.1 404 File not found\r\nServer: x\r\n\r\n",
        b"HTTP/1.1 204\r\n\r\n",
        b"HTTP/1.0 599 \r\nX:  two spaces \r\n\r\nbody\r\nmore",
        b"HTTP/1.1 200 OK\nLF-Only: yes\nTransfer-Encoding: chunked\n\nbody",
        b"HTTP/1.1 200 OK\r\nNo-Separator\r\n\r\n",
        b"HTTP/1.1 200 OK\r\nName:\r\n\r\n",
        b"HTTP/1.1 200 OK\r\nA:b: c\r\nD::  e\r\n\r\n",
        b"HTTP/1.1 200 OK\r\nX: mid\rcr\r\nContent-Type: te\rxt\r\n\r\n",
        b"HTTP/1.1 200 O\rK\r\n\r\n",
        b"HTTP/1.1 200 OK\r\nContent-Length: nope\r\n\r\n",
        b"HTTP/1.1 200 OK\r\ncontent-length:  7 \r\nKeep-Alive: timeout=5\r\nTE: x\r\nUpgrade: h2c\r\nTrailer: y\r\nProxy-Connection: z\r\n\r\n1234567",
        b"HTTP/1.1 200 OK\r\nX: y",
        b"HTTP/1.1 200 OK",
        b"HTTP/1.1 200 OK\r\n \t \r\nafter-blank",
        b"HTTP/2 200 OK\r\n\r\n",
        b"HTTP/1.1 20 OK\r\n\r\n",
        b"\r\n\r\n",
        b"",
        b"HTTP/1.1 200 OK\r\nX: \xff\xfe\r\n\r\n",
        b"HTTP/1.1 200 OK\r\nX: \xc3\xa9t\xc3\xa9\r\n\r\n\xff\x00binary",
        b"HTTP/1.1 206 Partial Content\r\nContent-Type: multipart/byteranges; boundary=XX\r\nConnection: close\r\n\r\n--XX\r\nContent-Type: text/plain\r\nContent-Range: bytes 0-1/10\r\n\r\nab\r\n--XX\r\nContent-Type: text/plain\r\nContent-Range: bytes 8-9/10\r\n\r\nij\r\n--XX--\r\n",
        b"HTTP/1.1 206 Partial Content\r\nContent-Type: multipart/byteranges\r\n\r\n--XX\r\n",
        b"HTTP/1.1 200 OK\r\nContent-Type: a\r\ncontent-type: b\r\nCONNECTION: close\r\n\r\nx",
        b"HTTP/1.1 304 Not Modified\r\nETag: \"a\"\r\nContent-Type: text/html\r\n\r\n",
    ];
    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; seed };
    let interesting = [b'\r', b'\n', b' ', b':', b'\t', 0xff, 0xc3, b'0', b'-'];
    let mut checked = 0;
    for input in &corpus {
        let mut cases = vec![input.to_vec()];
        for _ in 0..200 {
            let mut m = input.to_vec();
            for _ in 0..1 + next() % 3 {
                let r = next();
                match r % 3 {
                    0 if !m.is_empty() => { let i = (r >> 8) as usize % m.len(); m[i] = interesting[(r >> 32) as usize % interesting.len()]; }
                    1 => { let i = (r >> 8) as usize % (m.len() + 1); m.insert(i, interesting[(r >> 32) as usize % interesting.len()]); }
                    _ if !m.is_empty() => { let i = (r >> 8) as usize % m.len(); m.remove(i); }
                    _ => {}
                }
            }
            cases.push(m);
        }
        for case in cases {
            let fused = crate::response::Response::parse_upstream(&case, super::REGENERATED_OR_HOP_BY_HOP);
            let two_step = crate::response::Response::parse(&case).map(super::normalize_upstream_response);
            assert_eq!(summary(&two_step), summary(&fused), "input {:?}", String::from_utf8_lossy(&case));
            checked += 1;
        }
    }
    assert!(checked > 5000);
}

// ── #48: interim (1xx) responses from the upstream ───────────────────────────

/// A keep-alive backend on one connection that answers the n-th request
/// with `responses[n]` (raw bytes, possibly several heads).
fn scripted_backend(responses: Vec<&'static str>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            for response in responses {
                let mut buf = [0u8; 4096];
                let mut got = Vec::new();
                while !got.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut buf) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => got.extend_from_slice(&buf[..n]),
                    }
                }
                let _ = stream.write_all(response.as_bytes());
            }
        }
    });
    port
}

const EARLY_HINTS_THEN_A: &str = "HTTP/1.1 103 Early Hints\r\nLink: </a.css>; rel=preload\r\n\r\nHTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 6\r\n\r\nbody-A";
const PLAIN_B: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 6\r\n\r\nbody-B";

fn body_of(response: &crate::response::Response) -> String {
    String::from_utf8_lossy(&response.content_range_list.first().map(|c| c.body.clone()).unwrap_or_default()).to_string()
}

#[test]
fn sync_forwarder_reads_past_interim_responses_and_keeps_the_pool_in_step() {
    let port = scripted_backend(vec![EARLY_HINTS_THEN_A, PLAIN_B]);
    let pool = ConnPool::new(4, Duration::from_secs(30));
    let t = Duration::from_secs(5);
    let first = super::forward_upstream(&pool, &get("/a"), "10.0.0.1", "127.0.0.1", port, false, t, t).unwrap();
    assert_eq!(200, first.status_code);
    assert_eq!("body-A", body_of(&first));
    // The second request reuses the pooled connection: it must get B, not
    // the remainder of the first exchange.
    let second = super::forward_upstream(&pool, &get("/b"), "10.0.0.1", "127.0.0.1", port, false, t, t).unwrap();
    assert_eq!(200, second.status_code);
    assert_eq!("body-B", body_of(&second));
}

#[cfg(feature = "http2")]
#[tokio::test]
async fn async_forwarder_reads_past_interim_responses_and_keeps_the_pool_in_step() {
    let port = scripted_backend(vec![EARLY_HINTS_THEN_A, PLAIN_B]);
    let t = Duration::from_secs(5);
    let first = super::async_upstream::forward(&get("/a"), "10.0.0.1", "127.0.0.1", port, None, t, t).await.unwrap();
    assert_eq!(200, first.status_code);
    assert_eq!("body-A", body_of(&first));
    let second = super::async_upstream::forward(&get("/b"), "10.0.0.1", "127.0.0.1", port, None, t, t).await.unwrap();
    assert_eq!(200, second.status_code);
    assert_eq!("body-B", body_of(&second));
}

#[test]
fn an_endless_stream_of_interim_responses_is_an_error() {
    let flood: &'static str = Box::leak("HTTP/1.1 100 Continue\r\n\r\n".repeat(super::MAX_INTERIM_RESPONSES + 2).into_boxed_str());
    let port = scripted_backend(vec![flood]);
    let pool = ConnPool::new(4, Duration::from_secs(30));
    let t = Duration::from_secs(2);
    assert!(super::forward_upstream(&pool, &get("/"), "10.0.0.1", "127.0.0.1", port, false, t, t).is_err());
}

#[test]
fn interim_classification() {
    assert!(super::is_interim_response(b"HTTP/1.1 100 Continue\r\n\r\n"));
    assert!(super::is_interim_response(b"HTTP/1.1 103 Early Hints\r\n\r\n"));
    assert!(!super::is_interim_response(b"HTTP/1.1 101 Switching Protocols\r\n\r\n"));
    assert!(!super::is_interim_response(b"HTTP/1.1 200 OK\r\n\r\n"));
    assert!(!super::is_interim_response(b"HTTP/1.1 1\r\n\r\n"));
    assert!(!super::is_interim_response(b""));
}

#[test]
fn inside_a_traced_request_the_upstream_gets_our_traceparent_and_baggage() {
    crate::otel::setup(crate::otel::TracingConfig { exporter: crate::otel::ExporterConfig::Discard, ..Default::default() });
    let req = crate::request::Request::parse(
        b"GET / HTTP/1.1\r\nHost: a\r\ntraceparent: 00-11111111111111111111111111111111-2222222222222222-01\r\nbaggage: theirs=1\r\ntracestate: k=v\r\n\r\n",
    )
    .unwrap();
    thread::spawn(move || {
        let untraced = String::from_utf8(crate::proxy::build_request(&req, "b:9", "10.0.0.1", true)).unwrap();
        assert!(untraced.contains("traceparent: 00-1111") && untraced.contains("baggage: theirs=1"), "passed through untouched: {untraced:?}");
        let root = crate::otel::span("request");
        crate::otel::set_baggage("tenant", "acme");
        let traced = String::from_utf8(crate::proxy::build_request(&req, "b:9", "10.0.0.1", true)).unwrap();
        let ours = crate::otel::propagation_headers().unwrap().0;
        assert_eq!(1, traced.matches("traceparent").count(), "{traced:?}");
        assert!(traced.contains(&format!("traceparent: {ours}\r\n")), "{traced:?}");
        assert!(traced.contains("baggage: tenant=acme\r\n") && !traced.contains("theirs"), "{traced:?}");
        assert!(traced.contains("tracestate: k=v\r\n"), "tracestate kept: {traced:?}");
        drop(root);
    })
    .join()
    .unwrap();
}
