//! `server::request_shutdown` ends the accept loops as SIGTERM does. A test
//! binary of its own: the request is process-wide and can't be undone, so in
//! the library's tests it would stop every other test's server.
#![cfg(feature = "http2")]

use std::io::{Read, Write};
use std::time::Duration;

use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::server::{request_shutdown, Server};

fn get_healthz(addr: std::net::SocketAddr) -> String {
    let mut stream = std::net::TcpStream::connect(addr).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    stream.write_all(b"GET /healthz HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n").unwrap();
    let mut reply = String::new();
    let _ = stream.read_to_string(&mut reply);
    reply
}

#[test]
fn request_shutdown_ends_run_plain() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::runtime::Runtime::new().unwrap().block_on(async move {
        let server = tokio::spawn(Server::run_plain(listener, App::new()));
        let reply = tokio::task::spawn_blocking(move || get_healthz(addr)).await.unwrap();
        assert!(reply.starts_with("HTTP/1.1 200"), "server should be serving: {reply}");

        request_shutdown();
        tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .expect("run_plain should return after request_shutdown")
            .unwrap();
        assert!(std::net::TcpStream::connect(addr).is_err(), "the listener should be closed");
    });
}
