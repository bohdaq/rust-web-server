use super::*;
use crate::http_client::Client;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// ── mock HTTP server ─────────────────────────────────────────────────────────
//
// Serves a fixed sequence of canned responses, one per accepted connection,
// in order — matches how `fetch_irsa_credentials`/`fetch_ecs_credentials`
// (one request) and `fetch_imds_credentials` (three sequential requests)
// each open a fresh connection per call.

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

/// Spawns a mock server that answers `responses.len()` connections in order,
/// one canned `(status_line, body)` response per connection, then stops.
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
                "{status_line}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.write_all(&body);
        }
    });

    (format!("http://127.0.0.1:{port}"), captured)
}

/// Accepts one connection and holds it open without ever responding, forcing
/// the client to hit its own read timeout.
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

fn write_temp_token_file(contents: &str, name: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!("rws-irsa-test-{name}-{n}"));
    std::fs::write(&path, contents).expect("write temp token file");
    path.to_str().unwrap().to_string()
}

// ── extract_tag / extract_json_str_field ────────────────────────────────────

#[test]
fn extract_tag_finds_nested_value() {
    let xml = "<Response><Result><AccessKeyId>ABC123</AccessKeyId></Result></Response>";
    assert_eq!(Some("ABC123".to_string()), extract_tag(xml, "AccessKeyId"));
}

#[test]
fn extract_tag_returns_none_when_missing() {
    let xml = "<Response><Result></Result></Response>";
    assert_eq!(None, extract_tag(xml, "AccessKeyId"));
}

#[test]
fn extract_json_str_field_finds_value() {
    let json = r#"{"Code":"Success","AccessKeyId":"ABC123","Expiration":"2026-07-03T18:24:30Z"}"#;
    assert_eq!(Some("ABC123".to_string()), extract_json_str_field(json, "AccessKeyId"));
}

#[test]
fn extract_json_str_field_returns_none_when_missing() {
    let json = r#"{"Code":"Success"}"#;
    assert_eq!(None, extract_json_str_field(json, "AccessKeyId"));
}

// ── parse_iso8601_epoch ──────────────────────────────────────────────────────

#[test]
fn parse_iso8601_epoch_parses_known_timestamp() {
    // Cross-checked against the golden EPOCH constant used in aws_sigv4/tests.rs.
    assert_eq!(Some(1_369_353_600), parse_iso8601_epoch("2013-05-24T00:00:00Z"));
}

#[test]
fn parse_iso8601_epoch_returns_none_on_garbage() {
    assert_eq!(None, parse_iso8601_epoch("not-a-timestamp"));
    assert_eq!(None, parse_iso8601_epoch(""));
}

// ── parse_sts_response / parse_json_credentials ─────────────────────────────

#[test]
fn parse_sts_response_extracts_all_fields() {
    let xml = "<AssumeRoleWithWebIdentityResponse><AssumeRoleWithWebIdentityResult><Credentials>\
        <AccessKeyId>AKIAEXAMPLE</AccessKeyId>\
        <SecretAccessKey>secretexample</SecretAccessKey>\
        <SessionToken>tokenexample</SessionToken>\
        <Expiration>2013-05-24T00:00:00Z</Expiration>\
        </Credentials></AssumeRoleWithWebIdentityResult></AssumeRoleWithWebIdentityResponse>";
    let creds = parse_sts_response(xml).unwrap();
    assert_eq!("AKIAEXAMPLE", creds.access_key);
    assert_eq!("secretexample", creds.secret_key);
    assert_eq!(Some("tokenexample".to_string()), creds.session_token);
    assert_eq!(Some(1_369_353_600), creds.expires_at_epoch_secs);
}

#[test]
fn parse_sts_response_errors_on_missing_field() {
    let xml = "<Credentials><AccessKeyId>AKIAEXAMPLE</AccessKeyId></Credentials>";
    assert!(parse_sts_response(xml).is_err());
}

#[test]
fn parse_json_credentials_extracts_all_fields() {
    let json = r#"{"Code":"Success","AccessKeyId":"AKIAEXAMPLE","SecretAccessKey":"secretexample","Token":"tokenexample","Expiration":"2013-05-24T00:00:00Z"}"#;
    let creds = parse_json_credentials(json).unwrap();
    assert_eq!("AKIAEXAMPLE", creds.access_key);
    assert_eq!("secretexample", creds.secret_key);
    assert_eq!(Some("tokenexample".to_string()), creds.session_token);
    assert_eq!(Some(1_369_353_600), creds.expires_at_epoch_secs);
}

#[test]
fn parse_json_credentials_errors_on_missing_field() {
    let json = r#"{"Code":"Success"}"#;
    assert!(parse_json_credentials(json).is_err());
}

// ── fetch_irsa_credentials ───────────────────────────────────────────────────

#[test]
fn fetch_irsa_credentials_happy_path() {
    let xml = "<AssumeRoleWithWebIdentityResponse><AssumeRoleWithWebIdentityResult><Credentials>\
        <AccessKeyId>AKIAIRSA</AccessKeyId><SecretAccessKey>secretirsa</SecretAccessKey>\
        <SessionToken>tokenirsa</SessionToken><Expiration>2013-05-24T00:00:00Z</Expiration>\
        </Credentials></AssumeRoleWithWebIdentityResult></AssumeRoleWithWebIdentityResponse>";
    let (base_url, captured) = spawn_sequential_mock(vec![("HTTP/1.1 200 OK", xml.as_bytes().to_vec())]);
    let token_file = write_temp_token_file("dummy-jwt-token", "irsa-happy");

    let client = Client::new();
    let creds = fetch_irsa_credentials(&client, &base_url, "arn:aws:iam::123456789012:role/my-role", &token_file).unwrap();
    assert_eq!("AKIAIRSA", creds.access_key);
    assert_eq!(Some("tokenirsa".to_string()), creds.session_token);

    let req = &captured.lock().unwrap()[0];
    assert_eq!("GET", req.method);
    assert!(req.path.contains("Action=AssumeRoleWithWebIdentity"));
    assert!(req.path.contains("RoleArn=arn%3Aaws%3Aiam%3A%3A123456789012%3Arole%2Fmy-role"));
    assert!(req.path.contains("WebIdentityToken=dummy-jwt-token"));

    std::fs::remove_file(&token_file).ok();
}

#[test]
fn fetch_irsa_credentials_errors_when_role_arn_missing() {
    let client = Client::new();
    let err = fetch_irsa_credentials(&client, "http://127.0.0.1:1", "", "/some/token/file").unwrap_err();
    assert!(err.to_string().contains("AWS_ROLE_ARN"));
}

#[test]
fn fetch_irsa_credentials_errors_when_token_file_unreadable() {
    let client = Client::new();
    let err = fetch_irsa_credentials(&client, "http://127.0.0.1:1", "arn:aws:iam::123:role/x", "/no/such/file").unwrap_err();
    assert!(err.to_string().contains("AWS_WEB_IDENTITY_TOKEN_FILE"));
}

// ── fetch_imds_credentials ───────────────────────────────────────────────────

#[test]
fn fetch_imds_credentials_happy_path() {
    let creds_json = r#"{"Code":"Success","AccessKeyId":"AKIAIMDS","SecretAccessKey":"secretimds","Token":"tokenimds","Expiration":"2013-05-24T00:00:00Z"}"#;
    let (base_url, captured) = spawn_sequential_mock(vec![
        ("HTTP/1.1 200 OK", b"imds-token-value".to_vec()),
        ("HTTP/1.1 200 OK", b"my-instance-role".to_vec()),
        ("HTTP/1.1 200 OK", creds_json.as_bytes().to_vec()),
    ]);

    let client = Client::new();
    let creds = fetch_imds_credentials(&client, &base_url).unwrap();
    assert_eq!("AKIAIMDS", creds.access_key);
    assert_eq!(Some("tokenimds".to_string()), creds.session_token);

    let requests = captured.lock().unwrap();
    assert_eq!(3, requests.len());
    assert_eq!("PUT", requests[0].method);
    assert_eq!("/latest/api/token", requests[0].path);
    assert!(requests[0].headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("X-aws-ec2-metadata-token-ttl-seconds") && v == "21600"));

    assert_eq!("GET", requests[1].method);
    assert_eq!("/latest/meta-data/iam/security-credentials/", requests[1].path);
    assert!(requests[1].headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("X-aws-ec2-metadata-token") && v == "imds-token-value"));

    assert_eq!("/latest/meta-data/iam/security-credentials/my-instance-role", requests[2].path);
}

#[test]
fn fetch_imds_credentials_times_out_cleanly_when_nothing_responds() {
    let base_url = spawn_black_hole_server();
    let client = Client::new();
    let start = std::time::Instant::now();
    let result = fetch_imds_credentials(&client, &base_url);
    let elapsed = start.elapsed();
    assert!(result.is_err());
    assert!(elapsed < Duration::from_secs(2), "IMDS probe should fail fast on its short timeout, took {elapsed:?}");
}

// ── fetch_ecs_credentials ────────────────────────────────────────────────────

#[test]
fn fetch_ecs_credentials_happy_path() {
    let json = r#"{"Code":"Success","AccessKeyId":"AKIAECS","SecretAccessKey":"secretecs","Token":"tokenecs","Expiration":"2013-05-24T00:00:00Z"}"#;
    let (base_url, captured) = spawn_sequential_mock(vec![("HTTP/1.1 200 OK", json.as_bytes().to_vec())]);
    let client = Client::new();
    let creds = fetch_ecs_credentials(&client, &format!("{base_url}/creds"), None).unwrap();
    assert_eq!("AKIAECS", creds.access_key);
    assert!(captured.lock().unwrap()[0].headers.iter().all(|(k, _)| !k.eq_ignore_ascii_case("Authorization")));
}

#[test]
fn fetch_ecs_credentials_sends_authorization_header_when_provided() {
    let json = r#"{"Code":"Success","AccessKeyId":"AKIAECS","SecretAccessKey":"secretecs","Token":"tokenecs","Expiration":"2013-05-24T00:00:00Z"}"#;
    let (base_url, captured) = spawn_sequential_mock(vec![("HTTP/1.1 200 OK", json.as_bytes().to_vec())]);
    let client = Client::new();
    fetch_ecs_credentials(&client, &format!("{base_url}/creds"), Some("secret-auth-token")).unwrap();
    let req = &captured.lock().unwrap()[0];
    assert!(req.headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("Authorization") && v == "secret-auth-token"));
}

#[test]
fn fetch_ecs_credentials_errors_on_non_success_status() {
    let (base_url, _captured) = spawn_sequential_mock(vec![("HTTP/1.1 500 Internal Server Error", b"boom".to_vec())]);
    let client = Client::new();
    let err = fetch_ecs_credentials(&client, &format!("{base_url}/creds"), None).unwrap_err();
    assert!(err.to_string().contains("500"));
}

// ── CredentialsProvider::get — caching ───────────────────────────────────────

#[test]
fn static_source_never_touches_network() {
    // No mock server is started at all — if `get()` attempted a network
    // call, this would hang or error instead of succeeding immediately.
    let provider = CredentialsProvider::new(
        Source::Static(Credentials {
            access_key: "AK".to_string(),
            secret_key: "SK".to_string(),
            session_token: None,
            expires_at_epoch_secs: None,
        }),
        "us-east-1",
    );
    let creds = provider.get().unwrap();
    assert_eq!("AK", creds.access_key);
    let creds_again = provider.get().unwrap();
    assert_eq!("AK", creds_again.access_key);
}

#[test]
fn cache_returns_cached_value_before_expiry_margin() {
    let now = epoch_now();
    let json = format!(
        r#"{{"Code":"Success","AccessKeyId":"AK1","SecretAccessKey":"SK1","Token":"TOK1","Expiration":"{}"}}"#,
        iso8601_from_epoch(now + 3600)
    );
    let (base_url, captured) = spawn_sequential_mock(vec![("HTTP/1.1 200 OK", json.into_bytes())]);
    let provider = CredentialsProvider::new(Source::EcsFull { url: format!("{base_url}/creds"), auth_token: None }, "us-east-1");

    let first = provider.get().unwrap();
    assert_eq!("AK1", first.access_key);
    let second = provider.get().unwrap();
    assert_eq!("AK1", second.access_key);

    // Only one network round-trip: the second `get()` was served from cache.
    assert_eq!(1, captured.lock().unwrap().len());
}

#[test]
fn cache_refetches_after_expiry_margin() {
    let now = epoch_now();
    // Expiration only 60s out — inside the 120s refresh margin, so every
    // `get()` call must refetch.
    let json = format!(
        r#"{{"Code":"Success","AccessKeyId":"AK1","SecretAccessKey":"SK1","Token":"TOK1","Expiration":"{}"}}"#,
        iso8601_from_epoch(now + 60)
    );
    let (base_url, captured) = spawn_sequential_mock(vec![
        ("HTTP/1.1 200 OK", json.clone().into_bytes()),
        ("HTTP/1.1 200 OK", json.into_bytes()),
    ]);
    let provider = CredentialsProvider::new(Source::EcsFull { url: format!("{base_url}/creds"), auth_token: None }, "us-east-1");

    provider.get().unwrap();
    provider.get().unwrap();

    assert_eq!(2, captured.lock().unwrap().len());
}

fn iso8601_from_epoch(epoch_secs: u64) -> String {
    let days = epoch_secs / 86400;
    let secs_in_day = epoch_secs % 86400;
    let (y, m, d) = crate::scheduler::cron::days_to_ymd(days);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, secs_in_day / 3600, (secs_in_day % 3600) / 60, secs_in_day % 60)
}

// ── CredentialsProvider::detect — precedence chain ──────────────────────────

const CHAIN_VARS: &[&str] = &[
    "RWS_S3_CREDENTIAL_SOURCE",
    "AWS_ROLE_ARN",
    "AWS_WEB_IDENTITY_TOKEN_FILE",
    "AWS_CONTAINER_CREDENTIALS_FULL_URI",
    "AWS_CONTAINER_CREDENTIALS_RELATIVE_URI",
    "AWS_CONTAINER_AUTHORIZATION_TOKEN",
];

fn clear_chain_vars() {
    for v in CHAIN_VARS {
        std::env::remove_var(v);
    }
}

#[test]
fn detect_prefers_static_when_both_keys_present_and_no_env_vars_set() {
    let _g = credential_env_lock().lock().unwrap();
    clear_chain_vars();
    let provider = CredentialsProvider::detect("us-east-1", Some("AK".to_string()), Some("SK".to_string()));
    assert_eq!(
        Source::Static(Credentials { access_key: "AK".to_string(), secret_key: "SK".to_string(), session_token: None, expires_at_epoch_secs: None }),
        provider.source
    );
    clear_chain_vars();
}

#[test]
fn detect_falls_back_to_irsa_when_static_keys_absent() {
    let _g = credential_env_lock().lock().unwrap();
    clear_chain_vars();
    std::env::set_var("AWS_ROLE_ARN", "arn:aws:iam::123:role/x");
    std::env::set_var("AWS_WEB_IDENTITY_TOKEN_FILE", "/var/run/token");
    let provider = CredentialsProvider::detect("us-east-1", None, None);
    assert_eq!(Source::Irsa { role_arn: "arn:aws:iam::123:role/x".to_string(), token_file: "/var/run/token".to_string() }, provider.source);
    clear_chain_vars();
}

#[test]
fn detect_prefers_irsa_over_ecs_when_both_present() {
    let _g = credential_env_lock().lock().unwrap();
    clear_chain_vars();
    std::env::set_var("AWS_ROLE_ARN", "arn:aws:iam::123:role/x");
    std::env::set_var("AWS_WEB_IDENTITY_TOKEN_FILE", "/var/run/token");
    std::env::set_var("AWS_CONTAINER_CREDENTIALS_RELATIVE_URI", "/v2/credentials/abc");
    let provider = CredentialsProvider::detect("us-east-1", None, None);
    assert!(matches!(provider.source, Source::Irsa { .. }));
    clear_chain_vars();
}

#[test]
fn detect_prefers_ecs_full_uri_over_relative_uri() {
    let _g = credential_env_lock().lock().unwrap();
    clear_chain_vars();
    std::env::set_var("AWS_CONTAINER_CREDENTIALS_FULL_URI", "http://169.254.170.2/full");
    std::env::set_var("AWS_CONTAINER_CREDENTIALS_RELATIVE_URI", "/v2/credentials/abc");
    std::env::set_var("AWS_CONTAINER_AUTHORIZATION_TOKEN", "auth-tok");
    let provider = CredentialsProvider::detect("us-east-1", None, None);
    assert_eq!(
        Source::EcsFull { url: "http://169.254.170.2/full".to_string(), auth_token: Some("auth-tok".to_string()) },
        provider.source
    );
    clear_chain_vars();
}

#[test]
fn detect_falls_back_to_ecs_relative_uri() {
    let _g = credential_env_lock().lock().unwrap();
    clear_chain_vars();
    std::env::set_var("AWS_CONTAINER_CREDENTIALS_RELATIVE_URI", "/v2/credentials/abc");
    let provider = CredentialsProvider::detect("us-east-1", None, None);
    assert_eq!(Source::EcsRelative { path: "/v2/credentials/abc".to_string() }, provider.source);
    clear_chain_vars();
}

#[test]
fn detect_falls_back_to_imds_as_last_resort() {
    let _g = credential_env_lock().lock().unwrap();
    clear_chain_vars();
    let provider = CredentialsProvider::detect("us-east-1", None, None);
    assert_eq!(Source::Imds, provider.source);
    clear_chain_vars();
}

#[test]
fn credential_source_override_forces_static_even_with_ecs_vars_present() {
    let _g = credential_env_lock().lock().unwrap();
    clear_chain_vars();
    std::env::set_var("RWS_S3_CREDENTIAL_SOURCE", "static");
    std::env::set_var("AWS_CONTAINER_CREDENTIALS_RELATIVE_URI", "/v2/credentials/abc");
    let provider = CredentialsProvider::detect("us-east-1", Some("AK".to_string()), Some("SK".to_string()));
    assert!(matches!(provider.source, Source::Static(_)));
    clear_chain_vars();
}

#[test]
fn credential_source_override_forces_imds_even_with_static_keys_present() {
    let _g = credential_env_lock().lock().unwrap();
    clear_chain_vars();
    std::env::set_var("RWS_S3_CREDENTIAL_SOURCE", "imds");
    let provider = CredentialsProvider::detect("us-east-1", Some("AK".to_string()), Some("SK".to_string()));
    assert_eq!(Source::Imds, provider.source);
    clear_chain_vars();
}

#[test]
fn credential_source_override_unrecognized_value_falls_back_to_auto_chain() {
    let _g = credential_env_lock().lock().unwrap();
    clear_chain_vars();
    std::env::set_var("RWS_S3_CREDENTIAL_SOURCE", "totally-bogus");
    let provider = CredentialsProvider::detect("us-east-1", Some("AK".to_string()), Some("SK".to_string()));
    assert!(matches!(provider.source, Source::Static(_)), "unrecognized override value should fall back to the auto chain");
    clear_chain_vars();
}
