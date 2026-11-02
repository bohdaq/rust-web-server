//! Tests of rws-server's `server` that need the facade (proxy routes,
//! `rws --check`, the admin API, ACME, the `routes!` macro).
#![allow(unused_imports, dead_code)]

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
use crate::application::Application;
use crate::server::ConnectionInfo;

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

/// Answers `uri=<uri> body=<body>` so a test can see how the request was framed.
#[derive(Clone)]
struct EchoFraming;

impl crate::application::Application for EchoFraming {
    fn execute(&self, request: &Request, _connection: &crate::server::ConnectionInfo) -> Result<crate::response::Response, String> {
        let text = format!("uri={} body={}", request.request_uri, String::from_utf8_lossy(&request.body));
        Ok(crate::response::Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, &text))
    }
}

fn websocket_echo_app() -> crate::state::AppWithState<()> {
    crate::app::App::with_state(()).websocket("/echo", |mut ws, _params, _state| {
        while let Ok(Some(crate::websocket::Message::Text(text))) = ws.recv() {
            let _ = ws.send_text(&format!("echo: {text}"));
        }
    })
}

const UPGRADE: &str = "GET /echo HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
    Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n";

#[cfg(feature = "acme")]
#[tokio::test]
async fn redirect_listener_answers_pending_acme_challenge_instead_of_redirecting() {
    crate::acme::challenges().lock().unwrap().insert("srv-tok".to_string(), "srv-tok.thumb".to_string());
    let out = redirect_round_trip("GET /.well-known/acme-challenge/srv-tok HTTP/1.1\r\nHost: example.com\r\n\r\n").await;
    crate::acme::challenges().lock().unwrap().remove("srv-tok");
    assert!(out.starts_with("HTTP/1.1 200 OK"), "{out}");
    assert!(out.ends_with("srv-tok.thumb"), "{out}");
}

