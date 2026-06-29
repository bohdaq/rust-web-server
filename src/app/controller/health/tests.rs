use crate::app::controller::health::HealthController;
use crate::controller::Controller;
use crate::core::New;
use crate::header::Header;
use crate::http::VERSION;
use crate::mime_type::MimeType;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::{Address, ConnectionInfo};

fn make_connection() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 12345 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 4096,
    }
}

fn make_healthz_request() -> Request {
    Request {
        method: METHOD.get.to_string(),
        request_uri: "/healthz".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![Header { name: "Host".to_string(), value: "localhost".to_string() }],
        body: vec![],
    }
}

#[test]
fn healthz_is_matching_get() {
    let req = make_healthz_request();
    assert!(HealthController::is_matching(&req, &make_connection()));
}

#[test]
fn healthz_does_not_match_other_uri() {
    let req = Request {
        method: METHOD.get.to_string(),
        request_uri: "/other".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };
    assert!(!HealthController::is_matching(&req, &make_connection()));
}

#[test]
fn healthz_does_not_match_post() {
    let req = Request {
        method: METHOD.post.to_string(),
        request_uri: "/healthz".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };
    assert!(!HealthController::is_matching(&req, &make_connection()));
}

#[test]
fn healthz_returns_200_ok() {
    let req = make_healthz_request();
    let response = Response::new();
    let conn = make_connection();
    let result = HealthController::process(&req, response, &conn);
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n200_ok.status_code, result.status_code);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase, result.reason_phrase);
}

#[test]
fn healthz_body_is_ok() {
    let req = make_healthz_request();
    let response = Response::new();
    let conn = make_connection();
    let result = HealthController::process(&req, response, &conn);
    let body = &result.content_range_list[0].body;
    assert_eq!(b"OK".to_vec(), *body);
}

#[test]
fn healthz_content_type_is_text_plain() {
    let req = make_healthz_request();
    let response = Response::new();
    let conn = make_connection();
    let result = HealthController::process(&req, response, &conn);
    assert_eq!(MimeType::TEXT_PLAIN, result.content_range_list[0].content_type);
}
