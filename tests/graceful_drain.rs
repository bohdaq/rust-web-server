//! A request that is being handled when shutdown starts still gets its
//! answer: the accept loop stops, then waits for handlers (18.35.0). A test
//! binary of its own, as `request_shutdown` can't be undone.
#![cfg(feature = "http2")]

use std::io::{Read, Write};
use std::time::{Duration, Instant};

use rust_web_server::app::App;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::server::{request_shutdown, Server};

fn get(addr: std::net::SocketAddr, path: &str) -> String {
    let mut stream = std::net::TcpStream::connect(addr).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    stream.write_all(format!("GET {path} HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n").as_bytes()).unwrap();
    let mut reply = String::new();
    let _ = stream.read_to_string(&mut reply);
    reply
}

#[test]
fn in_flight_requests_finish_before_the_server_returns() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let app = App::with_state(()).get("/slow", |_, _, _, _| {
        std::thread::sleep(Duration::from_millis(800));
        Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "done")
    });
    tokio::runtime::Runtime::new().unwrap().block_on(async move {
        let server = tokio::spawn(Server::run_plain(listener, app));
        // A library app is ready once it serves (before 18.35 only the rws
        // binary said so, and /readyz stayed 503).
        let ready = tokio::task::spawn_blocking(move || get(addr, "/readyz")).await.unwrap();
        assert!(ready.starts_with("HTTP/1.1 200"), "{ready}");
        let slow = tokio::task::spawn_blocking(move || get(addr, "/slow"));
        tokio::time::sleep(Duration::from_millis(200)).await;
        let started = Instant::now();
        request_shutdown();
        tokio::time::timeout(Duration::from_secs(10), server).await.expect("run_plain returns").unwrap();
        assert!(started.elapsed() >= Duration::from_millis(400), "it waited for the slow handler: {:?}", started.elapsed());
        let reply = slow.await.unwrap();
        assert!(reply.starts_with("HTTP/1.1 200") && reply.ends_with("done"), "{reply}");
    });
}
