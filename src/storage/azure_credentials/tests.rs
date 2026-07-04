use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// ── mock HTTP server (same pattern as aws_credentials/tests.rs) ─────────────

struct CapturedRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn read_request(stream: &mut std::net::TcpStream) -> CapturedRequest {
    let mut data = Vec::new();
    let mut buf = [0u8; 8192];
    let header_end = loop {
        let n = stream.read(&mut buf).unwrap_or(0);
        if n == 0 {
            break data.len();
        }
        data.extend_from_slice(&buf[..n]);
        if let Some(pos) = find_subslice(&data, b"\r\n\r\n") {
            break pos + 4;
        }
    };
    let header_str = String::from_utf8_lossy(&data[..header_end.min(data.len())]).to_string();
    let mut lines = header_str.lines();
    let request_line = lines.next().unwrap_or("").to_string();
    let mut rl_parts = request_line.split_whitespace();
    let method = rl_parts.next().unwrap_or("").to_string();
    let path = rl_parts.next().unwrap_or("").to_string();
    let mut headers = Vec::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    CapturedRequest { method, path, headers }
}

fn spawn_sequential_mock(responses: Vec<(&'static str, Vec<u8>)>) -> (String, Arc<Mutex<Vec<CapturedRequest>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let port = listener.local_addr().unwrap().port();
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured2 = Arc::clone(&captured);

    thread::spawn(move || {
        for (status_line, body) in responses {
            let Ok((mut stream, _)) = listener.accept() else { return };
            let req = read_request(&mut stream);
            captured2.lock().unwrap().push(req);
            let resp = format!(
                "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.write_all(&body);
        }
    });

    (format!("http://127.0.0.1:{port}"), captured)
}

fn spawn_black_hole_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind black hole server");
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            thread::sleep(Duration::from_secs(5));
            drop(stream);
        }
    });
    format!("http://127.0.0.1:{port}")
}

fn token_json(access_token: &str, expires_on: u64) -> String {
    format!(r#"{{"access_token":"{access_token}","expires_on":"{expires_on}","token_type":"Bearer","resource":"https://storage.azure.com/"}}"#)
}

// ── parse_token_response ─────────────────────────────────────────────────────

#[test]
fn parse_token_response_extracts_fields() {
    let json = token_json("tok123", 1_700_000_000);
    let (token, expires_at) = parse_token_response(&json).unwrap();
    assert_eq!("tok123", token);
    assert_eq!(1_700_000_000, expires_at);
}

#[test]
fn parse_token_response_errors_on_missing_access_token() {
    let json = r#"{"token_type":"Bearer"}"#;
    assert!(parse_token_response(json).is_err());
}

#[test]
fn parse_token_response_defaults_expiry_to_zero_when_missing() {
    let json = r#"{"access_token":"tok123","token_type":"Bearer"}"#;
    let (token, expires_at) = parse_token_response(json).unwrap();
    assert_eq!("tok123", token);
    assert_eq!(0, expires_at, "missing expires_on should fail safe (already expired) not panic");
}

// ── fetch_imds_token ─────────────────────────────────────────────────────────

#[test]
fn fetch_imds_token_happy_path() {
    let (base_url, captured) = spawn_sequential_mock(vec![("HTTP/1.1 200 OK", token_json("imds-token", 1_700_000_000).into_bytes())]);
    let client = crate::http_client::Client::new();
    let (token, expires_at) = fetch_imds_token(&client, &base_url).unwrap();
    assert_eq!("imds-token", token);
    assert_eq!(1_700_000_000, expires_at);

    let req = &captured.lock().unwrap()[0];
    assert_eq!("GET", req.method);
    assert!(req.path.contains("api-version=2018-02-01"));
    assert!(req.path.contains("resource="));
    assert!(req.headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("Metadata") && v == "true"));
}

#[test]
fn fetch_imds_token_times_out_cleanly_when_nothing_responds() {
    let base_url = spawn_black_hole_server();
    let client = crate::http_client::Client::new();
    let start = std::time::Instant::now();
    let result = fetch_imds_token(&client, &base_url);
    let elapsed = start.elapsed();
    assert!(result.is_err());
    assert!(elapsed < Duration::from_secs(2), "Azure IMDS probe should fail fast on its short timeout, took {elapsed:?}");
}

// ── fetch_app_service_token ──────────────────────────────────────────────────

#[test]
fn fetch_app_service_token_happy_path() {
    // Real IDENTITY_ENDPOINT values always include a path (e.g. `/MSI/token`);
    // include one here too so the mock URL matches http_client's
    // authority/path splitting (it splits at the first `/`, so a bare
    // `host:port?query` with no path is an edge case real Azure never hits).
    let (base_url, captured) =
        spawn_sequential_mock(vec![("HTTP/1.1 200 OK", token_json("app-service-token", 1_700_000_000).into_bytes())]);
    let endpoint = format!("{base_url}/msi/token");
    let client = crate::http_client::Client::new();
    let (token, _) = fetch_app_service_token(&client, &endpoint, "identity-header-secret").unwrap();
    assert_eq!("app-service-token", token);

    let req = &captured.lock().unwrap()[0];
    assert!(req.path.contains("api-version=2019-08-01"));
    assert!(req.headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("X-IDENTITY-HEADER") && v == "identity-header-secret"));
}

#[test]
fn fetch_app_service_token_errors_on_non_success_status() {
    let (base_url, _captured) = spawn_sequential_mock(vec![("HTTP/1.1 500 Internal Server Error", b"boom".to_vec())]);
    let endpoint = format!("{base_url}/msi/token");
    let client = crate::http_client::Client::new();
    let err = fetch_app_service_token(&client, &endpoint, "tok").unwrap_err();
    assert!(err.to_string().contains("500"));
}

// ── AzureCredentialsProvider::get — Shared Key / caching ────────────────────

#[test]
fn shared_key_source_never_touches_network() {
    let provider = AzureCredentialsProvider::new(Source::SharedKey("YWNjb3VudGtleQ==".to_string()));
    let a = provider.get().unwrap();
    assert_eq!(Credential::SharedKey("YWNjb3VudGtleQ==".to_string()), a);
    let b = provider.get().unwrap();
    assert_eq!(a, b);
}

#[test]
fn shared_key_source_errors_clearly_when_key_is_empty() {
    let provider = AzureCredentialsProvider::new(Source::SharedKey(String::new()));
    let err = provider.get().unwrap_err();
    assert!(err.to_string().contains("RWS_AZURE_ACCOUNT_KEY"));
}

#[test]
fn cache_returns_cached_value_before_expiry_margin() {
    let now = epoch_now();
    let (base_url, captured) =
        spawn_sequential_mock(vec![("HTTP/1.1 200 OK", token_json("tok1", now + 3600).into_bytes())]);
    let provider = AzureCredentialsProvider::new(Source::ManagedIdentity(IdentityEndpoint::AppService {
        endpoint: format!("{base_url}/msi/token"),
        header: "dummy".to_string(),
    }));

    let first = provider.get().unwrap();
    assert_eq!(Credential::Bearer("tok1".to_string()), first);
    let second = provider.get().unwrap();
    assert_eq!(first, second);

    assert_eq!(1, captured.lock().unwrap().len(), "second get() should be served from cache");
}

#[test]
fn cache_refetches_after_expiry_margin() {
    let now = epoch_now();
    // 60s out — inside the 120s refresh margin, so every get() must refetch.
    let body = token_json("tok1", now + 60);
    let (base_url, captured) =
        spawn_sequential_mock(vec![("HTTP/1.1 200 OK", body.clone().into_bytes()), ("HTTP/1.1 200 OK", body.into_bytes())]);
    let provider = AzureCredentialsProvider::new(Source::ManagedIdentity(IdentityEndpoint::AppService {
        endpoint: format!("{base_url}/msi/token"),
        header: "dummy".to_string(),
    }));

    provider.get().unwrap();
    provider.get().unwrap();

    assert_eq!(2, captured.lock().unwrap().len());
}

// ── detect — precedence chain ───────────────────────────────────────────────

const CHAIN_VARS: &[&str] = &["RWS_AZURE_CREDENTIAL_SOURCE", "IDENTITY_ENDPOINT", "IDENTITY_HEADER"];

fn clear_chain_vars() {
    for v in CHAIN_VARS {
        std::env::remove_var(v);
    }
}

#[test]
fn detect_prefers_shared_key_when_account_key_present_and_no_env_vars_set() {
    let _g = credential_env_lock().lock().unwrap();
    clear_chain_vars();
    let provider = AzureCredentialsProvider::detect(Some("YWNjb3VudGtleQ==".to_string()));
    assert_eq!(Source::SharedKey("YWNjb3VudGtleQ==".to_string()), provider.source);
    clear_chain_vars();
}

#[test]
fn detect_falls_back_to_app_service_endpoint_when_identity_env_vars_set() {
    let _g = credential_env_lock().lock().unwrap();
    clear_chain_vars();
    std::env::set_var("IDENTITY_ENDPOINT", "http://127.0.0.1:1/msi/token");
    std::env::set_var("IDENTITY_HEADER", "secret-header");
    let provider = AzureCredentialsProvider::detect(None);
    assert_eq!(
        Source::ManagedIdentity(IdentityEndpoint::AppService {
            endpoint: "http://127.0.0.1:1/msi/token".to_string(),
            header: "secret-header".to_string(),
        }),
        provider.source
    );
    clear_chain_vars();
}

#[test]
fn detect_falls_back_to_imds_as_last_resort() {
    let _g = credential_env_lock().lock().unwrap();
    clear_chain_vars();
    let provider = AzureCredentialsProvider::detect(None);
    assert_eq!(Source::ManagedIdentity(IdentityEndpoint::Imds), provider.source);
    clear_chain_vars();
}

#[test]
fn credential_source_override_forces_key_even_with_identity_env_vars_present() {
    let _g = credential_env_lock().lock().unwrap();
    clear_chain_vars();
    std::env::set_var("RWS_AZURE_CREDENTIAL_SOURCE", "key");
    std::env::set_var("IDENTITY_ENDPOINT", "http://127.0.0.1:1/msi/token");
    std::env::set_var("IDENTITY_HEADER", "secret-header");
    let provider = AzureCredentialsProvider::detect(Some("YWNjb3VudGtleQ==".to_string()));
    assert_eq!(Source::SharedKey("YWNjb3VudGtleQ==".to_string()), provider.source);
    clear_chain_vars();
}

#[test]
fn credential_source_override_forces_managed_identity_even_with_account_key_present() {
    let _g = credential_env_lock().lock().unwrap();
    clear_chain_vars();
    std::env::set_var("RWS_AZURE_CREDENTIAL_SOURCE", "managed-identity");
    let provider = AzureCredentialsProvider::detect(Some("YWNjb3VudGtleQ==".to_string()));
    assert_eq!(Source::ManagedIdentity(IdentityEndpoint::Imds), provider.source);
    clear_chain_vars();
}

#[test]
fn credential_source_override_unrecognized_value_falls_back_to_auto_chain() {
    let _g = credential_env_lock().lock().unwrap();
    clear_chain_vars();
    std::env::set_var("RWS_AZURE_CREDENTIAL_SOURCE", "totally-bogus");
    let provider = AzureCredentialsProvider::detect(Some("YWNjb3VudGtleQ==".to_string()));
    assert_eq!(Source::SharedKey("YWNjb3VudGtleQ==".to_string()), provider.source);
    clear_chain_vars();
}
