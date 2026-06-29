use crate::app::controller::metrics::MetricsController;
use crate::controller::Controller;
use crate::core::New;
use crate::http::VERSION;
use crate::metrics::{ERRORS_TOTAL, REQUESTS_TOTAL};
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::{Address, ConnectionInfo};
use std::sync::atomic::Ordering;

fn make_connection() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 12345 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 4096,
    }
}

fn make_metrics_request() -> Request {
    Request {
        method: METHOD.get.to_string(),
        request_uri: "/metrics".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    }
}

#[test]
fn metrics_is_matching() {
    let req = make_metrics_request();
    assert!(MetricsController::is_matching(&req, &make_connection()));
}

#[test]
fn metrics_does_not_match_other_uri() {
    let req = Request {
        method: METHOD.get.to_string(),
        request_uri: "/other".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };
    assert!(!MetricsController::is_matching(&req, &make_connection()));
}

#[test]
fn metrics_returns_200() {
    let req = make_metrics_request();
    let response = Response::new();
    let conn = make_connection();
    let result = MetricsController::process(&req, response, &conn);
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n200_ok.status_code, result.status_code);
}

#[test]
fn metrics_body_contains_prometheus_counters() {
    REQUESTS_TOTAL.store(7, Ordering::SeqCst);
    ERRORS_TOTAL.store(2, Ordering::SeqCst);

    let req = make_metrics_request();
    let response = Response::new();
    let conn = make_connection();
    let result = MetricsController::process(&req, response, &conn);
    let body = String::from_utf8(result.content_range_list[0].body.clone()).unwrap();

    assert!(body.contains("rws_requests_total"), "missing counter name");
    assert!(body.contains("rws_errors_total"), "missing errors counter");
    assert!(body.contains("rws_active_connections"), "missing gauge");
}

#[test]
fn metrics_content_type_is_prometheus() {
    let req = make_metrics_request();
    let response = Response::new();
    let conn = make_connection();
    let result = MetricsController::process(&req, response, &conn);
    assert_eq!("text/plain; version=0.0.4", result.content_range_list[0].content_type);
}
