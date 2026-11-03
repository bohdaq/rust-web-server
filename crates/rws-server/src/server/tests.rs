use std::borrow::Borrow;
use std::{env, fs};
use std::fs::File;
use std::io::{BufReader, Read, Write};

use std::cmp::min;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use crate::core::New;
use crate::entry_point::config_file::override_environment_variables_from_config;
use file_ext::FileExt;

use crate::header::Header;
use crate::http::VERSION;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::Server;
use crate::symbol::SYMBOL;

pub struct MockTcpStream {
    pub read_data: Vec<u8>,
    pub write_data: Vec<u8>,
}

impl Read for MockTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let size : usize = min(self.read_data.len(), buf.len());
        buf[..size].copy_from_slice(&self.read_data[..size]);
        Ok(size)
    }
}

impl Write for MockTcpStream {

    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.write_data = Vec::from(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Unpin for MockTcpStream {}

#[test]
fn it_generates_bad_request_for_non_ut8_char_in_request() {
    // request test data
    let mut non_utf8_char: Vec<u8> = vec![255];

    let method = METHOD.get.to_lowercase();
    let request_uri = "/path";
    let http_version = VERSION.http_1_1.to_lowercase();

    let mut request_vec : Vec<u8> = vec![];
    request_vec.append(&mut method.as_bytes().to_vec());
    request_vec.append(&mut request_uri.as_bytes().to_vec());
    request_vec.append(&mut non_utf8_char);
    request_vec.append(&mut http_version.as_bytes().to_vec());

    let mock_tcp_stream = MockTcpStream {
        read_data: request_vec,
        write_data: vec![],
    };
    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream, peer_addr);
    let str = String::from_utf8(raw_response.clone()).unwrap();
    println!("{}", str);

    let response = Response::_parse_response(&raw_response);
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n400_bad_request.status_code, response.status_code);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n400_bad_request.reason_phrase, response.reason_phrase);
    assert_eq!(VERSION.http_1_1, response.http_version);

    let content_type_header = response._get_header(Header::_CONTENT_TYPE.to_string()).unwrap();
    assert_eq!(crate::error::problem::PROBLEM_JSON, content_type_header.value);

    let response_body = response.content_range_list.get(0).unwrap();
    assert_eq!(
        r#"{"type":"about:blank","title":"Bad Request","status":400,"detail":"invalid utf-8 sequence of 1 bytes from index 8"}"#,
        String::from_utf8(response_body.clone().body).unwrap()
    );

}

// ──────────────────────────────────────────────────────────────────────────────
// Keep-alive / streaming tests
// ──────────────────────────────────────────────────────────────────────────────

/// A mock stream that returns one pre-built request per `read` call, then EOF.
/// All writes are accumulated in `written`.
struct SequentialMockStream {
    requests: Vec<Vec<u8>>,
    read_index: usize,
    written: Vec<u8>,
}

impl SequentialMockStream {
    fn new(requests: Vec<Vec<u8>>) -> Self {
        SequentialMockStream { requests, read_index: 0, written: vec![] }
    }
}

impl Read for SequentialMockStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.read_index >= self.requests.len() {
            return Ok(0);
        }
        let data = &self.requests[self.read_index];
        let n = data.len().min(buf.len());
        buf[..n].copy_from_slice(&data[..n]);
        self.read_index += 1;
        Ok(n)
    }
}

impl Write for SequentialMockStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.written.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

impl Unpin for SequentialMockStream {}

fn make_connection() -> crate::server::ConnectionInfo {
    crate::server::ConnectionInfo {
        client: crate::server::Address { ip: "127.0.0.1".to_string(), port: 12345 },
        server: crate::server::Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 16000,
    sni_hostname: None,
    tls: false,
    }
}

fn raw_get_request(uri: &str, http_version: &str, extra_headers: &[(&str, &str)]) -> Vec<u8> {
    let mut req = format!("GET {} {}\r\nHost: localhost:7878\r\n", uri, http_version);
    for (name, value) in extra_headers {
        req.push_str(&format!("{}: {}\r\n", name, value));
    }
    req.push_str("\r\n");
    req.into_bytes()
}

fn count_occurrences(haystack: &[u8], needle: &[u8]) -> usize {
    haystack.windows(needle.len()).filter(|w| *w == needle).count()
}

#[test]
fn write_chunked_file_produces_valid_chunked_encoding() {
    let filepath = env::temp_dir().join(format!("rws_chunked_test_file_{}.txt", std::process::id()));
    let content = b"hello chunked world";
    std::fs::write(&filepath, content).unwrap();

    let response = Response {
        http_version: VERSION.http_1_1.to_string(),
        status_code: *STATUS_CODE_REASON_PHRASE.n200_ok.status_code,
        reason_phrase: STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string(),
        headers: vec![Header {
            name: Header::_CONTENT_TYPE.into(),
            value: MimeType::TEXT_PLAIN.into(),
        }],
        content_range_list: vec![],
        stream_file: None,
        stream_pipe: None,
        trailers: None,
    };

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: Default::default(),
    };

    let mut out: Vec<u8> = vec![];
    Server::write_chunked_file(&mut out, response, request, filepath.to_str().unwrap()).unwrap();

    let raw = String::from_utf8_lossy(&out);
    assert!(raw.contains("HTTP/1.1 200 OK"), "missing status line");
    assert!(raw.contains("Transfer-Encoding: chunked"), "missing Transfer-Encoding header");
    // 19 bytes = 0x13
    let hex_prefix = format!("{:x}\r\n", content.len());
    assert!(
        raw.contains(&hex_prefix),
        "missing chunk size '{}' in:\n{}",
        hex_prefix.trim(),
        raw
    );
    assert!(raw.contains("hello chunked world"), "missing chunk body");
    assert!(out.ends_with(b"0\r\n\r\n"), "missing terminal chunk");

    std::fs::remove_file(&filepath).ok();
}

#[test]
fn connection_info_peer_addr_parses_ipv4() {
    use crate::server::{Address, ConnectionInfo};
    let info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 8080 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 0,
    sni_hostname: None,
    tls: false,
    };
    let addr = info.peer_addr().unwrap();
    assert_eq!("127.0.0.1:8080", addr.to_string());
}

#[test]
fn connection_info_peer_addr_returns_none_for_bad_ip() {
    use crate::server::{Address, ConnectionInfo};
    let info = ConnectionInfo {
        client: Address { ip: "not-an-ip".to_string(), port: 80 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 0,
    sni_hostname: None,
    tls: false,
    };
    assert!(info.peer_addr().is_none());
}

#[test]
fn address_to_socket_addr_parses_ipv6() {
    use crate::server::Address;
    let addr = Address { ip: "::1".to_string(), port: 443 };
    let sa = addr.to_socket_addr().unwrap();
    assert_eq!(443, sa.port());
}

#[test]
fn address_to_socket_addr_returns_none_for_negative_port() {
    use crate::server::Address;
    let addr = Address { ip: "127.0.0.1".to_string(), port: -1 };
    assert!(addr.to_socket_addr().is_none());
}

// ──────────────────────────────────────────────────────────────────────────────
// Body reading: multi-read accumulation and max_body_size
// ──────────────────────────────────────────────────────────────────────────────

/// Captures the request `Server::process` dispatched to the app, via a shared
/// `Arc<Mutex<_>>` since `Server::process` takes `app` by value.
struct BodyCapturingApp {
    captured: std::sync::Arc<std::sync::Mutex<Option<Request>>>,
}

impl crate::application::Application for BodyCapturingApp {
    fn execute(&self, request: &Request, _connection: &crate::server::ConnectionInfo) -> Result<Response, String> {
        *self.captured.lock().unwrap() = Some(request.clone());
        let mut response = Response::new();
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        Ok(response)
    }
}

fn raw_post_request(body: &[u8]) -> Vec<u8> {
    let mut req = format!(
        "POST /echo HTTP/1.1\r\nHost: localhost:7878\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    ).into_bytes();
    req.extend_from_slice(body);
    req
}

#[test]
fn body_larger_than_a_single_read_is_fully_accumulated() {
    let _g = crate::test_env::lock();
    std::env::remove_var(crate::entry_point::Config::RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES);

    let body = vec![b'x'; 5000];
    let full_request = raw_post_request(&body);

    // Split the raw request into two chunks to simulate the body arriving
    // across two separate TCP reads — the exact scenario the old single-read
    // implementation could not handle.
    let split_at = full_request.len() / 2;
    let first_chunk = full_request[..split_at].to_vec();
    let second_chunk = full_request[split_at..].to_vec();

    let mut stream = SequentialMockStream::new(vec![first_chunk, second_chunk]);
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None));
    let app = BodyCapturingApp { captured: captured.clone() };
    Server::process(&mut stream, make_connection(), app).unwrap();

    let received = captured.lock().unwrap().clone().expect("app was not called");
    assert_eq!(body, received.body, "body split across two reads should be fully reassembled");
    assert!(String::from_utf8_lossy(&stream.written).contains("200"));
}

#[test]
fn get_request_body_has_no_trailing_padding() {
    let _g = crate::test_env::lock();
    std::env::remove_var(crate::entry_point::Config::RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES);

    // make_connection() sizes the read buffer at 16000 bytes; a request far
    // smaller than that must not pick up trailing zero-byte padding from the
    // unused rest of the read buffer.
    let mut stream = SequentialMockStream::new(vec![raw_get_request("/static/test.txt", VERSION.http_1_1, &[])]);
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None));
    let app = BodyCapturingApp { captured: captured.clone() };
    Server::process(&mut stream, make_connection(), app).unwrap();

    let received = captured.lock().unwrap().clone().expect("app was not called");
    assert_eq!(Vec::<u8>::new(), received.body, "GET request body should be empty, not zero-padded");
}

#[test]
fn max_body_size_rejects_oversized_declared_content_length() {
    let _g = crate::test_env::lock();
    std::env::set_var(crate::entry_point::Config::RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES, "10");

    let body = vec![b'x'; 1000];
    let mut stream = SequentialMockStream::new(vec![raw_post_request(&body)]);
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None));
    let app = BodyCapturingApp { captured: captured.clone() };
    Server::process(&mut stream, make_connection(), app).unwrap();

    std::env::remove_var(crate::entry_point::Config::RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES);

    assert!(captured.lock().unwrap().is_none(), "app should never be called for a rejected request");
    let written = String::from_utf8_lossy(&stream.written);
    assert!(written.contains("413"), "expected a 413 response, got:\n{}", written);
    assert!(written.contains("Connection: close"), "connection must close after rejecting an oversized body");
}

#[test]
fn max_body_size_allows_requests_within_the_limit() {
    let _g = crate::test_env::lock();
    std::env::set_var(crate::entry_point::Config::RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES, "10000");

    let body = vec![b'x'; 500];
    let mut stream = SequentialMockStream::new(vec![raw_post_request(&body)]);
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None));
    let app = BodyCapturingApp { captured: captured.clone() };
    Server::process(&mut stream, make_connection(), app).unwrap();

    std::env::remove_var(crate::entry_point::Config::RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES);

    let received = captured.lock().unwrap().clone().expect("app should have been called");
    assert_eq!(body, received.body);
    assert!(String::from_utf8_lossy(&stream.written).contains("200"));
}

#[test]
fn max_body_size_zero_means_unlimited() {
    let _g = crate::test_env::lock();
    std::env::set_var(crate::entry_point::Config::RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES, "0");

    let body = vec![b'x'; 20000];
    let full_request = raw_post_request(&body);
    let split_at = full_request.len() / 2;
    let mut stream = SequentialMockStream::new(vec![
        full_request[..split_at].to_vec(),
        full_request[split_at..].to_vec(),
    ]);
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None));
    let app = BodyCapturingApp { captured: captured.clone() };
    Server::process(&mut stream, make_connection(), app).unwrap();

    std::env::remove_var(crate::entry_point::Config::RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES);

    let received = captured.lock().unwrap().clone().expect("app should have been called");
    assert_eq!(body, received.body);
}

// ──────────────────────────────────────────────────────────────────────────────
// Expect: 100-continue (RFC 9110 §10.1.1)
// ──────────────────────────────────────────────────────────────────────────────

/// Builds just the request line + headers (ending in the blank line), with no
/// body bytes — simulating a 100-continue-aware client, which sends headers
/// and then waits for an interim response before sending the body.
fn raw_post_request_headers_only(content_length: usize, extra_headers: &[(&str, &str)]) -> Vec<u8> {
    let mut req = format!(
        "POST /echo HTTP/1.1\r\nHost: localhost:7878\r\nContent-Length: {}\r\nConnection: close\r\n",
        content_length
    );
    for (name, value) in extra_headers {
        req.push_str(&format!("{}: {}\r\n", name, value));
    }
    req.push_str("\r\n");
    req.into_bytes()
}

#[test]
fn continue_response_is_a_bare_100_status_line() {
    let raw = Server::continue_response();
    let text = String::from_utf8_lossy(&raw);
    assert_eq!("HTTP/1.1 100 Continue\r\n\r\n", text);
}

#[test]
fn expect_100_continue_sends_interim_response_then_reads_body() {
    let _g = crate::test_env::lock();
    std::env::remove_var(crate::entry_point::Config::RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES);

    let body = b"hello world".to_vec();
    let headers = raw_post_request_headers_only(body.len(), &[("Expect", "100-continue")]);

    // Headers arrive in one read; the body arrives in a second, separate
    // read — exactly what a real client waiting for "100 Continue" does.
    let mut stream = SequentialMockStream::new(vec![headers, body.clone()]);
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None));
    let app = BodyCapturingApp { captured: captured.clone() };
    Server::process(&mut stream, make_connection(), app).unwrap();

    let written = String::from_utf8_lossy(&stream.written).to_string();
    assert!(written.contains("HTTP/1.1 100 Continue"), "expected interim response, got:\n{}", written);
    assert!(written.contains("HTTP/1.1 200"), "expected final response, got:\n{}", written);

    let continue_pos = written.find("100 Continue").unwrap();
    let ok_pos = written.rfind("HTTP/1.1 200").unwrap();
    assert!(continue_pos < ok_pos, "100 Continue must be written before the final response");

    let received = captured.lock().unwrap().clone().expect("app should have been called");
    assert_eq!(body, received.body, "full body should still be captured after the interim response");
}

#[test]
fn no_expect_header_never_sends_100_continue() {
    // Server::process reads RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES (via get_max_body_size);
    // pre-existing gap found while stress-testing an unrelated change — a
    // neighboring test (expect_100_continue_with_oversized_body_gets_413_not_100_continue)
    // sets it to "10" under the lock, and without taking the lock here too, this
    // test could transiently see that value and get 413 instead of 200.
    let _g = crate::test_env::lock();
    std::env::remove_var(crate::entry_point::Config::RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES);

    let body = vec![b'y'; 20];
    let mut stream = SequentialMockStream::new(vec![raw_post_request(&body)]);
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None));
    let app = BodyCapturingApp { captured: captured.clone() };
    Server::process(&mut stream, make_connection(), app).unwrap();

    let written = String::from_utf8_lossy(&stream.written);
    assert!(!written.contains("100 Continue"), "must not send 100 Continue without an Expect header");
    assert!(written.contains("200"));
}

#[test]
fn expect_unsupported_value_returns_417_without_reading_body() {
    let _g = crate::test_env::lock(); // Server::process reads RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES
    let mut stream = SequentialMockStream::new(vec![
        raw_post_request_headers_only(5, &[("Expect", "gzip-trailer")]),
        b"hello".to_vec(), // never consumed — the request is rejected before this read
    ]);
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None));
    let app = BodyCapturingApp { captured: captured.clone() };
    Server::process(&mut stream, make_connection(), app).unwrap();

    assert!(captured.lock().unwrap().is_none(), "app must not run for an expectation this server can't meet");
    let written = String::from_utf8_lossy(&stream.written);
    assert!(written.contains("417"), "expected 417 Expectation Failed, got:\n{}", written);
    assert!(!written.contains("100 Continue"));
}

#[test]
fn expect_100_continue_with_oversized_body_gets_413_not_100_continue() {
    let _g = crate::test_env::lock();
    std::env::set_var(crate::entry_point::Config::RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES, "10");

    let headers = raw_post_request_headers_only(1000, &[("Expect", "100-continue")]);
    let mut stream = SequentialMockStream::new(vec![headers]);
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None));
    let app = BodyCapturingApp { captured: captured.clone() };
    Server::process(&mut stream, make_connection(), app).unwrap();

    std::env::remove_var(crate::entry_point::Config::RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES);

    assert!(captured.lock().unwrap().is_none(), "app must not run for a rejected oversized body");
    let written = String::from_utf8_lossy(&stream.written);
    assert!(written.contains("413"), "expected 413 Payload Too Large, got:\n{}", written);
    assert!(!written.contains("100 Continue"), "must not tell the client to send a body we're about to reject");
}

// ── redirect listener: dual-stack bind + ACME challenge answering ─────────────

#[cfg(feature = "http2")]
async fn redirect_round_trip(request: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        Server::handle_redirect_connection(stream, 8443).await;
    });
    let mut client = tokio::net::TcpStream::connect(addr).await.unwrap();
    client.write_all(request.as_bytes()).await.unwrap();
    let mut out = String::new();
    client.read_to_string(&mut out).await.unwrap();
    server.await.unwrap();
    out
}

#[cfg(feature = "http2")]
#[tokio::test]
async fn redirect_listener_sends_301_to_https_port() {
    let out = redirect_round_trip("GET /a/b?c=1 HTTP/1.1\r\nHost: example.com\r\n\r\n").await;
    assert!(out.starts_with("HTTP/1.1 301"), "{out}");
    assert!(out.contains("Location: https://example.com:8443/a/b?c=1\r\n"), "{out}");
}

#[cfg(feature = "http2")]
#[tokio::test]
async fn bind_all_families_wildcard_accepts_ipv4_and_ipv6_when_available() {
    // Find a free port, then bind it for every address family.
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let listeners = crate::server::bind_all_families("0.0.0.0", port).await.unwrap();
    assert!(!listeners.is_empty());
    for l in &listeners {
        assert_eq!(port, l.local_addr().unwrap().port());
    }
    assert!(tokio::net::TcpStream::connect(("127.0.0.1", port)).await.is_ok(), "IPv4 not accepted");
    if std::net::TcpListener::bind("[::1]:0").is_ok() {
        assert!(tokio::net::TcpStream::connect(("::1", port)).await.is_ok(), "IPv6 not accepted");
    }
}

#[cfg(feature = "http2")]
#[tokio::test]
async fn bind_all_families_specific_ip_binds_only_that_address() {
    let listeners = crate::server::bind_all_families("127.0.0.1", 0).await.unwrap();
    assert_eq!(1, listeners.len());
    assert!(listeners[0].local_addr().unwrap().ip().is_loopback());
}

#[test]
fn write_chunked_file_with_known_length_sends_raw_body_not_chunked() {
    let file = crate::test_env::TempPath::new("raw_body.txt");
    std::fs::write(&*file, b"hello raw").unwrap();
    let response = Response {
        http_version: VERSION.http_1_1.to_string(),
        status_code: 200,
        reason_phrase: "OK".to_string(),
        headers: vec![Header { name: Header::_CONTENT_LENGTH.into(), value: "9".into() }],
        content_range_list: vec![],
        stream_file: None,
        stream_pipe: None,
        trailers: None,
    };
    let request = Request { method: METHOD.get.to_string(), request_uri: "/".to_string(), http_version: VERSION.http_1_1.to_string(), headers: vec![], body: Default::default() };
    let mut out = Vec::new();
    Server::write_chunked_file(&mut out, response, request, file.as_str()).unwrap();
    let raw = String::from_utf8(out).unwrap();
    // Content-Length and Transfer-Encoding together are forbidden (RFC 9112 §6.1).
    assert!(!raw.to_ascii_lowercase().contains("transfer-encoding"), "{raw}");
    assert!(raw.ends_with("\r\n\r\nhello raw"), "{raw:?}");
}

// ──────────────────────────────────────────────────────────────────────────────
// Request framing: heads spanning reads, pipelining, chunked bodies, limits
// ──────────────────────────────────────────────────────────────────────────────

/// Answers `uri=<uri> body=<body>` so a test can see how the request was framed.
#[derive(Clone)]
struct EchoFraming;

impl crate::application::Application for EchoFraming {
    fn execute(&self, request: &Request, _connection: &crate::server::ConnectionInfo) -> Result<crate::response::Response, String> {
        let text = format!("uri={} body={}", request.request_uri, String::from_utf8_lossy(&request.body));
        Ok(crate::response::Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, &text))
    }
}

#[test]
fn framing_request_head_larger_than_one_read_is_served() {
    let _g = crate::test_env::lock(); // Server::process reads RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES
    // A 30 KB cookie, delivered in 4 KB reads: used to be cut off at the first read.
    let mut raw = b"GET /big HTTP/1.1\r\nHost: x\r\nCookie: ".to_vec();
    raw.extend(std::iter::repeat(b'c').take(30_000));
    raw.extend_from_slice(b"\r\nConnection: close\r\n\r\n");
    let reads: Vec<Vec<u8>> = raw.chunks(4096).map(|c| c.to_vec()).collect();
    let mut stream = SequentialMockStream::new(reads);
    Server::process(&mut stream, make_connection(), EchoFraming).unwrap();
    let written = String::from_utf8_lossy(&stream.written);
    assert!(written.starts_with("HTTP/1.1 200") && written.contains("uri=/big"), "{written}");
}

#[test]
fn framing_request_head_over_the_limit_gets_431() {
    let _g = crate::test_env::lock(); // Server::process reads RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES
    let mut raw = b"GET / HTTP/1.1\r\nCookie: ".to_vec();
    raw.extend(std::iter::repeat(b'c').take(crate::server::framing::MAX_HEAD_SIZE));
    let reads: Vec<Vec<u8>> = raw.chunks(16000).map(|c| c.to_vec()).collect();
    let mut stream = SequentialMockStream::new(reads);
    assert!(Server::process(&mut stream, make_connection(), EchoFraming).is_err());
    let written = String::from_utf8_lossy(&stream.written);
    assert!(written.starts_with("HTTP/1.1 431"), "{written}");
}

#[test]
fn framing_pipelined_requests_in_one_read_are_all_served() {
    let _g = crate::test_env::lock(); // Server::process reads RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES
    let raw = b"POST /a HTTP/1.1\r\nHost: x\r\nContent-Length: 3\r\n\r\nabcGET /b HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n".to_vec();
    let mut stream = SequentialMockStream::new(vec![raw]);
    Server::process(&mut stream, make_connection(), EchoFraming).unwrap();
    let written = String::from_utf8_lossy(&stream.written);
    assert!(written.contains("uri=/a body=abc"), "{written}");
    assert!(written.contains("uri=/b body="), "{written}");
}

#[test]
fn framing_chunked_request_body_is_decoded() {
    let _g = crate::test_env::lock(); // Server::process reads RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES
    let raw = b"POST /c HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\nhello\r\n".to_vec();
    let rest = b"6\r\n world\r\n0\r\n\r\n".to_vec();
    let mut stream = SequentialMockStream::new(vec![raw, rest]);
    Server::process(&mut stream, make_connection(), EchoFraming).unwrap();
    let written = String::from_utf8_lossy(&stream.written);
    assert!(written.contains("uri=/c body=hello world"), "{written}");
}

#[test]
fn framing_content_length_with_transfer_encoding_is_rejected() {
    let _g = crate::test_env::lock(); // Server::process reads RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES
    let raw = b"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 3\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n".to_vec();
    let mut stream = SequentialMockStream::new(vec![raw]);
    assert!(Server::process(&mut stream, make_connection(), EchoFraming).is_err());
    assert!(String::from_utf8_lossy(&stream.written).starts_with("HTTP/1.1 400"));
}

#[test]
fn framing_body_cut_short_is_not_dispatched() {
    let _g = crate::test_env::lock(); // Server::process reads RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES
    let raw = b"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 10\r\n\r\nabc".to_vec();
    let mut stream = SequentialMockStream::new(vec![raw]);
    Server::process(&mut stream, make_connection(), EchoFraming).unwrap();
    assert!(stream.written.is_empty(), "{}", String::from_utf8_lossy(&stream.written));
}

#[cfg(feature = "http2")]
#[tokio::test]
async fn the_plain_listener_serves_cleartext_http2_beside_http1() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let app = crate::app::App::with_state(()).put("/p", |_, _, _, _| {
        crate::response::Response::text(crate::response::STATUS_CODE_REASON_PHRASE.n200_ok, "put")
    });
    tokio::spawn(crate::server::Server::run_plain(listener, app));

    // Prior knowledge: the HTTP/2 preface straight away.
    let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (send, connection) = h2::client::handshake(tcp).await.unwrap();
    tokio::spawn(connection);
    let request = http::Request::get(format!("http://{addr}/healthz")).body(()).unwrap();
    let (response, _) = send.ready().await.unwrap().send_request(request, true).unwrap();
    assert_eq!(200, response.await.unwrap().status().as_u16());

    // HTTP/1.1 on the same listener, including a method starting with 'P'.
    let replies = tokio::task::spawn_blocking(move || {
        let mut replies = Vec::new();
        for request in ["GET /healthz HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n", "PUT /p HTTP/1.1\r\nHost: x\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"] {
            let mut stream = std::net::TcpStream::connect(addr).unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
            stream.write_all(request.as_bytes()).unwrap();
            let mut reply = String::new();
            let _ = stream.read_to_string(&mut reply);
            replies.push(reply);
        }
        replies
    })
    .await
    .unwrap();
    assert!(replies[0].starts_with("HTTP/1.1 200"), "{}", replies[0]);
    assert!(replies[1].starts_with("HTTP/1.1 200") && replies[1].ends_with("put"), "{}", replies[1]);
}

// ── WebSocket hand-over (sync loop) ───────────────────────────────────────────

fn websocket_echo_app() -> crate::state::AppWithState<()> {
    crate::app::App::with_state(()).websocket("/echo", |mut ws, _params, _state| {
        while let Ok(Some(crate::websocket::Message::Text(text))) = ws.recv() {
            let _ = ws.send_text(&format!("echo: {text}"));
        }
    })
}

const UPGRADE: &str = "GET /echo HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
    Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n";

#[test]
fn the_sync_loop_hands_an_upgraded_socket_to_the_handler() {
    let _g = crate::test_env::lock(); // Server::process reads RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        Server::process_upgradable(stream, make_connection(), websocket_echo_app(), |s| {
            Some((Box::new(s.try_clone().ok()?) as Box<dyn Read + Send>, Box::new(s.try_clone().ok()?) as Box<dyn Write + Send>))
        })
    });
    let mut client = std::net::TcpStream::connect(addr).unwrap();
    client.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
    // The first frame in the same write as the request: it must reach the handler.
    let mask = [1u8, 2, 3, 4];
    let mut bytes = UPGRADE.as_bytes().to_vec();
    bytes.extend([0x81, 0x80 | 2]);
    bytes.extend(mask);
    bytes.extend(b"hi".iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    client.write_all(&bytes).unwrap();
    server.join().unwrap().unwrap();
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        let mut byte = [0u8; 1];
        client.read_exact(&mut byte).unwrap();
        head.push(byte[0]);
    }
    let head = String::from_utf8(head).unwrap();
    assert!(head.starts_with("HTTP/1.1 101 Switching Protocols\r\n"), "{head}");
    assert!(head.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n"), "{head}");
    assert!(!head.to_ascii_lowercase().contains("x-rws-websocket-upgrade"), "the internal header isn't sent: {head}");
    assert_eq!(crate::websocket::Frame::Text("echo: hi".to_string()), crate::websocket::WebSocket::read_frame(&mut client).unwrap());
}

#[test]
fn server_process_without_a_socket_to_hand_over_refuses_the_upgrade() {
    let _g = crate::test_env::lock();
    let mut stream = MockTcpStream { read_data: UPGRADE.as_bytes().to_vec(), write_data: vec![] };
    assert!(Server::process(&mut stream, make_connection(), websocket_echo_app()).is_err());
    assert!(String::from_utf8_lossy(&stream.write_data).starts_with("HTTP/1.1 500 "));
}


// ---- Server::pipe_stream: framing of proxied streamed bodies over HTTP/1.1 ----

fn piped_response(headers: &[(&str, &str)]) -> Response {
    Response {
        http_version: VERSION.http_1_1.to_string(),
        status_code: *STATUS_CODE_REASON_PHRASE.n200_ok.status_code,
        reason_phrase: STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string(),
        headers: headers.iter().map(|(n, v)| Header::new(n.to_string(), v.to_string())).collect(),
        content_range_list: vec![],
        stream_file: None,
        stream_pipe: None,
        trailers: None,
    }
}

fn piped_request(method: &str) -> Request {
    Request {
        method: method.to_string(),
        request_uri: "/".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: Default::default(),
    }
}

/// Yields `data` in reads of at most `step` bytes, then fails if `fail_at_end`.
struct StepReader {
    data: Vec<u8>,
    pos: usize,
    step: usize,
    fail_at_end: bool,
}

impl Read for StepReader {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.data.len() {
            return if self.fail_at_end {
                Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "backend stalled"))
            } else {
                Ok(0)
            };
        }
        let n = self.step.min(out.len()).min(self.data.len() - self.pos);
        out[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

fn step_reader(data: &[u8], step: usize, fail_at_end: bool) -> Box<dyn Read + Send> {
    Box::new(StepReader { data: data.to_vec(), pos: 0, step, fail_at_end })
}

fn split_head(out: &[u8]) -> (String, &[u8]) {
    let end = out.windows(4).position(|w| w == b"\r\n\r\n").expect("no head") + 4;
    (String::from_utf8_lossy(&out[..end]).to_string(), &out[end..])
}

/// Decodes a chunked body strictly: every chunk followed by CRLF, a final
/// zero-size chunk and nothing after it. Returns the data and chunk sizes.
fn decode_chunked_strict(mut body: &[u8]) -> (Vec<u8>, Vec<usize>) {
    let mut data = vec![];
    let mut sizes = vec![];
    loop {
        let eol = body.windows(2).position(|w| w == b"\r\n").expect("chunk size line");
        let size = usize::from_str_radix(std::str::from_utf8(&body[..eol]).unwrap(), 16).unwrap();
        body = &body[eol + 2..];
        if size == 0 {
            assert_eq!(body, b"\r\n", "no clean end after the last chunk");
            return (data, sizes);
        }
        data.extend_from_slice(&body[..size]);
        assert_eq!(&body[size..size + 2], b"\r\n", "chunk not followed by CRLF");
        sizes.push(size);
        body = &body[size + 2..];
    }
}

fn large_body() -> Vec<u8> {
    (0..300_000u32).map(|i| (i % 251) as u8).collect()
}

#[test]
fn pipe_stream_keeps_an_upstream_content_length() {
    let body = large_body();
    let len = body.len().to_string();
    let response = piped_response(&[("Content-Type", "application/octet-stream"), ("Content-Length", &len)]);
    let mut out = vec![];
    Server::pipe_stream(&mut out, response, piped_request(METHOD.get), step_reader(&body, 100_000, false)).unwrap();
    let (head, rest) = split_head(&out);
    assert!(head.contains(&format!("Content-Length: {}\r\n", len)), "{head}");
    assert!(!head.to_ascii_lowercase().contains("transfer-encoding"), "{head}");
    assert_eq!(rest, &body[..], "the body goes out unframed and complete");
}

#[test]
fn pipe_stream_fails_a_body_shorter_than_its_content_length() {
    let body = large_body();
    let response = piped_response(&[("Content-Length", "400000")]);
    let mut out = vec![];
    let err = Server::pipe_stream(&mut out, response, piped_request(METHOD.get), step_reader(&body, 65_536, false))
        .unwrap_err();
    assert!(err.contains("300000 of 400000"), "{err}");
    let (_, rest) = split_head(&out);
    assert_eq!(rest.len(), body.len(), "only what arrived is written");
}

#[test]
fn pipe_stream_never_writes_more_than_its_content_length() {
    let body = large_body();
    let response = piped_response(&[("Content-Length", "1000")]);
    let mut out = vec![];
    Server::pipe_stream(&mut out, response, piped_request(METHOD.get), step_reader(&body, 65_536, false)).unwrap();
    let (_, rest) = split_head(&out);
    assert_eq!(rest, &body[..1000]);
}

#[test]
fn pipe_stream_read_error_with_a_content_length_is_an_error() {
    let body = large_body();
    let len = (body.len() + 10).to_string();
    let response = piped_response(&[("Content-Length", &len)]);
    let mut out = vec![];
    let err = Server::pipe_stream(&mut out, response, piped_request(METHOD.get), step_reader(&body, 65_536, true))
        .unwrap_err();
    assert!(err.contains("backend stalled"), "{err}");
}

#[test]
fn pipe_stream_without_a_length_is_chunked_in_large_frames() {
    let body = large_body();
    let response = piped_response(&[("Content-Type", "application/octet-stream")]);
    let mut out = vec![];
    Server::pipe_stream(&mut out, response, piped_request(METHOD.get), step_reader(&body, 1 << 20, false)).unwrap();
    let (head, rest) = split_head(&out);
    assert!(head.contains("Transfer-Encoding: chunked\r\n"), "{head}");
    assert!(!head.to_ascii_lowercase().contains("content-length"), "{head}");
    let (data, sizes) = decode_chunked_strict(rest);
    assert_eq!(data, body);
    assert_eq!(sizes[0], crate::server::PIPE_CHUNK, "{sizes:?}");
}

#[test]
fn pipe_stream_read_error_never_ends_the_chunked_body() {
    // An SSE-like pipe that breaks: the client must not see a clean end.
    let response = piped_response(&[("Content-Type", "text/event-stream")]);
    let mut out = vec![];
    let err = Server::pipe_stream(&mut out, response, piped_request(METHOD.get), step_reader(b"data: 1\n\n", 64, true))
        .unwrap_err();
    assert!(err.contains("backend stalled"), "{err}");
    assert!(!out.ends_with(b"0\r\n\r\n"), "a terminating chunk was written");
    let (_, rest) = split_head(&out);
    assert_eq!(rest, b"9\r\ndata: 1\n\n\r\n");
}

#[test]
fn pipe_stream_conflicting_content_lengths_go_chunked() {
    let response = piped_response(&[("Content-Length", "5"), ("Content-Length", "6")]);
    let mut out = vec![];
    Server::pipe_stream(&mut out, response, piped_request(METHOD.get), step_reader(b"hello!", 64, false)).unwrap();
    let (head, rest) = split_head(&out);
    assert!(head.contains("Transfer-Encoding: chunked\r\n"), "{head}");
    assert!(!head.to_ascii_lowercase().contains("content-length"), "{head}");
    assert_eq!(decode_chunked_strict(rest).0, b"hello!");
}

#[test]
fn pipe_stream_reframes_an_upstream_chunked_body() {
    let framed = b"5\r\nhello\r\n7\r\n, world\r\n0\r\n\r\n";
    let response = piped_response(&[("Transfer-Encoding", "chunked")]);
    let mut out = vec![];
    Server::pipe_stream(&mut out, response, piped_request(METHOD.get), step_reader(framed, 3, false)).unwrap();
    let (head, rest) = split_head(&out);
    assert_eq!(head.matches("Transfer-Encoding").count(), 1, "{head}");
    assert_eq!(decode_chunked_strict(rest).0, b"hello, world");
}

#[test]
fn pipe_stream_chunked_body_ends_at_the_last_chunk() {
    // After its last chunk the upstream's keep-alive connection has nothing
    // more; reading on would wait for the read timeout (an error here).
    let framed = b"5\r\nhello\r\n0\r\n\r\n";
    let response = piped_response(&[("Transfer-Encoding", "chunked")]);
    let mut out = vec![];
    Server::pipe_stream(&mut out, response, piped_request(METHOD.get), step_reader(framed, 64, true)).unwrap();
    let (_, rest) = split_head(&out);
    assert_eq!(decode_chunked_strict(rest).0, b"hello");
}

#[test]
fn pipe_stream_content_length_body_ends_without_reading_past_it() {
    let response = piped_response(&[("Content-Length", "5")]);
    let mut out = vec![];
    Server::pipe_stream(&mut out, response, piped_request(METHOD.get), step_reader(b"hello", 64, true)).unwrap();
    assert_eq!(split_head(&out).1, b"hello");
}

#[test]
fn pipe_stream_head_keeps_the_length_and_sends_no_body() {
    let response = piped_response(&[("Content-Length", "300000")]);
    let mut out = vec![];
    Server::pipe_stream(&mut out, response, piped_request(METHOD.head), step_reader(&large_body(), 64, false)).unwrap();
    let (head, rest) = split_head(&out);
    assert!(head.contains("Content-Length: 300000\r\n"), "{head}");
    assert!(rest.is_empty());
}
