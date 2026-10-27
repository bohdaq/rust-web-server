//! `proxy_protocol = true`: the client address comes from the PROXY protocol
//! header each connection starts with. A test binary of its own: the setting
//! is read once per process and applies to every listener in it.
#![cfg(feature = "http2")]

use std::io::{Read, Write};
use std::net::SocketAddr;
use std::time::Duration;

use rust_web_server::app::App;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::server::proxy_protocol::V2_SIGNATURE;
use rust_web_server::server::Server;

/// `GET /ip` answers with the client address rws saw.
fn start() -> SocketAddr {
    std::env::set_var("RWS_CONFIG_PROXY_PROTOCOL", "true");
    assert!(rust_web_server::server::proxy_protocol::enabled());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let app = App::with_state(()).get("/ip", |_, _, connection, _| {
        Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, &format!("{}:{}", connection.client.ip, connection.client.port))
    });
    std::thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(Server::run_plain(listener, app));
    });
    addr
}

fn exchange(addr: SocketAddr, prefix: &[u8]) -> String {
    let mut stream = std::net::TcpStream::connect(addr).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
    stream.write_all(prefix).unwrap();
    stream.write_all(b"GET /ip HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").unwrap();
    let mut reply = String::new();
    let _ = stream.read_to_string(&mut reply);
    reply
}

fn v2_inet(source: [u8; 4], port: u16) -> Vec<u8> {
    let mut header = V2_SIGNATURE.to_vec();
    header.extend_from_slice(&[0x21, 0x11, 0x00, 0x0C]);
    header.extend_from_slice(&source);
    header.extend_from_slice(&[10, 0, 0, 1]);
    header.extend_from_slice(&port.to_be_bytes());
    header.extend_from_slice(&443u16.to_be_bytes());
    header
}

#[test]
fn the_client_address_comes_from_the_header() {
    let addr = start();

    let reply = exchange(addr, b"PROXY TCP4 203.0.113.7 10.0.0.1 51234 443\r\n");
    assert!(reply.ends_with("203.0.113.7:51234"), "v1: {reply}");

    let reply = exchange(addr, &v2_inet([198, 51, 100, 9], 40000));
    assert!(reply.ends_with("198.51.100.9:40000"), "v2: {reply}");

    // A load balancer's health check (LOCAL) keeps the peer address.
    let local = [V2_SIGNATURE.as_slice(), &[0x20, 0x00, 0x00, 0x00]].concat();
    let reply = exchange(addr, &local);
    assert!(reply.contains("200 OK") && reply.contains("127.0.0.1:"), "LOCAL: {reply}");

    // No header: the connection is closed without an answer.
    let reply = exchange(addr, b"");
    assert!(reply.is_empty(), "no header: {reply}");
}
