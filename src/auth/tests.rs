use crate::application::Application;
use crate::auth::{
    BasicAuthLayer, Claims, JwtLayer, base64_decode, base64_encode, base64url_encode,
    build_jwt, extract_bearer_token, verify_jwt,
};
use sha2::{Digest, Sha256};
use crate::core::New;
use crate::error::IntoResponse;
use crate::header::Header;
use crate::http::VERSION;
use crate::middleware::{Middleware, WithMiddleware};
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::{Address, ConnectionInfo};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn conn() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 16000,
    sni_hostname: None,
    }
}

fn get(uri: &str) -> Request {
    Request {
        method: METHOD.get.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    }
}

fn with_header(mut req: Request, name: &str, value: &str) -> Request {
    req.headers.push(Header { name: name.to_string(), value: value.to_string() });
    req
}

fn basic_header(user: &str, pass: &str) -> String {
    format!("Basic {}", base64_encode(format!("{}:{}", user, pass).as_bytes()))
}

/// A minimal application that always returns 200 OK.
struct OkApp;
impl Application for OkApp {
    fn execute(&self, _: &Request, _: &ConnectionInfo) -> Result<Response, String> {
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        Ok(r)
    }
}

// ── base64 helpers ────────────────────────────────────────────────────────────

#[test]
fn base64_roundtrip_standard() {
    let original = b"Hello, World!";
    let encoded = base64_encode(original);
    let decoded = base64_decode(&encoded).unwrap();
    assert_eq!(original, decoded.as_slice());
}

#[test]
fn base64_roundtrip_url_safe() {
    let original = b"\xfb\xff\xfe"; // produces `-_/` in base64url
    let encoded = base64url_encode(original);
    let decoded = base64_decode(&encoded).unwrap();
    assert_eq!(original, decoded.as_slice());
}

#[test]
fn base64_decode_with_padding() {
    // "Man" → "TWFu" (no padding needed); "Ma" → "TWE=" (1 pad); "M" → "TQ==" (2 pad)
    assert_eq!(base64_decode("TWFu").unwrap(), b"Man");
    assert_eq!(base64_decode("TWE=").unwrap(), b"Ma");
    assert_eq!(base64_decode("TQ==").unwrap(), b"M");
}

// ── verify_jwt / build_jwt ────────────────────────────────────────────────────

const SECRET: &[u8] = b"test-secret";
const FAR_FUTURE: u64 = 9_999_999_999;

#[test]
fn valid_jwt_returns_claims() {
    let claims_json = format!(r#"{{"sub":"user1","exp":{}}}"#, FAR_FUTURE);
    let token = build_jwt(&claims_json, SECRET);
    let claims = verify_jwt(&token, SECRET).unwrap();
    assert_eq!(Some("user1".to_string()), claims.sub);
    assert_eq!(Some(FAR_FUTURE), claims.exp);
}

#[test]
fn wrong_secret_returns_none() {
    let token = build_jwt(r#"{"sub":"u"}"#, SECRET);
    assert!(verify_jwt(&token, b"wrong-secret").is_none());
}

#[test]
fn tampered_payload_returns_none() {
    let token = build_jwt(r#"{"sub":"u"}"#, SECRET);
    let mut parts: Vec<&str> = token.splitn(3, '.').collect();
    parts[1] = "dGFtcGVyZWQ"; // "tampered"
    let tampered = parts.join(".");
    assert!(verify_jwt(&tampered, SECRET).is_none());
}

#[test]
fn expired_token_returns_none() {
    let token = build_jwt(r#"{"sub":"u","exp":1}"#, SECRET); // exp in the past
    assert!(verify_jwt(&token, SECRET).is_none());
}

#[test]
fn token_without_exp_is_accepted() {
    let token = build_jwt(r#"{"sub":"no-exp"}"#, SECRET);
    assert!(verify_jwt(&token, SECRET).is_some());
}

#[test]
fn wrong_algorithm_returns_none() {
    // Manually build a token with alg=RS256 in the header
    let header = base64url_encode(br#"{"alg":"RS256","typ":"JWT"}"#);
    let payload = base64url_encode(br#"{"sub":"u"}"#);
    let fake_sig = base64url_encode(b"not-a-real-sig");
    let token = format!("{}.{}.{}", header, payload, fake_sig);
    assert!(verify_jwt(&token, SECRET).is_none());
}

#[test]
fn malformed_token_returns_none() {
    assert!(verify_jwt("not.a.jwt.with.extra.dots", SECRET).is_none());
    assert!(verify_jwt("onlytwoparts", SECRET).is_none());
    assert!(verify_jwt("", SECRET).is_none());
}

// ── Claims helpers ────────────────────────────────────────────────────────────

#[test]
fn claims_is_valid_at_before_exp() {
    let c = Claims { sub: None, exp: Some(FAR_FUTURE), raw: String::new() };
    assert!(c.is_valid_at(1_000_000));
}

#[test]
fn claims_is_valid_at_after_exp() {
    let c = Claims { sub: None, exp: Some(1), raw: String::new() };
    assert!(!c.is_valid_at(1_000_000));
}

#[test]
fn claims_no_exp_always_valid() {
    let c = Claims { sub: None, exp: None, raw: String::new() };
    assert!(c.is_valid_at(u64::MAX));
}

// ── extract_bearer_token ──────────────────────────────────────────────────────

#[test]
fn extract_bearer_token_from_header() {
    let req = with_header(get("/"), "Authorization", "Bearer tok123");
    assert_eq!(Some("tok123".to_string()), extract_bearer_token(&req));
}

#[test]
fn extract_bearer_token_absent_header() {
    assert!(extract_bearer_token(&get("/")).is_none());
}

// ── BasicAuthLayer ────────────────────────────────────────────────────────────

#[test]
fn basic_auth_missing_header_returns_401_with_challenge() {
    let layer = BasicAuthLayer::new(|_, _| true);
    let resp = layer.handle(&get("/"), &conn(), &OkApp).unwrap();
    assert_eq!(401, resp.status_code);
    let has_challenge = resp.headers.iter().any(|h| h.name == "WWW-Authenticate");
    assert!(has_challenge, "expected WWW-Authenticate header");
}

#[test]
fn basic_auth_wrong_password_returns_401() {
    let layer = BasicAuthLayer::new(|user, pass| user == "admin" && pass == "correct");
    let req = with_header(get("/"), "Authorization", &basic_header("admin", "wrong"));
    let resp = layer.handle(&req, &conn(), &OkApp).unwrap();
    assert_eq!(401, resp.status_code);
}

#[test]
fn basic_auth_correct_credentials_passes_through() {
    let layer = BasicAuthLayer::new(|user, pass| user == "admin" && pass == "secret");
    let req = with_header(get("/"), "Authorization", &basic_header("admin", "secret"));
    let resp = layer.handle(&req, &conn(), &OkApp).unwrap();
    assert_eq!(200, resp.status_code);
}

#[test]
fn basic_auth_password_with_colon() {
    let layer = BasicAuthLayer::new(|user, pass| user == "u" && pass == "p:with:colons");
    let req = with_header(get("/"), "Authorization", &basic_header("u", "p:with:colons"));
    let resp = layer.handle(&req, &conn(), &OkApp).unwrap();
    assert_eq!(200, resp.status_code);
}

#[test]
fn basic_auth_via_middleware_stack() {
    use crate::app::App;
    let app = WithMiddleware::new(OkApp).wrap(
        BasicAuthLayer::new(|user, pass| user == "a" && pass == "b"),
    );
    let req = with_header(get("/does-not-exist"), "Authorization", &basic_header("a", "b"));
    let resp = app.execute(&req, &conn()).unwrap();
    // OkApp always 200, so a valid credential passes through
    assert_eq!(200, resp.status_code);
}

// ── JwtLayer ──────────────────────────────────────────────────────────────────

#[test]
fn jwt_layer_valid_token_passes_through() {
    let token = build_jwt(&format!(r#"{{"sub":"u","exp":{}}}"#, FAR_FUTURE), SECRET);
    let layer = JwtLayer::new(SECRET);
    let req = with_header(get("/"), "Authorization", &format!("Bearer {}", token));
    let resp = layer.handle(&req, &conn(), &OkApp).unwrap();
    assert_eq!(200, resp.status_code);
}

#[test]
fn jwt_layer_missing_token_returns_401() {
    let layer = JwtLayer::new(SECRET);
    let resp = layer.handle(&get("/"), &conn(), &OkApp).unwrap();
    assert_eq!(401, resp.status_code);
}

#[test]
fn jwt_layer_invalid_token_returns_401() {
    let layer = JwtLayer::new(SECRET);
    let req = with_header(get("/"), "Authorization", "Bearer not.a.valid.token");
    let resp = layer.handle(&req, &conn(), &OkApp).unwrap();
    assert_eq!(401, resp.status_code);
}

#[test]
fn jwt_layer_expired_token_returns_401() {
    let token = build_jwt(r#"{"sub":"u","exp":1}"#, SECRET);
    let layer = JwtLayer::new(SECRET);
    let req = with_header(get("/"), "Authorization", &format!("Bearer {}", token));
    let resp = layer.handle(&req, &conn(), &OkApp).unwrap();
    assert_eq!(401, resp.status_code);
}

// ── BasicAuthLayer::from_htpasswd_file ──────────────────────────────────────────

fn temp_htpasswd(contents: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("rws_htpasswd_test_{}_{}", std::process::id(), rand_suffix()));
    std::fs::write(&path, contents).unwrap();
    path
}

fn rand_suffix() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    nanos ^ COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn sha256_entry(password: &str) -> String {
    format!("{{SHA256}}{}", base64_encode(&Sha256::digest(password.as_bytes())))
}

#[test]
fn sha256_entry_matches_independently_computed_openssl_output() {
    // Cross-check against `printf '%s' 'hunter2' | openssl dgst -sha256 -binary | openssl base64`,
    // computed independently of this crate — also the exact value used in DEVELOPER.md's example.
    assert_eq!(
        "{SHA256}9S+9MrKzuG/4jvbEkGKChfSCrxXdyylUH5S89Saj9sc=",
        sha256_entry("hunter2")
    );
}

#[test]
fn from_htpasswd_file_accepts_plain_text_password() {
    let path = temp_htpasswd("alice:s3cret\n");
    let layer = BasicAuthLayer::from_htpasswd_file(path.to_str().unwrap()).unwrap();

    let req = with_header(get("/"), "Authorization", &basic_header("alice", "s3cret"));
    let resp = layer.handle(&req, &conn(), &OkApp).unwrap();
    assert_eq!(200, resp.status_code);

    std::fs::remove_file(&path).ok();
}

#[test]
fn from_htpasswd_file_rejects_wrong_plain_text_password() {
    let path = temp_htpasswd("alice:s3cret\n");
    let layer = BasicAuthLayer::from_htpasswd_file(path.to_str().unwrap()).unwrap();

    let req = with_header(get("/"), "Authorization", &basic_header("alice", "wrong"));
    let resp = layer.handle(&req, &conn(), &OkApp).unwrap();
    assert_eq!(401, resp.status_code);

    std::fs::remove_file(&path).ok();
}

#[test]
fn from_htpasswd_file_accepts_sha256_scheme() {
    let path = temp_htpasswd(&format!("bob:{}\n", sha256_entry("hunter2")));
    let layer = BasicAuthLayer::from_htpasswd_file(path.to_str().unwrap()).unwrap();

    let req = with_header(get("/"), "Authorization", &basic_header("bob", "hunter2"));
    let resp = layer.handle(&req, &conn(), &OkApp).unwrap();
    assert_eq!(200, resp.status_code);

    let req_wrong = with_header(get("/"), "Authorization", &basic_header("bob", "wrong"));
    let resp_wrong = layer.handle(&req_wrong, &conn(), &OkApp).unwrap();
    assert_eq!(401, resp_wrong.status_code);

    std::fs::remove_file(&path).ok();
}

#[test]
fn from_htpasswd_file_rejects_unknown_user() {
    let path = temp_htpasswd("alice:s3cret\n");
    let layer = BasicAuthLayer::from_htpasswd_file(path.to_str().unwrap()).unwrap();

    let req = with_header(get("/"), "Authorization", &basic_header("mallory", "s3cret"));
    let resp = layer.handle(&req, &conn(), &OkApp).unwrap();
    assert_eq!(401, resp.status_code);

    std::fs::remove_file(&path).ok();
}

#[test]
fn from_htpasswd_file_ignores_comments_and_blank_lines() {
    let path = temp_htpasswd("# comment\n\nalice:s3cret\n   \n");
    let layer = BasicAuthLayer::from_htpasswd_file(path.to_str().unwrap()).unwrap();

    let req = with_header(get("/"), "Authorization", &basic_header("alice", "s3cret"));
    let resp = layer.handle(&req, &conn(), &OkApp).unwrap();
    assert_eq!(200, resp.status_code);

    std::fs::remove_file(&path).ok();
}

#[test]
fn from_htpasswd_file_supports_multiple_users() {
    let path = temp_htpasswd(&format!("alice:s3cret\nbob:{}\n", sha256_entry("hunter2")));
    let layer = BasicAuthLayer::from_htpasswd_file(path.to_str().unwrap()).unwrap();

    let alice = with_header(get("/"), "Authorization", &basic_header("alice", "s3cret"));
    assert_eq!(200, layer.handle(&alice, &conn(), &OkApp).unwrap().status_code);

    let bob = with_header(get("/"), "Authorization", &basic_header("bob", "hunter2"));
    assert_eq!(200, layer.handle(&bob, &conn(), &OkApp).unwrap().status_code);

    std::fs::remove_file(&path).ok();
}

#[test]
fn from_htpasswd_file_errors_when_file_is_missing() {
    let result = BasicAuthLayer::from_htpasswd_file("/nonexistent/path/.htpasswd-rws-test");
    assert!(result.is_err());
}

#[test]
fn from_htpasswd_file_missing_authorization_header_returns_401_challenge() {
    let path = temp_htpasswd("alice:s3cret\n");
    let layer = BasicAuthLayer::from_htpasswd_file(path.to_str().unwrap()).unwrap();

    let resp = layer.handle(&get("/"), &conn(), &OkApp).unwrap();
    assert_eq!(401, resp.status_code);
    assert!(resp._get_header("WWW-Authenticate".to_string()).is_some());

    std::fs::remove_file(&path).ok();
}
