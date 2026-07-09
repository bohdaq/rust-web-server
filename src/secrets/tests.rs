//! Unit tests for `secrets::resolve` and its three backends.
//!
//! Each backend test spins up a loopback `TcpListener` mock server (same
//! idiom as `storage::aws_credentials::tests`/`storage::azure_credentials::tests`)
//! and points the backend at it via an endpoint-override env var
//! (`AWS_ENDPOINT_URL_SECRETSMANAGER`, `AZURE_KEY_VAULT_ENDPOINT_OVERRIDE`,
//! `AZURE_AD_LOGIN_ENDPOINT_OVERRIDE`) or, for Vault, `VAULT_ADDR` itself
//! (which is always meant to be overridden to point at a real Vault server).
//!
//! Every test that reads/writes one of these process-wide env vars holds
//! [`env_lock`] for its full duration — the same rule `CLAUDE.md` documents
//! for `RWS_CONFIG_*` vars, applied here to `VAULT_*`/`AWS_*`/`AZURE_*` since
//! `cargo test` runs everything in one process. AWS tests additionally hold
//! `storage::aws_credentials::credential_env_lock()` and Managed-Identity
//! Azure tests additionally hold `storage::azure_credentials::credential_env_lock()`,
//! since those modules' own tests mutate the same `AWS_ROLE_ARN`/
//! `IDENTITY_ENDPOINT`-family vars this file's AWS/Azure tests read through
//! `CredentialsProvider::detect`/`managed_identity_endpoint_from_env`.

use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

// ── env lock (local to this file's own VAULT_*/AWS_*/AZURE_* vars) ─────────────

fn env_lock() -> &'static Mutex<()> {
    static LOCK: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

const VAULT_VARS: &[&str] = &["VAULT_ADDR", "VAULT_TOKEN"];
const AWS_SM_VARS: &[&str] =
    &["AWS_REGION", "AWS_DEFAULT_REGION", "AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY", "AWS_ENDPOINT_URL_SECRETSMANAGER"];
const AZURE_KV_VARS: &[&str] = &[
    "AZURE_KEY_VAULT_TENANT_ID",
    "AZURE_KEY_VAULT_CLIENT_ID",
    "AZURE_KEY_VAULT_CLIENT_SECRET",
    "AZURE_KEY_VAULT_ENDPOINT_OVERRIDE",
    "AZURE_AD_LOGIN_ENDPOINT_OVERRIDE",
    "IDENTITY_ENDPOINT",
    "IDENTITY_HEADER",
];
const AWS_CHAIN_VARS: &[&str] = &[
    "RWS_S3_CREDENTIAL_SOURCE",
    "AWS_ROLE_ARN",
    "AWS_WEB_IDENTITY_TOKEN_FILE",
    "AWS_CONTAINER_CREDENTIALS_FULL_URI",
    "AWS_CONTAINER_CREDENTIALS_RELATIVE_URI",
    "AWS_CONTAINER_AUTHORIZATION_TOKEN",
];

fn clear_vars(vars: &[&str]) {
    for v in vars {
        std::env::remove_var(v);
    }
}

// ── mock HTTP server ─────────────────────────────────────────────────────────

struct CapturedRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: String,
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
    let content_length: usize = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    let mut body = data[header_end.min(data.len())..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut buf).unwrap_or(0);
        if n == 0 {
            break;
        }
        body.extend_from_slice(&buf[..n]);
    }
    CapturedRequest { method, path, headers, body: String::from_utf8_lossy(&body).to_string() }
}

/// Spawns a mock server that answers one connection with `(status_line, body)`,
/// then stops. Returns the `http://127.0.0.1:PORT` base URL and the captured request.
fn spawn_mock(status_line: &'static str, body: Vec<u8>) -> (String, Arc<Mutex<Option<CapturedRequest>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let port = listener.local_addr().unwrap().port();
    let captured = Arc::new(Mutex::new(None));
    let captured2 = Arc::clone(&captured);

    thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else { return };
        let req = read_request(&mut stream);
        *captured2.lock().unwrap() = Some(req);
        let resp =
            format!("{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
        let _ = stream.write_all(resp.as_bytes());
        let _ = stream.write_all(&body);
    });

    (format!("http://127.0.0.1:{port}"), captured)
}

// ── resolve() prefix dispatch / passthrough ─────────────────────────────────

#[test]
fn resolve_passes_through_a_value_with_no_recognized_prefix() {
    assert_eq!("plain-value".to_string(), resolve("plain-value").unwrap());
    assert_eq!("postgres://user:pw@host/db".to_string(), resolve("postgres://user:pw@host/db").unwrap());
}

#[test]
fn is_secret_ref_recognizes_all_three_prefixes() {
    assert!(is_secret_ref("vault://x#y"));
    assert!(is_secret_ref("aws-sm://x"));
    assert!(is_secret_ref("azkv://x/y"));
    assert!(!is_secret_ref("http://example.com"));
}

// ── vault:// ─────────────────────────────────────────────────────────────────

#[test]
fn vault_resolve_happy_path() {
    let _g = env_lock().lock().unwrap();
    clear_vars(VAULT_VARS);
    let (base_url, captured) = spawn_mock("HTTP/1.1 200 OK", br#"{"data":{"data":{"password":"s3cr3t"}}}"#.to_vec());
    std::env::set_var("VAULT_ADDR", &base_url);
    std::env::set_var("VAULT_TOKEN", "test-token");

    let result = resolve("vault://secret/myapp/db#password").unwrap();
    assert_eq!("s3cr3t", result);

    let req = captured.lock().unwrap().take().unwrap();
    assert_eq!("GET", req.method);
    assert_eq!("/v1/secret/data/myapp/db", req.path);
    assert!(req.headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("X-Vault-Token") && v == "test-token"));

    clear_vars(VAULT_VARS);
}

#[test]
fn vault_resolve_missing_field_suffix_errors() {
    let _g = env_lock().lock().unwrap();
    clear_vars(VAULT_VARS);
    let err = resolve("vault://secret/myapp/db").unwrap_err();
    assert!(err.to_string().contains("#field"));
    clear_vars(VAULT_VARS);
}

#[test]
fn vault_resolve_requires_mount_and_secret_path() {
    let _g = env_lock().lock().unwrap();
    clear_vars(VAULT_VARS);
    let err = resolve("vault://secret#field").unwrap_err();
    assert!(err.to_string().contains("mount/secret-path"));
    clear_vars(VAULT_VARS);
}

#[test]
fn vault_resolve_missing_token_errors() {
    let _g = env_lock().lock().unwrap();
    clear_vars(VAULT_VARS);
    std::env::set_var("VAULT_ADDR", "http://127.0.0.1:1");
    let err = resolve("vault://secret/myapp/db#password").unwrap_err();
    assert!(err.to_string().contains("VAULT_TOKEN"));
    clear_vars(VAULT_VARS);
}

#[test]
fn vault_resolve_non_success_status_errors() {
    let _g = env_lock().lock().unwrap();
    clear_vars(VAULT_VARS);
    let (base_url, _captured) = spawn_mock("HTTP/1.1 404 Not Found", b"no such secret".to_vec());
    std::env::set_var("VAULT_ADDR", &base_url);
    std::env::set_var("VAULT_TOKEN", "test-token");

    let err = resolve("vault://secret/myapp/db#password").unwrap_err();
    assert!(err.to_string().contains("404"));
    clear_vars(VAULT_VARS);
}

#[test]
fn vault_resolve_field_not_found_errors() {
    let _g = env_lock().lock().unwrap();
    clear_vars(VAULT_VARS);
    let (base_url, _captured) = spawn_mock("HTTP/1.1 200 OK", br#"{"data":{"data":{"username":"admin"}}}"#.to_vec());
    std::env::set_var("VAULT_ADDR", &base_url);
    std::env::set_var("VAULT_TOKEN", "test-token");

    let err = resolve("vault://secret/myapp/db#password").unwrap_err();
    assert!(err.to_string().contains("no field 'password'"));
    clear_vars(VAULT_VARS);
}

// ── aws-sm:// ────────────────────────────────────────────────────────────────

fn aws_sm_setup(endpoint: &str) {
    clear_vars(AWS_CHAIN_VARS);
    std::env::set_var("AWS_REGION", "us-east-1");
    std::env::set_var("AWS_ACCESS_KEY_ID", "AKIATEST");
    std::env::set_var("AWS_SECRET_ACCESS_KEY", "secrettest");
    std::env::set_var("AWS_ENDPOINT_URL_SECRETSMANAGER", endpoint);
}

#[test]
fn aws_secrets_manager_resolve_happy_path_whole_secret_string() {
    let _g1 = crate::storage::aws_credentials::credential_env_lock().lock().unwrap();
    let _g2 = env_lock().lock().unwrap();
    clear_vars(AWS_SM_VARS);
    let (base_url, captured) = spawn_mock("HTTP/1.1 200 OK", br#"{"SecretString":"plainpassword"}"#.to_vec());
    aws_sm_setup(&base_url);

    let result = resolve("aws-sm://prod/db-password").unwrap();
    assert_eq!("plainpassword", result);

    let req = captured.lock().unwrap().take().unwrap();
    assert_eq!("POST", req.method);
    assert_eq!("/", req.path);
    assert!(req.headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("X-Amz-Target") && v == "secretsmanager.GetSecretValue"));
    assert!(req.headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("Content-Type") && v == "application/x-amz-json-1.1"));
    assert!(req.headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("Authorization") && v.starts_with("AWS4-HMAC-SHA256")));
    assert!(req.body.contains(r#""SecretId":"prod/db-password""#));

    clear_vars(AWS_SM_VARS);
    clear_vars(AWS_CHAIN_VARS);
}

#[test]
fn aws_secrets_manager_resolve_extracts_json_field() {
    let _g1 = crate::storage::aws_credentials::credential_env_lock().lock().unwrap();
    let _g2 = env_lock().lock().unwrap();
    clear_vars(AWS_SM_VARS);
    let secret_string = r#"{\"username\":\"admin\",\"password\":\"hunter2\"}"#;
    let response = format!(r#"{{"SecretString":"{secret_string}"}}"#);
    let (base_url, _captured) = spawn_mock("HTTP/1.1 200 OK", response.into_bytes());
    aws_sm_setup(&base_url);

    let result = resolve("aws-sm://prod/db-creds#password").unwrap();
    assert_eq!("hunter2", result);

    clear_vars(AWS_SM_VARS);
    clear_vars(AWS_CHAIN_VARS);
}

#[test]
fn aws_secrets_manager_resolve_missing_secret_name_errors() {
    let _g1 = crate::storage::aws_credentials::credential_env_lock().lock().unwrap();
    let _g2 = env_lock().lock().unwrap();
    clear_vars(AWS_SM_VARS);
    aws_sm_setup("http://127.0.0.1:1");

    let err = resolve("aws-sm://").unwrap_err();
    assert!(err.to_string().contains("missing a secret name"));

    clear_vars(AWS_SM_VARS);
    clear_vars(AWS_CHAIN_VARS);
}

#[test]
fn aws_secrets_manager_resolve_missing_region_errors() {
    let _g1 = crate::storage::aws_credentials::credential_env_lock().lock().unwrap();
    let _g2 = env_lock().lock().unwrap();
    clear_vars(AWS_SM_VARS);
    clear_vars(AWS_CHAIN_VARS);

    let err = resolve("aws-sm://prod/db-password").unwrap_err();
    assert!(err.to_string().contains("AWS_REGION"));

    clear_vars(AWS_SM_VARS);
}

#[test]
fn aws_secrets_manager_resolve_non_success_status_errors() {
    let _g1 = crate::storage::aws_credentials::credential_env_lock().lock().unwrap();
    let _g2 = env_lock().lock().unwrap();
    clear_vars(AWS_SM_VARS);
    let (base_url, _captured) = spawn_mock("HTTP/1.1 400 Bad Request", b"ResourceNotFoundException".to_vec());
    aws_sm_setup(&base_url);

    let err = resolve("aws-sm://prod/db-password").unwrap_err();
    assert!(err.to_string().contains("400"));

    clear_vars(AWS_SM_VARS);
    clear_vars(AWS_CHAIN_VARS);
}

#[test]
fn aws_secrets_manager_resolve_missing_secret_string_errors() {
    let _g1 = crate::storage::aws_credentials::credential_env_lock().lock().unwrap();
    let _g2 = env_lock().lock().unwrap();
    clear_vars(AWS_SM_VARS);
    let (base_url, _captured) = spawn_mock("HTTP/1.1 200 OK", b"{}".to_vec());
    aws_sm_setup(&base_url);

    let err = resolve("aws-sm://prod/db-password").unwrap_err();
    assert!(err.to_string().contains("no SecretString"));

    clear_vars(AWS_SM_VARS);
    clear_vars(AWS_CHAIN_VARS);
}

#[test]
fn aws_secrets_manager_resolve_field_requested_on_non_json_secret_string_errors() {
    let _g1 = crate::storage::aws_credentials::credential_env_lock().lock().unwrap();
    let _g2 = env_lock().lock().unwrap();
    clear_vars(AWS_SM_VARS);
    let (base_url, _captured) = spawn_mock("HTTP/1.1 200 OK", br#"{"SecretString":"plain-not-json"}"#.to_vec());
    aws_sm_setup(&base_url);

    let err = resolve("aws-sm://prod/db-password#password").unwrap_err();
    assert!(err.to_string().contains("not valid JSON"));

    clear_vars(AWS_SM_VARS);
    clear_vars(AWS_CHAIN_VARS);
}

#[test]
fn aws_secrets_manager_resolve_field_missing_in_json_errors() {
    let _g1 = crate::storage::aws_credentials::credential_env_lock().lock().unwrap();
    let _g2 = env_lock().lock().unwrap();
    clear_vars(AWS_SM_VARS);
    let secret_string = r#"{\"username\":\"admin\"}"#;
    let response = format!(r#"{{"SecretString":"{secret_string}"}}"#);
    let (base_url, _captured) = spawn_mock("HTTP/1.1 200 OK", response.into_bytes());
    aws_sm_setup(&base_url);

    let err = resolve("aws-sm://prod/db-creds#password").unwrap_err();
    assert!(err.to_string().contains("no field 'password'"));

    clear_vars(AWS_SM_VARS);
    clear_vars(AWS_CHAIN_VARS);
}

// ── azkv:// ──────────────────────────────────────────────────────────────────

#[test]
fn azure_key_vault_resolve_happy_path_service_principal() {
    let _g = env_lock().lock().unwrap();
    clear_vars(AZURE_KV_VARS);

    // Two sequential mock servers: one for the Azure AD token endpoint, one
    // for the Key Vault secret GET — `fetch_token` and the secret GET each
    // open their own connection.
    let (token_url, token_captured) = spawn_mock("HTTP/1.1 200 OK", br#"{"access_token":"kv-token-abc"}"#.to_vec());
    let (kv_url, kv_captured) = spawn_mock("HTTP/1.1 200 OK", br#"{"value":"hunter2"}"#.to_vec());

    std::env::set_var("AZURE_KEY_VAULT_TENANT_ID", "tenant-1");
    std::env::set_var("AZURE_KEY_VAULT_CLIENT_ID", "client-1");
    std::env::set_var("AZURE_KEY_VAULT_CLIENT_SECRET", "client-secret-1");
    std::env::set_var("AZURE_AD_LOGIN_ENDPOINT_OVERRIDE", &token_url);
    std::env::set_var("AZURE_KEY_VAULT_ENDPOINT_OVERRIDE", &kv_url);

    let result = resolve("azkv://my-kv/db-password").unwrap();
    assert_eq!("hunter2", result);

    let token_req = token_captured.lock().unwrap().take().unwrap();
    assert_eq!("POST", token_req.method);
    assert!(token_req.path.contains("/tenant-1/oauth2/v2.0/token"));
    assert!(token_req.body.contains("grant_type=client_credentials"));
    assert!(token_req.body.contains("client_id=client-1"));
    assert!(token_req.body.contains("scope=https%3A%2F%2Fvault.azure.net%2F.default"));

    let kv_req = kv_captured.lock().unwrap().take().unwrap();
    assert_eq!("GET", kv_req.method);
    assert!(kv_req.path.contains("/secrets/db-password"));
    assert!(kv_req.headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("Authorization") && v == "Bearer kv-token-abc"));

    clear_vars(AZURE_KV_VARS);
}

#[test]
fn azure_key_vault_resolve_falls_back_to_managed_identity_when_no_service_principal_configured() {
    let _g1 = crate::storage::azure_credentials::credential_env_lock().lock().unwrap();
    let _g2 = env_lock().lock().unwrap();
    clear_vars(AZURE_KV_VARS);

    let (identity_url, identity_captured) = spawn_mock("HTTP/1.1 200 OK", br#"{"access_token":"mi-token-xyz"}"#.to_vec());
    let (kv_url, kv_captured) = spawn_mock("HTTP/1.1 200 OK", br#"{"value":"managed-identity-secret"}"#.to_vec());

    std::env::set_var("IDENTITY_ENDPOINT", format!("{identity_url}/token"));
    std::env::set_var("IDENTITY_HEADER", "identity-header-value");
    std::env::set_var("AZURE_KEY_VAULT_ENDPOINT_OVERRIDE", &kv_url);

    let result = resolve("azkv://my-kv/db-password").unwrap();
    assert_eq!("managed-identity-secret", result);

    let identity_req = identity_captured.lock().unwrap().take().unwrap();
    assert!(identity_req.headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("X-IDENTITY-HEADER") && v == "identity-header-value"));

    let kv_req = kv_captured.lock().unwrap().take().unwrap();
    assert!(kv_req.headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("Authorization") && v == "Bearer mi-token-xyz"));

    clear_vars(AZURE_KV_VARS);
}

#[test]
fn azure_key_vault_resolve_requires_vault_and_secret_name() {
    let _g = env_lock().lock().unwrap();
    clear_vars(AZURE_KV_VARS);
    let err = resolve("azkv://my-kv").unwrap_err();
    assert!(err.to_string().contains("vault-name/secret-name"));
    clear_vars(AZURE_KV_VARS);
}

#[test]
fn azure_key_vault_resolve_non_success_status_errors() {
    let _g = env_lock().lock().unwrap();
    clear_vars(AZURE_KV_VARS);
    let (token_url, _tc) = spawn_mock("HTTP/1.1 200 OK", br#"{"access_token":"kv-token-abc"}"#.to_vec());
    let (kv_url, _kc) = spawn_mock("HTTP/1.1 403 Forbidden", b"insufficient permissions".to_vec());

    std::env::set_var("AZURE_KEY_VAULT_TENANT_ID", "tenant-1");
    std::env::set_var("AZURE_KEY_VAULT_CLIENT_ID", "client-1");
    std::env::set_var("AZURE_KEY_VAULT_CLIENT_SECRET", "client-secret-1");
    std::env::set_var("AZURE_AD_LOGIN_ENDPOINT_OVERRIDE", &token_url);
    std::env::set_var("AZURE_KEY_VAULT_ENDPOINT_OVERRIDE", &kv_url);

    let err = resolve("azkv://my-kv/db-password").unwrap_err();
    assert!(err.to_string().contains("403"));

    clear_vars(AZURE_KV_VARS);
}

#[test]
fn azure_key_vault_resolve_token_endpoint_failure_errors() {
    let _g = env_lock().lock().unwrap();
    clear_vars(AZURE_KV_VARS);
    let (token_url, _tc) = spawn_mock("HTTP/1.1 401 Unauthorized", b"invalid_client".to_vec());

    std::env::set_var("AZURE_KEY_VAULT_TENANT_ID", "tenant-1");
    std::env::set_var("AZURE_KEY_VAULT_CLIENT_ID", "client-1");
    std::env::set_var("AZURE_KEY_VAULT_CLIENT_SECRET", "wrong-secret");
    std::env::set_var("AZURE_AD_LOGIN_ENDPOINT_OVERRIDE", &token_url);

    let err = resolve("azkv://my-kv/db-password").unwrap_err();
    assert!(err.to_string().contains("Azure AD token request failed"));

    clear_vars(AZURE_KV_VARS);
}

// ── resolve_env_vars ─────────────────────────────────────────────────────────

#[test]
fn resolve_env_vars_rewrites_only_matching_rws_prefixed_secret_refs() {
    let _g = env_lock().lock().unwrap();
    clear_vars(VAULT_VARS);
    let (base_url, _captured) = spawn_mock("HTTP/1.1 200 OK", br#"{"data":{"data":{"password":"resolved-value"}}}"#.to_vec());
    std::env::set_var("VAULT_ADDR", &base_url);
    std::env::set_var("VAULT_TOKEN", "test-token");

    std::env::set_var("RWS_CONFIG_TEST_SECRET_REF", "vault://secret/myapp/db#password");
    std::env::set_var("RWS_CONFIG_TEST_PLAIN", "already-plain-value");
    std::env::remove_var("NOT_RWS_PREFIXED_SECRET_REF");
    std::env::set_var("NOT_RWS_PREFIXED_SECRET_REF", "vault://secret/myapp/db#password");

    resolve_env_vars().unwrap();

    assert_eq!("resolved-value", std::env::var("RWS_CONFIG_TEST_SECRET_REF").unwrap());
    assert_eq!("already-plain-value", std::env::var("RWS_CONFIG_TEST_PLAIN").unwrap());
    // Not RWS_-prefixed — resolve_env_vars must never touch it, even though
    // its value also matches a secret-reference prefix.
    assert_eq!("vault://secret/myapp/db#password", std::env::var("NOT_RWS_PREFIXED_SECRET_REF").unwrap());

    std::env::remove_var("RWS_CONFIG_TEST_SECRET_REF");
    std::env::remove_var("RWS_CONFIG_TEST_PLAIN");
    std::env::remove_var("NOT_RWS_PREFIXED_SECRET_REF");
    clear_vars(VAULT_VARS);
}

#[test]
fn resolve_env_vars_propagates_a_resolution_failure() {
    let _g = env_lock().lock().unwrap();
    clear_vars(VAULT_VARS);
    std::env::set_var("VAULT_ADDR", "http://127.0.0.1:1");
    std::env::set_var("RWS_CONFIG_TEST_BROKEN_REF", "vault://secret/myapp/db#password");

    let err = resolve_env_vars().unwrap_err();
    assert!(err.to_string().contains("RWS_CONFIG_TEST_BROKEN_REF"));

    std::env::remove_var("RWS_CONFIG_TEST_BROKEN_REF");
    clear_vars(VAULT_VARS);
}

// ── SecretsError ─────────────────────────────────────────────────────────────

#[test]
fn secrets_error_display_shows_the_message() {
    let err = SecretsError::new("something went wrong");
    assert_eq!("something went wrong", err.to_string());
}

#[test]
fn secrets_error_from_storage_error_preserves_message() {
    let storage_err = crate::storage::StorageError::new("storage failed");
    let secrets_err: SecretsError = storage_err.into();
    assert_eq!("storage failed", secrets_err.to_string());
}
