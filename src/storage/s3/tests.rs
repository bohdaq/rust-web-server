use super::{S3Config, S3Storage};
use crate::storage::Storage;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

// ── mock S3 server ───────────────────────────────────────────────────────────

struct CapturedRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Spawns a one-shot mock S3 endpoint: accepts one connection, captures the
/// request line/headers/body, replies with `status_line` + `response_body`,
/// then closes.
fn spawn_mock_s3(
    status_line: &'static str,
    response_body: &'static [u8],
) -> (u16, Arc<Mutex<Option<CapturedRequest>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock S3 server");
    let port = listener.local_addr().unwrap().port();
    let captured = Arc::new(Mutex::new(None));
    let captured2 = Arc::clone(&captured);

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut data = Vec::new();
            let mut buf = [0u8; 8192];
            let header_end = loop {
                let n = stream.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    return;
                }
                data.extend_from_slice(&buf[..n]);
                if let Some(pos) = find_subslice(&data, b"\r\n\r\n") {
                    break pos + 4;
                }
            };

            let header_str = String::from_utf8_lossy(&data[..header_end]).to_string();
            let mut lines = header_str.lines();
            let request_line = lines.next().unwrap_or("").to_string();
            let mut rl_parts = request_line.split_whitespace();
            let method = rl_parts.next().unwrap_or("").to_string();
            let path = rl_parts.next().unwrap_or("").to_string();

            let mut headers = Vec::new();
            let mut content_length = 0usize;
            for line in lines {
                if let Some((k, v)) = line.split_once(':') {
                    let k = k.trim().to_string();
                    let v = v.trim().to_string();
                    if k.eq_ignore_ascii_case("content-length") {
                        content_length = v.parse().unwrap_or(0);
                    }
                    headers.push((k, v));
                }
            }

            while data.len() < header_end + content_length {
                let n = stream.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break;
                }
                data.extend_from_slice(&buf[..n]);
            }
            let available = data.len().saturating_sub(header_end);
            let body = data[header_end..header_end + content_length.min(available)].to_vec();

            *captured2.lock().unwrap() = Some(CapturedRequest { method, path, headers, body });

            let resp = format!(
                "{status_line}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response_body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.write_all(response_body);
        }
    });

    (port, captured)
}

fn test_config(port: u16) -> S3Config {
    S3Config {
        bucket: "test-bucket".to_string(),
        region: "us-east-1".to_string(),
        access_key: "AKIAIOSFODNN7EXAMPLE".to_string(),
        secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
        endpoint: format!("http://127.0.0.1:{port}"),
    }
}

fn header<'a>(req: &'a CapturedRequest, name: &str) -> Option<&'a str> {
    req.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

// ── tests ────────────────────────────────────────────────────────────────────

#[test]
fn put_sends_signed_request_with_correct_path_and_body() {
    let (port, captured) = spawn_mock_s3("HTTP/1.1 200 OK", b"");
    let store = S3Storage::new(test_config(port));

    let key = store.put("uploads/photo.png", b"binary-data", "image/png").unwrap();
    assert_eq!("uploads/photo.png", key);

    let req = captured.lock().unwrap().take().unwrap();
    assert_eq!("PUT", req.method);
    assert_eq!("/test-bucket/uploads/photo.png", req.path);
    assert_eq!(b"binary-data".to_vec(), req.body);

    assert!(header(&req, "Authorization").unwrap().starts_with(
        "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/"
    ));
    assert!(header(&req, "x-amz-date").is_some());
    assert!(header(&req, "x-amz-content-sha256").is_some());
    assert_eq!(Some("image/png"), header(&req, "Content-Type"));

    // Exactly one Host header — no duplicate from the signed header list.
    let host_count = req.headers.iter().filter(|(k, _)| k.eq_ignore_ascii_case("host")).count();
    assert_eq!(1, host_count);
}

#[test]
fn get_returns_body_on_success() {
    let (port, _captured) = spawn_mock_s3("HTTP/1.1 200 OK", b"file contents");
    let store = S3Storage::new(test_config(port));
    let bytes = store.get("uploads/photo.png").unwrap();
    assert_eq!(b"file contents".to_vec(), bytes);
}

#[test]
fn get_returns_error_on_404() {
    let (port, _captured) = spawn_mock_s3("HTTP/1.1 404 Not Found", b"NoSuchKey");
    let store = S3Storage::new(test_config(port));
    let err = store.get("missing.png").unwrap_err();
    assert!(err.to_string().contains("404"));
}

#[test]
fn delete_sends_delete_method() {
    let (port, captured) = spawn_mock_s3("HTTP/1.1 204 No Content", b"");
    let store = S3Storage::new(test_config(port));
    store.delete("uploads/photo.png").unwrap();
    let req = captured.lock().unwrap().take().unwrap();
    assert_eq!("DELETE", req.method);
    assert_eq!("/test-bucket/uploads/photo.png", req.path);
}

#[test]
fn url_uses_path_style_addressing() {
    let store = S3Storage::new(test_config(9999));
    assert_eq!("http://127.0.0.1:9999/test-bucket/uploads/photo.png", store.url("uploads/photo.png"));
}

#[test]
fn key_with_special_characters_is_percent_encoded_in_path() {
    let (port, captured) = spawn_mock_s3("HTTP/1.1 200 OK", b"");
    let store = S3Storage::new(test_config(port));
    store.put("a file.txt", b"x", "text/plain").unwrap();
    let req = captured.lock().unwrap().take().unwrap();
    assert_eq!("/test-bucket/a%20file.txt", req.path);
}

// ── S3Config::from_env ───────────────────────────────────────────────────────

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn from_env_requires_bucket_and_credentials() {
    let _g = env_lock().lock().unwrap();
    std::env::remove_var("RWS_S3_BUCKET");
    std::env::remove_var("RWS_S3_ACCESS_KEY");
    std::env::remove_var("RWS_S3_SECRET_KEY");
    assert!(S3Config::from_env().is_err());
}

#[test]
fn from_env_applies_defaults() {
    let _g = env_lock().lock().unwrap();
    std::env::set_var("RWS_S3_BUCKET", "my-bucket");
    std::env::set_var("RWS_S3_ACCESS_KEY", "AK");
    std::env::set_var("RWS_S3_SECRET_KEY", "SK");
    std::env::remove_var("RWS_S3_REGION");
    std::env::remove_var("RWS_S3_ENDPOINT");

    let cfg = S3Config::from_env().unwrap();
    assert_eq!("my-bucket", cfg.bucket);
    assert_eq!("us-east-1", cfg.region);
    assert_eq!("https://s3.us-east-1.amazonaws.com", cfg.endpoint);

    std::env::remove_var("RWS_S3_BUCKET");
    std::env::remove_var("RWS_S3_ACCESS_KEY");
    std::env::remove_var("RWS_S3_SECRET_KEY");
}

#[test]
fn from_env_respects_custom_endpoint_and_region() {
    let _g = env_lock().lock().unwrap();
    std::env::set_var("RWS_S3_BUCKET", "my-bucket");
    std::env::set_var("RWS_S3_ACCESS_KEY", "AK");
    std::env::set_var("RWS_S3_SECRET_KEY", "SK");
    std::env::set_var("RWS_S3_REGION", "eu-west-1");
    std::env::set_var("RWS_S3_ENDPOINT", "https://accountid.r2.cloudflarestorage.com");

    let cfg = S3Config::from_env().unwrap();
    assert_eq!("eu-west-1", cfg.region);
    assert_eq!("https://accountid.r2.cloudflarestorage.com", cfg.endpoint);

    std::env::remove_var("RWS_S3_BUCKET");
    std::env::remove_var("RWS_S3_ACCESS_KEY");
    std::env::remove_var("RWS_S3_SECRET_KEY");
    std::env::remove_var("RWS_S3_REGION");
    std::env::remove_var("RWS_S3_ENDPOINT");
}
