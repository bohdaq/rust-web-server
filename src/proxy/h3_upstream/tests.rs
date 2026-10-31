//! The HTTP/3 upstream client against rws's own HTTP/3 server
//! (`h3_handler`), in-process over loopback UDP.

use super::*;
use crate::application::Application;
use crate::response::STATUS_CODE_REASON_PHRASE;
use crate::server::ConnectionInfo;
use crate::test_env::TempPath;

const BIG: usize = 2 * 1024 * 1024 + 7;

#[derive(Clone)]
struct Backend;

impl Application for Backend {
    fn execute(&self, request: &Request, _connection: &ConnectionInfo) -> Result<Response, String> {
        let header = |name: &str| request.header(name).map(|h| h.value.to_string()).unwrap_or_default();
        Ok(match request.request_uri.as_str() {
            "/hello" => Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "hello over h3"),
            "/headers" => Response::text(
                STATUS_CODE_REASON_PHRASE.n200_ok,
                &format!("{} via={} xff={} proto={} custom={}", request.http_version, header("via"), header("x-forwarded-for"), header("x-forwarded-proto"), header("x-custom")),
            ),
            "/echo" => {
                let mut r = Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "");
                r.content_range_list[0].body = format!("{} {}", request.method, String::from_utf8_lossy(&request.body)).into_bytes();
                r
            }
            "/big" => {
                let mut r = Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "");
                r.content_range_list[0].body = (0..BIG).map(|i| (i % 251) as u8).collect();
                r
            }
            "/trailers" => {
                let mut r = Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "with trailers");
                r.trailers = Some(Trailers::with(vec![Header::new("x-checksum", "abc".to_string())]));
                r
            }
            _ => Response::text(STATUS_CODE_REASON_PHRASE.n404_not_found, "missing"),
        })
    }
}

fn server_runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap())
}

/// An HTTP/3 server for `localhost` with a fresh self-signed certificate;
/// its address and a client configuration trusting it.
fn start_backend() -> (SocketAddr, Arc<rustls::ClientConfig>) {
    let (endpoint, tls) = start_backend_on("127.0.0.1:0".parse().unwrap(), None);
    (endpoint.local_addr().unwrap(), tls)
}

/// [`start_backend`] on `bind`, optionally with a given certificate
/// (`(cert pem, key pem, der)`); returns the endpoint so a test can close it.
fn start_backend_on(bind: SocketAddr, cert: Option<(String, String, Vec<u8>)>) -> (quinn::Endpoint, Arc<rustls::ClientConfig>) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let cert_file = TempPath::new(&format!("h3_upstream_cert_{n}.pem"));
    let key_file = TempPath::new(&format!("h3_upstream_key_{n}.pem"));
    let (cert_pem, key_pem, der) = cert.unwrap_or_else(|| {
        let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        (certified.cert.pem(), certified.signing_key.serialize_pem(), certified.cert.der().to_vec())
    });
    std::fs::write(cert_file.as_str(), &cert_pem).unwrap();
    std::fs::write(key_file.as_str(), &key_pem).unwrap();
    let server_config = crate::tls::create_quinn_server_config(cert_file.as_str(), key_file.as_str()).unwrap();
    let endpoint = server_runtime().block_on(async move {
        let endpoint = quinn::Endpoint::server(server_config, bind).unwrap();
        let accepting = endpoint.clone();
        tokio::spawn(async move {
            while let Some(incoming) = accepting.accept().await {
                tokio::spawn(async move {
                    if let Ok(conn) = incoming.await {
                        let peer = conn.remote_address();
                        let _ = crate::h3_handler::handle_connection(conn, peer, Backend).await;
                    }
                });
            }
        });
        endpoint
    });
    let mut roots = rustls::RootCertStore::empty();
    roots.add(rustls::pki_types::CertificateDer::from(der)).unwrap();
    (endpoint, Arc::new(crate::tls_client_builder().with_root_certificates(roots).with_no_client_auth()))
}

fn target(port: u16, tls: &Arc<rustls::ClientConfig>) -> Target<'_> {
    Target { host: "localhost", port, tls, connect_timeout: Duration::from_millis(1500), read_timeout: Duration::from_secs(5) }
}

fn peer() -> crate::proxy::Peer<'static> {
    crate::proxy::Peer { ip: "203.0.113.9", tls: true }
}

fn request(raw: &str) -> Request {
    Request::parse(raw.as_bytes()).unwrap()
}

fn body(response: &mut Response) -> Vec<u8> {
    match response.stream_pipe.take() {
        Some(mut pipe) => {
            let mut out = Vec::new();
            pipe.read_to_end(&mut out).unwrap();
            out
        }
        None => response.content_range_list.first().map(|c| c.body.clone()).unwrap_or_default(),
    }
}

#[test]
fn requests_reach_the_backend_over_http3_and_share_a_connection() {
    let (addr, tls) = start_backend();
    let target = target(addr.port(), &tls);
    let mut hello = forward(&request("GET /hello HTTP/1.1\r\nHost: shop.example\r\n\r\n"), peer(), &target).unwrap();
    assert_eq!((hello.status_code, body(&mut hello)), (200, b"hello over h3".to_vec()));
    let mut seen = forward(&request("GET /headers HTTP/1.1\r\nHost: shop.example\r\nX-Custom: 1\r\nConnection: keep-alive\r\n\r\n"), peer(), &target).unwrap();
    assert_eq!(String::from_utf8(body(&mut seen)).unwrap(), "HTTP/3.0 via=3 rws xff=203.0.113.9 proto=https custom=1");
    let key = format!("{:x}|localhost:{}", Arc::as_ptr(&tls) as usize, addr.port());
    assert!(pool().lock().unwrap().contains_key(&key), "one pooled connection");
    let mut missing = forward(&request("GET /nothing HTTP/1.1\r\nHost: x\r\n\r\n"), peer(), &target).unwrap();
    assert_eq!((missing.status_code, body(&mut missing)), (404, b"missing".to_vec()));
}

#[test]
fn request_bodies_go_up_whole_or_streamed() {
    let (addr, tls) = start_backend();
    let target = target(addr.port(), &tls);
    let mut echo = forward(&request("POST /echo HTTP/1.1\r\nHost: x\r\nContent-Length: 5\r\n\r\nhello"), peer(), &target).unwrap();
    assert_eq!(body(&mut echo), b"POST hello");
    let mut streamed = request("PUT /echo HTTP/1.1\r\nHost: x\r\n\r\n");
    let chunks: Vec<u8> = b"streamed ".repeat(5000);
    streamed.body = Body::stream(std::io::Cursor::new(chunks.clone()));
    let mut echo = forward(&streamed, peer(), &target).unwrap();
    assert_eq!(body(&mut echo), [b"PUT ".as_slice(), &chunks].concat());
}

#[test]
fn large_responses_stream_and_trailers_arrive() {
    let (addr, tls) = start_backend();
    let target = target(addr.port(), &tls);
    let mut big = forward(&request("GET /big HTTP/1.1\r\nHost: x\r\n\r\n"), peer(), &target).unwrap();
    assert!(big.stream_pipe.is_some(), "over the buffer limit: streamed");
    let bytes = body(&mut big);
    assert_eq!(bytes.len(), BIG);
    assert!(bytes.iter().enumerate().all(|(i, b)| *b == (i % 251) as u8));
    let mut with_trailers = forward(&request("GET /trailers HTTP/1.1\r\nHost: x\r\n\r\n"), peer(), &target).unwrap();
    assert_eq!(body(&mut with_trailers), b"with trailers");
    let trailers = with_trailers.trailers.expect("trailers").get();
    assert!(trailers.iter().any(|h| h.name == "x-checksum" && h.value == "abc"), "{trailers:?}");
}

#[test]
fn connect_failures_are_told_apart() {
    // Nothing listens on this UDP port.
    let free = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let port = free.local_addr().unwrap().port();
    drop(free);
    let (_, tls) = start_backend();
    let started = std::time::Instant::now();
    let error = forward(&request("GET / HTTP/1.1\r\nHost: x\r\n\r\n"), peer(), &target(port, &tls)).unwrap_err();
    assert!(matches!(error, H3Error::Connect(_)), "{error}");
    assert!(started.elapsed() < Duration::from_secs(4), "bounded by the connect timeout");

    // A backend whose certificate isn't trusted: the handshake fails.
    let (addr, _) = start_backend();
    let error = forward(&request("GET / HTTP/1.1\r\nHost: x\r\n\r\n"), peer(), &target(addr.port(), default_tls())).unwrap_err();
    assert!(matches!(error, H3Error::Connect(_)), "{error}");
}

#[test]
fn the_async_entry_point_answers_too() {
    let (addr, tls) = start_backend();
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let mut hello = runtime.block_on(async { forward_async(&request("GET /hello HTTP/1.1\r\nHost: x\r\n\r\n"), peer(), &target(addr.port(), &tls)).await }).unwrap();
    assert_eq!(body(&mut hello), b"hello over h3");
}

#[test]
fn a_closed_connection_is_replaced_by_a_new_one() {
    let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let cert = (certified.cert.pem(), certified.signing_key.serialize_pem(), certified.cert.der().to_vec());
    let (first, tls) = start_backend_on("127.0.0.1:0".parse().unwrap(), Some(cert.clone()));
    let addr = first.local_addr().unwrap();
    let mut hello = forward(&request("GET /hello HTTP/1.1\r\nHost: x\r\n\r\n"), peer(), &target(addr.port(), &tls)).unwrap();
    assert_eq!(body(&mut hello), b"hello over h3");
    // The backend restarts: its connections are closed, a new one listens
    // on the same port with the same certificate.
    first.close(0u32.into(), b"restart");
    std::thread::sleep(Duration::from_millis(300));
    drop(first);
    std::thread::sleep(Duration::from_millis(100));
    let (_second, _) = start_backend_on(addr, Some(cert));
    let mut again = forward(&request("GET /hello HTTP/1.1\r\nHost: x\r\n\r\n"), peer(), &target(addr.port(), &tls)).unwrap();
    assert_eq!(body(&mut again), b"hello over h3");
}
