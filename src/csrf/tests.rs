use crate::application::Application;
use crate::core::New;
use crate::csrf::{CsrfLayer, CsrfToken};
use crate::header::Header;
use crate::http::VERSION;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::{Address, ConnectionInfo};

// ── helpers ───────────────────────────────────────────────────────────────────

fn conn() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 12345 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 16000,
        sni_hostname: None,
    }
}

fn req(method: &str, headers: Vec<Header>, body: Vec<u8>) -> Request {
    Request {
        method: method.to_string(),
        request_uri: "/".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers,
        body,
    }
}

fn header(name: &str, value: &str) -> Header {
    Header { name: name.to_string(), value: value.to_string() }
}

/// An inner app that always returns 200.
struct OkApp;
impl Application for OkApp {
    fn execute(&self, _: &Request, _: &ConnectionInfo) -> Result<Response, String> {
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        Ok(r)
    }
}

/// An inner app that exposes the injected CSRF token in the response body.
struct TokenEchoApp;
impl Application for TokenEchoApp {
    fn execute(&self, request: &Request, _: &ConnectionInfo) -> Result<Response, String> {
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        if let Some(tok) = CsrfToken::from_request(request) {
            r.headers.push(Header {
                name: "X-Echo-Token".to_string(),
                value: tok.value().to_string(),
            });
        }
        Ok(r)
    }
}

fn set_cookie_header(response: &Response) -> Option<String> {
    response
        .headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("set-cookie"))
        .map(|h| h.value.clone())
}

fn extract_cookie_token(response: &Response) -> Option<String> {
    let sc = set_cookie_header(response)?;
    sc.split(';')
        .next()
        .and_then(|pair| pair.find('=').map(|pos| pair[pos + 1..].trim().to_string()))
}

// ── safe methods ─────────────────────────────────────────────────────────────

#[test]
fn get_passes_through_and_sets_cookie() {
    let layer = CsrfLayer::new();
    let response = layer.handle(&req("GET", vec![], vec![]), &conn(), &OkApp).unwrap();
    assert_eq!(200, response.status_code);
    assert!(
        set_cookie_header(&response).is_some(),
        "Set-Cookie header should be present on GET"
    );
}

#[test]
fn set_cookie_contains_samesite_strict() {
    let layer = CsrfLayer::new();
    let response = layer.handle(&req("GET", vec![], vec![]), &conn(), &OkApp).unwrap();
    let sc = set_cookie_header(&response).unwrap();
    assert!(sc.contains("SameSite=Strict"), "cookie should have SameSite=Strict: {sc}");
}

#[test]
fn cookie_token_is_64_char_hex() {
    let layer = CsrfLayer::new();
    let response = layer.handle(&req("GET", vec![], vec![]), &conn(), &OkApp).unwrap();
    let token = extract_cookie_token(&response).unwrap();
    assert_eq!(64, token.len(), "token should be 64 hex chars (32 bytes): {token}");
    assert!(
        token.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')),
        "token must be lowercase hex: {token}"
    );
}

#[test]
fn each_get_without_cookie_generates_fresh_token() {
    let layer = CsrfLayer::new();
    let t1 = extract_cookie_token(
        &layer.handle(&req("GET", vec![], vec![]), &conn(), &OkApp).unwrap()
    ).unwrap();
    let t2 = extract_cookie_token(
        &layer.handle(&req("GET", vec![], vec![]), &conn(), &OkApp).unwrap()
    ).unwrap();
    assert_ne!(t1, t2, "two fresh GET requests should yield different tokens");
}

#[test]
fn existing_cookie_is_reused_on_get() {
    let layer = CsrfLayer::new();
    let existing_token = "a".repeat(64);
    let get_req = req("GET", vec![header("Cookie", &format!("_csrf={existing_token}"))], vec![]);
    let response = layer.handle(&get_req, &conn(), &OkApp).unwrap();
    let returned = extract_cookie_token(&response).unwrap();
    assert_eq!(existing_token, returned, "existing cookie token should be reused");
}

#[test]
fn head_and_options_pass_through() {
    let layer = CsrfLayer::new();
    for method in &["HEAD", "OPTIONS", "TRACE"] {
        let status = layer.handle(&req(method, vec![], vec![]), &conn(), &OkApp).unwrap().status_code;
        assert_eq!(200, status, "{method} should pass through");
    }
}

// ── CsrfToken::from_request ───────────────────────────────────────────────────

#[test]
fn csrf_token_from_request_returns_value_on_get() {
    let layer = CsrfLayer::new();
    let response =
        layer.handle(&req("GET", vec![], vec![]), &conn(), &TokenEchoApp).unwrap();
    let echoed = response
        .headers
        .iter()
        .find(|h| h.name == "X-Echo-Token")
        .map(|h| h.value.clone());
    assert!(echoed.is_some(), "CsrfToken::from_request should return Some inside a GET handler");
    assert_eq!(64, echoed.unwrap().len());
}

#[test]
fn csrf_token_from_request_returns_none_without_layer() {
    let bare_req = req("GET", vec![], vec![]);
    assert!(CsrfToken::from_request(&bare_req).is_none());
}

// ── POST validation — header ──────────────────────────────────────────────────

#[test]
fn post_with_matching_csrf_header_passes() {
    let token = "b".repeat(64);
    let post_req = req(
        "POST",
        vec![
            header("Cookie", &format!("_csrf={token}")),
            header("X-CSRF-Token", &token),
        ],
        vec![],
    );
    let layer = CsrfLayer::new();
    let status = layer.handle(&post_req, &conn(), &OkApp).unwrap().status_code;
    assert_eq!(200, status);
}

#[test]
fn post_with_mismatched_csrf_header_returns_403() {
    let layer = CsrfLayer::new();
    let post_req = req(
        "POST",
        vec![
            header("Cookie", "_csrf=aaaa"),
            header("X-CSRF-Token", "bbbb"),
        ],
        vec![],
    );
    assert_eq!(403, layer.handle(&post_req, &conn(), &OkApp).unwrap().status_code);
}

#[test]
fn post_missing_cookie_returns_403() {
    let layer = CsrfLayer::new();
    let post_req = req(
        "POST",
        vec![header("X-CSRF-Token", "some_token")],
        vec![],
    );
    assert_eq!(403, layer.handle(&post_req, &conn(), &OkApp).unwrap().status_code);
}

#[test]
fn post_missing_token_submission_returns_403() {
    let layer = CsrfLayer::new();
    let post_req = req(
        "POST",
        vec![header("Cookie", "_csrf=some_token")],
        vec![],
    );
    assert_eq!(403, layer.handle(&post_req, &conn(), &OkApp).unwrap().status_code);
}

// ── POST validation — form field ──────────────────────────────────────────────

#[test]
fn post_with_matching_form_field_passes() {
    let token = "c".repeat(64);
    let body = format!("name=alice&_csrf={token}&age=30").into_bytes();
    let post_req = req(
        "POST",
        vec![
            header("Cookie", &format!("_csrf={token}")),
            header("Content-Type", "application/x-www-form-urlencoded"),
        ],
        body,
    );
    let layer = CsrfLayer::new();
    assert_eq!(200, layer.handle(&post_req, &conn(), &OkApp).unwrap().status_code);
}

#[test]
fn post_with_mismatched_form_field_returns_403() {
    let body = b"_csrf=wrong_token".to_vec();
    let layer = CsrfLayer::new();
    let post_req = req(
        "POST",
        vec![
            header("Cookie", "_csrf=correct_token"),
            header("Content-Type", "application/x-www-form-urlencoded"),
        ],
        body,
    );
    assert_eq!(403, layer.handle(&post_req, &conn(), &OkApp).unwrap().status_code);
}

// ── PUT / PATCH / DELETE ──────────────────────────────────────────────────────

#[test]
fn put_patch_delete_also_validated() {
    let layer = CsrfLayer::new();
    for method in &["PUT", "PATCH", "DELETE"] {
        let post_req = req(
            method,
            vec![header("Cookie", "_csrf=tok"), header("X-CSRF-Token", "different")],
            vec![],
        );
        assert_eq!(403, layer.handle(&post_req, &conn(), &OkApp).unwrap().status_code,
            "{method} should be validated");
    }
}

// ── builder options ───────────────────────────────────────────────────────────

#[test]
fn http_only_flag_appears_in_cookie() {
    let layer = CsrfLayer::new().http_only(true);
    let response = layer.handle(&req("GET", vec![], vec![]), &conn(), &OkApp).unwrap();
    let sc = set_cookie_header(&response).unwrap();
    assert!(sc.contains("HttpOnly"), "HttpOnly flag should be present: {sc}");
}

#[test]
fn http_only_false_omits_flag() {
    let layer = CsrfLayer::new().http_only(false);
    let response = layer.handle(&req("GET", vec![], vec![]), &conn(), &OkApp).unwrap();
    let sc = set_cookie_header(&response).unwrap();
    assert!(!sc.contains("HttpOnly"), "HttpOnly flag should be absent by default: {sc}");
}

#[test]
fn secure_flag_appears_in_cookie() {
    let layer = CsrfLayer::new().secure(true);
    let response = layer.handle(&req("GET", vec![], vec![]), &conn(), &OkApp).unwrap();
    let sc = set_cookie_header(&response).unwrap();
    assert!(sc.contains("Secure"), "Secure flag should be present: {sc}");
}

#[test]
fn custom_cookie_name_is_used() {
    let layer = CsrfLayer::new().cookie_name("xsrf");
    let response = layer.handle(&req("GET", vec![], vec![]), &conn(), &OkApp).unwrap();
    let sc = set_cookie_header(&response).unwrap();
    assert!(sc.starts_with("xsrf="), "cookie should use custom name: {sc}");
}

#[test]
fn custom_cookie_name_validated_on_post() {
    let token = "d".repeat(64);
    let layer = CsrfLayer::new().cookie_name("xsrf");
    let post_req = req(
        "POST",
        vec![
            header("Cookie", &format!("xsrf={token}")),
            header("X-CSRF-Token", &token),
        ],
        vec![],
    );
    assert_eq!(200, layer.handle(&post_req, &conn(), &OkApp).unwrap().status_code);
}

#[test]
fn custom_header_name_is_accepted() {
    let token = "e".repeat(64);
    let layer = CsrfLayer::new().header_name("X-My-Token");
    let post_req = req(
        "POST",
        vec![
            header("Cookie", &format!("_csrf={token}")),
            header("X-My-Token", &token),
        ],
        vec![],
    );
    assert_eq!(200, layer.handle(&post_req, &conn(), &OkApp).unwrap().status_code);
}

// ── Display impl ──────────────────────────────────────────────────────────────

#[test]
fn csrf_token_display_equals_value() {
    let layer = CsrfLayer::new();
    let existing_token = "f".repeat(64);
    let get_req = req("GET", vec![header("Cookie", &format!("_csrf={existing_token}"))], vec![]);
    let response = layer.handle(&get_req, &conn(), &TokenEchoApp).unwrap();
    let echoed = response.headers.iter().find(|h| h.name == "X-Echo-Token").unwrap().value.clone();
    assert_eq!(existing_token, echoed);
}
