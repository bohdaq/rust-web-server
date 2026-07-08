use super::discover;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

/// Spawns a one-shot mock Consul agent: accepts one connection, replies with
/// `status_line` + `response_body`, then closes. Mirrors the mock-server
/// pattern already used for `S3Storage` in `src/storage/s3/tests.rs`.
fn spawn_mock_consul(status_line: &'static str, response_body: &'static str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock Consul server");
    let port = listener.local_addr().unwrap().port();

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf); // discard the request

            let resp = format!(
                "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });

    port
}

#[test]
fn discovers_healthy_instances_with_service_address() {
    let body = r#"[
        {
            "Node": {"Address": "10.0.0.9"},
            "Service": {"Address": "10.0.0.5", "Port": 8080}
        },
        {
            "Node": {"Address": "10.0.0.10"},
            "Service": {"Address": "10.0.0.6", "Port": 9090}
        }
    ]"#;
    let port = spawn_mock_consul("HTTP/1.1 200 OK", body);

    let backends = discover(&format!("127.0.0.1:{}", port), "api");
    assert_eq!(2, backends.len(), "got {:?}", backends);
    assert!(backends.contains(&"10.0.0.5:8080".to_string()));
    assert!(backends.contains(&"10.0.0.6:9090".to_string()));
}

#[test]
fn falls_back_to_node_address_when_service_address_empty() {
    let body = r#"[
        {"Node": {"Address": "10.0.0.11"}, "Service": {"Address": "", "Port": 7070}}
    ]"#;
    let port = spawn_mock_consul("HTTP/1.1 200 OK", body);

    let backends = discover(&format!("127.0.0.1:{}", port), "api");
    assert_eq!(vec!["10.0.0.11:7070".to_string()], backends);
}

#[test]
fn returns_empty_on_non_success_status() {
    let port = spawn_mock_consul("HTTP/1.1 500 Internal Server Error", "oops");
    let backends = discover(&format!("127.0.0.1:{}", port), "api");
    assert!(backends.is_empty());
}

#[test]
fn returns_empty_on_malformed_json() {
    let port = spawn_mock_consul("HTTP/1.1 200 OK", "not json");
    let backends = discover(&format!("127.0.0.1:{}", port), "api");
    assert!(backends.is_empty());
}

#[test]
fn returns_empty_when_connection_fails() {
    // Port 0 with no listener behind it — connection should fail immediately.
    let backends = discover("127.0.0.1:1", "api");
    assert!(backends.is_empty());
}

#[test]
fn returns_empty_for_empty_array() {
    let port = spawn_mock_consul("HTTP/1.1 200 OK", "[]");
    let backends = discover(&format!("127.0.0.1:{}", port), "api");
    assert!(backends.is_empty());
}
