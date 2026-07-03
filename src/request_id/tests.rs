use crate::application::Application;
use crate::core::New;
use crate::http::VERSION;
use crate::middleware::{Middleware, WithMiddleware};
use crate::request::{METHOD, Request};
use crate::request_id::{generate_request_id, RequestIdLayer, DEFAULT_HEADER};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::{Address, ConnectionInfo};

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

fn ok() -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r
}

/// Echoes back whatever request-id header it sees as the response body, so
/// tests can prove the ID was actually injected *before* the handler ran —
/// not just added to the response afterward.
struct EchoRequestIdApp;
impl Application for EchoRequestIdApp {
    fn execute(&self, request: &Request, _conn: &ConnectionInfo) -> Result<Response, String> {
        use crate::mime_type::MimeType;
        use crate::range::Range;
        let seen = request.get_header(DEFAULT_HEADER.to_string()).map(|h| h.value.clone()).unwrap_or_default();
        let mut r = ok();
        r.content_range_list = vec![Range::get_content_range(seen.into_bytes(), MimeType::TEXT_PLAIN.to_string())];
        Ok(r)
    }
}

struct OkApp;
impl Application for OkApp {
    fn execute(&self, _request: &Request, _conn: &ConnectionInfo) -> Result<Response, String> {
        Ok(ok())
    }
}

fn body_of(resp: &Response) -> String {
    String::from_utf8(resp.content_range_list[0].body.clone()).unwrap()
}

// ── generate_request_id ─────────────────────────────────────────────────────

#[test]
fn generate_request_id_produces_unique_values() {
    let a = generate_request_id();
    let b = generate_request_id();
    assert_ne!(a, b);
}

#[test]
fn generate_request_id_is_uuid_shaped() {
    let id = generate_request_id();
    let parts: Vec<&str> = id.split('-').collect();
    assert_eq!(5, parts.len(), "expected 5 dash-separated groups, got: {id}");
    assert_eq!([8, 4, 4, 4, 12], [parts[0].len(), parts[1].len(), parts[2].len(), parts[3].len(), parts[4].len()]);
    assert!(id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
}

// ── RequestIdLayer ───────────────────────────────────────────────────────────

#[test]
fn generates_and_injects_an_id_when_none_is_present() {
    let app = WithMiddleware::new(EchoRequestIdApp).wrap(RequestIdLayer::new());
    let resp = app.execute(&get("/x"), &conn()).unwrap();

    // The handler saw a non-empty ID (proves it was injected into the
    // request before dispatch, not only added to the response after).
    let seen_by_handler = body_of(&resp);
    assert!(!seen_by_handler.is_empty());

    // The same value is set on the response.
    let on_response = resp._get_header(DEFAULT_HEADER.to_string()).unwrap();
    assert_eq!(seen_by_handler, on_response.value);
}

#[test]
fn preserves_an_incoming_id_unchanged() {
    let app = WithMiddleware::new(EchoRequestIdApp).wrap(RequestIdLayer::new());
    let mut req = get("/x");
    req.headers.push(crate::header::Header { name: DEFAULT_HEADER.to_string(), value: "upstream-id-123".to_string() });

    let resp = app.execute(&req, &conn()).unwrap();
    assert_eq!("upstream-id-123", body_of(&resp));
    assert_eq!("upstream-id-123", resp._get_header(DEFAULT_HEADER.to_string()).unwrap().value);
}

#[test]
fn two_requests_without_an_incoming_id_get_different_ids() {
    let app = WithMiddleware::new(OkApp).wrap(RequestIdLayer::new());
    let r1 = app.execute(&get("/x"), &conn()).unwrap();
    let r2 = app.execute(&get("/x"), &conn()).unwrap();

    let id1 = r1._get_header(DEFAULT_HEADER.to_string()).unwrap().value.clone();
    let id2 = r2._get_header(DEFAULT_HEADER.to_string()).unwrap().value.clone();
    assert_ne!(id1, id2);
}

#[test]
fn custom_header_name_is_honored() {
    let app = WithMiddleware::new(OkApp).wrap(RequestIdLayer::new().header("X-Correlation-Id"));
    let resp = app.execute(&get("/x"), &conn()).unwrap();

    assert!(resp._get_header("X-Correlation-Id".to_string()).is_some());
    assert!(resp._get_header(DEFAULT_HEADER.to_string()).is_none());
}

#[test]
fn response_status_is_unaffected() {
    let app = WithMiddleware::new(OkApp).wrap(RequestIdLayer::new());
    let resp = app.execute(&get("/x"), &conn()).unwrap();
    assert_eq!(200, resp.status_code);
}

// ── RequestId extractor ──────────────────────────────────────────────────────

#[test]
fn request_id_extractor_reads_the_header_when_present() {
    use crate::extract::{FromRequest, RequestId};
    let mut req = get("/x");
    req.headers.push(crate::header::Header { name: DEFAULT_HEADER.to_string(), value: "abc-123".to_string() });

    let id = RequestId::from_request(&req).unwrap();
    assert_eq!("abc-123", id.as_str());
}

#[test]
fn request_id_extractor_is_empty_when_absent() {
    use crate::extract::{FromRequest, RequestId};
    let id = RequestId::from_request(&get("/x")).unwrap();
    assert_eq!("", id.as_str());
}

#[test]
fn request_id_extractor_sees_the_id_injected_by_the_middleware() {
    use crate::extract::{FromRequest, RequestId};

    struct ExtractorApp;
    impl Application for ExtractorApp {
        fn execute(&self, request: &Request, _conn: &ConnectionInfo) -> Result<Response, String> {
            let id = RequestId::from_request(request).unwrap();
            assert!(!id.as_str().is_empty());
            Ok(ok())
        }
    }

    let app = WithMiddleware::new(ExtractorApp).wrap(RequestIdLayer::new());
    let resp = app.execute(&get("/x"), &conn()).unwrap();
    assert_eq!(200, resp.status_code);
}
