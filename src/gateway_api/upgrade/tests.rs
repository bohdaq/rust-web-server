use super::*;
use std::sync::{Arc, Mutex};

fn request(headers: &[(&str, &str)]) -> Request {
    Request {
        method: "GET".to_string(),
        request_uri: "/ws?x=1".to_string(),
        http_version: "HTTP/1.1".to_string(),
        headers: headers.iter().map(|(n, v)| Header::new(n.to_string(), v.to_string())).collect(),
        body: Default::default(),
    }
}

fn upgrade_request() -> Request {
    request(&[("Host", "example.org"), ("Connection", "keep-alive, Upgrade"), ("Upgrade", "websocket"), ("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")])
}

/// A backend that reads one head, answers `reply`, then echoes until EOF.
fn backend(reply: &'static str) -> (u16, std::thread::JoinHandle<String>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap_or(0) == 1 {
            head.push(byte[0]);
        }
        stream.write_all(reply.as_bytes()).unwrap();
        let mut copy = stream.try_clone().unwrap();
        let _ = std::io::copy(&mut stream, &mut copy);
        String::from_utf8_lossy(&head).to_string()
    });
    (port, handle)
}

#[derive(Clone, Default)]
struct Shared(Arc<Mutex<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn upgrade_requests_are_recognized() {
    assert!(is_upgrade(&upgrade_request()));
    assert!(!is_upgrade(&request(&[("Upgrade", "websocket")])), "needs Connection: upgrade");
    assert!(!is_upgrade(&request(&[("Connection", "upgrade")])), "needs Upgrade");
    let head = String::from_utf8(request_head(&upgrade_request(), "10.0.0.9", "example.org")).unwrap();
    assert!(head.starts_with("GET /ws?x=1 HTTP/1.1\r\nHost: example.org\r\n"), "{head}");
    assert!(head.contains("Upgrade: websocket\r\n") && head.contains("X-Forwarded-For: 10.0.0.9\r\n") && head.ends_with("\r\n\r\n"));
    assert_eq!(1, head.matches("Host:").count());
}

#[test]
fn a_101_hands_the_connection_to_a_relay() {
    let (port, backend) = backend("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\nearly");
    let mut response = forward(&upgrade_request(), "10.0.0.9", "127.0.0.1", port, "example.org", Duration::from_secs(5)).unwrap();
    assert_eq!(101, response.status_code);
    assert!(response.headers.iter().any(|h| h.name == "Upgrade" && h.value == "websocket"));
    let upgrade = crate::websocket::take_upgrade(&mut response).expect("a relay is waiting");
    let written = Shared::default();
    upgrade.run(Box::new(std::io::Cursor::new(b"hello".to_vec())), Box::new(written.clone()));
    assert_eq!(b"earlyhello".to_vec(), *written.0.lock().unwrap(), "bytes after the 101, then the echo");
    assert!(backend.join().unwrap().contains("Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n"));
}

#[test]
fn a_refused_upgrade_is_an_ordinary_response() {
    let (port, _backend) = backend("HTTP/1.1 403 Forbidden\r\nContent-Length: 4\r\n\r\nnope");
    let mut response = forward(&upgrade_request(), "10.0.0.9", "127.0.0.1", port, "example.org", Duration::from_secs(5)).unwrap();
    assert_eq!(403, response.status_code);
    assert!(crate::websocket::take_upgrade(&mut response).is_none(), "no relay registered");
    assert!(forward(&upgrade_request(), "10.0.0.9", "127.0.0.1", 1, "example.org", Duration::from_millis(200)).is_err(), "nothing listening");
}
