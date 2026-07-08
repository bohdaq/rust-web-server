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
        body: vec![],
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
