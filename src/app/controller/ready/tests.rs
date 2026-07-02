use std::sync::atomic::Ordering;
use crate::app::controller::ready::ReadyController;
use crate::controller::Controller;
use crate::core::New;
use crate::http::VERSION;
use crate::metrics::SERVER_READY;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::{Address, ConnectionInfo};

fn make_connection() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 12345 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 4096,
    sni_hostname: None,
    }
}

fn make_readyz_request() -> Request {
    Request {
        method: METHOD.get.to_string(),
        request_uri: "/readyz".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    }
}

#[test]
fn readyz_is_matching() {
    let req = make_readyz_request();
    assert!(ReadyController::is_matching(&req, &make_connection()));
}

#[test]
fn readyz_does_not_match_other_uri() {
    let req = Request {
        method: METHOD.get.to_string(),
        request_uri: "/other".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };
    assert!(!ReadyController::is_matching(&req, &make_connection()));
}

#[test]
fn readyz_returns_503_when_not_ready() {
    let _guard = crate::test_env::lock();
    SERVER_READY.store(false, Ordering::SeqCst);
    let req = make_readyz_request();
    let response = Response::new();
    let conn = make_connection();
    let result = ReadyController::process(&req, response, &conn);
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n503_service_unavailable.status_code, result.status_code);
    assert_eq!(b"not ready".to_vec(), result.content_range_list[0].body);
}

#[test]
fn readyz_returns_200_when_ready() {
    let _guard = crate::test_env::lock();
    SERVER_READY.store(true, Ordering::SeqCst);
    let req = make_readyz_request();
    let response = Response::new();
    let conn = make_connection();
    let result = ReadyController::process(&req, response, &conn);
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n200_ok.status_code, result.status_code);
    assert_eq!(b"OK".to_vec(), result.content_range_list[0].body);
    // restore
    SERVER_READY.store(false, Ordering::SeqCst);
}
