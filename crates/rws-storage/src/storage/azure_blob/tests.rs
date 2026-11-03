use super::{AzureBlobConfig, AzureBlobStorage};
use crate::storage::Storage;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

// ── mock Azure Blob server (same pattern as storage::s3::tests) ────────────

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

fn spawn_mock_blob(status_line: &'static str, response_body: &'static [u8]) -> (u16, Arc<Mutex<Option<CapturedRequest>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock blob server");
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

fn test_config(port: u16) -> AzureBlobConfig {
    AzureBlobConfig {
        account: "myaccount".to_string(),
        container: "mycontainer".to_string(),
        account_key: "YWNjb3VudGtleQ==".to_string(), // base64("accountkey")
        endpoint: format!("http://127.0.0.1:{port}"),
    }
}

fn header<'a>(req: &'a CapturedRequest, name: &str) -> Option<&'a str> {
    req.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

// ── tests ────────────────────────────────────────────────────────────────────

#[test]
fn put_sends_signed_request_with_correct_path_and_body() {
    // RWS_AZURE_CREDENTIAL_SOURCE is a process-wide override that takes
    // priority over static keys (see azure_credentials::AzureCredentialsProvider::
    // detect), so any test constructing an AzureBlobStorage — even with a
    // static key — races with tests that set it unless they share this lock
    // (same lesson learned from the equivalent AWS S3Storage tests).
    let _g = crate::storage::azure_credentials::credential_env_lock().lock().unwrap();
    let (port, captured) = spawn_mock_blob("HTTP/1.1 201 Created", b"");
    let store = AzureBlobStorage::new(test_config(port));

    let key = store.put("uploads/photo.png", b"binary-data", "image/png").unwrap();
    assert_eq!("uploads/photo.png", key);

    let req = captured.lock().unwrap().take().unwrap();
    assert_eq!("PUT", req.method);
    assert_eq!("/mycontainer/uploads/photo.png", req.path);
    assert_eq!(b"binary-data".to_vec(), req.body);

    assert!(header(&req, "Authorization").unwrap().starts_with("SharedKey myaccount:"));
    assert_eq!(Some("BlockBlob"), header(&req, "x-ms-blob-type"));
    assert!(header(&req, "x-ms-date").is_some());
    assert!(header(&req, "x-ms-version").is_some());
    assert_eq!(Some("image/png"), header(&req, "Content-Type"));
}

#[test]
fn get_returns_body_on_success() {
    let _g = crate::storage::azure_credentials::credential_env_lock().lock().unwrap();
    let (port, _captured) = spawn_mock_blob("HTTP/1.1 200 OK", b"file contents");
    let store = AzureBlobStorage::new(test_config(port));
    let bytes = store.get("uploads/photo.png").unwrap();
    assert_eq!(b"file contents".to_vec(), bytes);
}

#[test]
fn get_returns_error_on_404() {
    let _g = crate::storage::azure_credentials::credential_env_lock().lock().unwrap();
    let (port, _captured) = spawn_mock_blob("HTTP/1.1 404 Not Found", b"BlobNotFound");
    let store = AzureBlobStorage::new(test_config(port));
    let err = store.get("missing.png").unwrap_err();
    assert!(err.to_string().contains("404"));
}

#[test]
fn delete_sends_delete_method_without_blob_type_header() {
    let _g = crate::storage::azure_credentials::credential_env_lock().lock().unwrap();
    let (port, captured) = spawn_mock_blob("HTTP/1.1 202 Accepted", b"");
    let store = AzureBlobStorage::new(test_config(port));
    store.delete("uploads/photo.png").unwrap();
    let req = captured.lock().unwrap().take().unwrap();
    assert_eq!("DELETE", req.method);
    assert_eq!("/mycontainer/uploads/photo.png", req.path);
    assert!(header(&req, "x-ms-blob-type").is_none(), "x-ms-blob-type is only sent on PUT");
}

#[test]
fn url_returns_expected_path() {
    let store = AzureBlobStorage::new(test_config(9999));
    assert_eq!("http://127.0.0.1:9999/mycontainer/uploads/photo.png", store.url("uploads/photo.png"));
}

#[test]
fn key_with_special_characters_is_percent_encoded_in_path() {
    let _g = crate::storage::azure_credentials::credential_env_lock().lock().unwrap();
    let (port, captured) = spawn_mock_blob("HTTP/1.1 201 Created", b"");
    let store = AzureBlobStorage::new(test_config(port));
    store.put("a file.txt", b"x", "text/plain").unwrap();
    let req = captured.lock().unwrap().take().unwrap();
    assert_eq!("/mycontainer/a%20file.txt", req.path);
}

// ── Dynamic (Managed Identity) credentials ──────────────────────────────────

const DYNAMIC_CREDENTIAL_ENV_VARS: &[&str] = &["RWS_AZURE_CREDENTIAL_SOURCE", "IDENTITY_ENDPOINT", "IDENTITY_HEADER"];

fn clear_dynamic_credential_env_vars() {
    for v in DYNAMIC_CREDENTIAL_ENV_VARS {
        std::env::remove_var(v);
    }
}

#[test]
fn put_sends_bearer_token_when_using_managed_identity() {
    let _g = crate::storage::azure_credentials::credential_env_lock().lock().unwrap();
    clear_dynamic_credential_env_vars();

    // Stand-in App Service identity endpoint returning a Managed Identity token.
    let token_json = br#"{"access_token":"managed-identity-token","expires_on":"9999999999","token_type":"Bearer"}"#;
    let (identity_port, _identity_captured) = spawn_mock_blob("HTTP/1.1 200 OK", token_json);
    std::env::set_var("IDENTITY_ENDPOINT", format!("http://127.0.0.1:{identity_port}/msi/token"));
    std::env::set_var("IDENTITY_HEADER", "identity-header-secret");

    let (blob_port, blob_captured) = spawn_mock_blob("HTTP/1.1 201 Created", b"");
    let mut config = test_config(blob_port);
    config.account_key = String::new();
    let store = AzureBlobStorage::new(config);

    store.put("uploads/photo.png", b"data", "image/png").unwrap();

    let req = blob_captured.lock().unwrap().take().unwrap();
    assert_eq!(Some("Bearer managed-identity-token"), header(&req, "Authorization"));

    clear_dynamic_credential_env_vars();
}

#[test]
fn put_returns_storage_error_when_credential_source_is_unreachable() {
    let _g = crate::storage::azure_credentials::credential_env_lock().lock().unwrap();
    clear_dynamic_credential_env_vars();
    // Force IMDS with nothing listening — must fail cleanly, not hang.
    std::env::set_var("RWS_AZURE_CREDENTIAL_SOURCE", "managed-identity");

    let (blob_port, _blob_captured) = spawn_mock_blob("HTTP/1.1 200 OK", b"");
    let mut config = test_config(blob_port);
    config.account_key = String::new();
    let store = AzureBlobStorage::new(config);

    let err = store.put("uploads/photo.png", b"data", "image/png").unwrap_err();
    assert!(err.to_string().contains("IMDS"));

    clear_dynamic_credential_env_vars();
}

// ── AzureBlobConfig::from_env ────────────────────────────────────────────────

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn from_env_requires_account_and_container() {
    let _g = env_lock().lock().unwrap();
    std::env::remove_var("RWS_AZURE_ACCOUNT");
    std::env::remove_var("RWS_AZURE_CONTAINER");
    std::env::remove_var("RWS_AZURE_ACCOUNT_KEY");
    assert!(AzureBlobConfig::from_env().is_err());
}

#[test]
fn from_env_does_not_require_account_key() {
    let _g = env_lock().lock().unwrap();
    std::env::set_var("RWS_AZURE_ACCOUNT", "myaccount");
    std::env::set_var("RWS_AZURE_CONTAINER", "mycontainer");
    std::env::remove_var("RWS_AZURE_ACCOUNT_KEY");
    std::env::remove_var("RWS_AZURE_ENDPOINT");

    let cfg = AzureBlobConfig::from_env().unwrap();
    assert_eq!("myaccount", cfg.account);
    assert_eq!("mycontainer", cfg.container);
    assert_eq!("", cfg.account_key);
    assert_eq!("https://myaccount.blob.core.windows.net", cfg.endpoint);

    std::env::remove_var("RWS_AZURE_ACCOUNT");
    std::env::remove_var("RWS_AZURE_CONTAINER");
}

#[test]
fn from_env_respects_custom_endpoint() {
    let _g = env_lock().lock().unwrap();
    std::env::set_var("RWS_AZURE_ACCOUNT", "myaccount");
    std::env::set_var("RWS_AZURE_CONTAINER", "mycontainer");
    std::env::set_var("RWS_AZURE_ENDPOINT", "http://127.0.0.1:10000/myaccount");

    let cfg = AzureBlobConfig::from_env().unwrap();
    assert_eq!("http://127.0.0.1:10000/myaccount", cfg.endpoint);

    std::env::remove_var("RWS_AZURE_ACCOUNT");
    std::env::remove_var("RWS_AZURE_CONTAINER");
    std::env::remove_var("RWS_AZURE_ENDPOINT");
}
