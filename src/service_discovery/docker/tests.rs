use super::*;

// ── parse_containers (pure JSON, no socket needed) ────────────────────────────

#[test]
fn parse_containers_extracts_label_value_as_backend() {
    let body = r#"[
        {"Id": "abc", "Labels": {"rws.backend": "10.0.0.5:8080", "other": "x"}},
        {"Id": "def", "Labels": {"rws.backend": "10.0.0.6:9090"}}
    ]"#;
    let backends = parse_containers(body, "rws.backend");
    assert_eq!(2, backends.len(), "got {:?}", backends);
    assert!(backends.contains(&"10.0.0.5:8080".to_string()));
    assert!(backends.contains(&"10.0.0.6:9090".to_string()));
}

#[test]
fn parse_containers_skips_containers_without_the_label() {
    let body = r#"[
        {"Id": "abc", "Labels": {"other": "x"}},
        {"Id": "def", "Labels": {"rws.backend": "10.0.0.6:9090"}}
    ]"#;
    let backends = parse_containers(body, "rws.backend");
    assert_eq!(vec!["10.0.0.6:9090".to_string()], backends);
}

#[test]
fn parse_containers_skips_empty_label_value() {
    let body = r#"[{"Id": "abc", "Labels": {"rws.backend": ""}}]"#;
    assert!(parse_containers(body, "rws.backend").is_empty());
}

#[test]
fn parse_containers_returns_empty_on_malformed_json() {
    assert!(parse_containers("not json", "rws.backend").is_empty());
}

#[test]
fn parse_containers_returns_empty_when_not_an_array() {
    assert!(parse_containers(r#"{"a": 1}"#, "rws.backend").is_empty());
}

// ── decode_chunked ────────────────────────────────────────────────────────────

#[test]
fn decode_chunked_reassembles_body() {
    let chunked = b"5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n";
    let decoded = decode_chunked(chunked);
    assert_eq!(b"hello world".to_vec(), decoded);
}

#[test]
fn decode_chunked_empty_body() {
    let chunked = b"0\r\n\r\n";
    assert!(decode_chunked(chunked).is_empty());
}

// ── parse_http_response ───────────────────────────────────────────────────────

#[test]
fn parse_http_response_content_length_style() {
    let body = r#"[{"Id":"abc","Labels":{"rws.backend":"10.0.0.5:8080"}}]"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    let backends = parse_http_response(response.as_bytes(), "rws.backend");
    assert_eq!(vec!["10.0.0.5:8080".to_string()], backends);
}

#[test]
fn parse_http_response_chunked_style() {
    let body = r#"[{"Id":"abc","Labels":{"rws.backend":"10.0.0.5:8080"}}]"#;
    let chunked_body = format!("{:x}\r\n{}\r\n0\r\n\r\n", body.len(), body);
    let response = format!(
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{}",
        chunked_body
    );
    let backends = parse_http_response(response.as_bytes(), "rws.backend");
    assert_eq!(vec!["10.0.0.5:8080".to_string()], backends);
}

#[test]
fn parse_http_response_non_success_status_returns_empty() {
    let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
    assert!(parse_http_response(response.as_bytes(), "rws.backend").is_empty());
}

#[test]
fn parse_http_response_malformed_returns_empty() {
    assert!(parse_http_response(b"not an http response", "rws.backend").is_empty());
}

// ── discover() over a real Unix socket ────────────────────────────────────────

#[cfg(unix)]
#[test]
fn discover_queries_mock_docker_socket() {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;

    let socket_path = std::env::temp_dir().join(format!("rws-test-docker-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path).expect("bind mock Docker socket");

    let body = r#"[{"Id":"abc","Labels":{"rws.backend":"10.0.0.5:8080"}}]"#;
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });

    let backends = discover(socket_path.to_str().unwrap(), "rws.backend");
    let _ = std::fs::remove_file(&socket_path);
    assert_eq!(vec!["10.0.0.5:8080".to_string()], backends);
}

#[cfg(unix)]
#[test]
fn discover_returns_empty_when_socket_does_not_exist() {
    let backends = discover("/nonexistent/path/docker.sock", "rws.backend");
    assert!(backends.is_empty());
}
