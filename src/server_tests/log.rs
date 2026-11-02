//! Tests of rws-server's `log` that read repository fixtures (`static/`,
//! `index.html`): a crate's tests run in the crate's directory.
#![allow(unused_imports, dead_code)]

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use file_ext::FileExt;
use crate::app::App;
use crate::core::New;
use crate::header::Header;
use crate::http::VERSION;
use crate::log::Log;
use crate::range::Range;
use crate::mime_type::MimeType;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

fn make_request(method: &str, uri: &str) -> Request {
    Request {
        method: method.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: Default::default(),
    }
}

fn ok_response_with_body(body: &[u8]) -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r.content_range_list = vec![Range::get_content_range(body.to_vec(), MimeType::TEXT_PLAIN.to_string())];
    r
}

#[test]
fn combined_log_format_includes_clf_fields() {
    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/static/test.txt".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: Default::default(),
    };

    let (response, request) = App::handle_request(request);
    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n200_ok.status_code);

    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 4567);
    let log = Log::combined(&request, &response, &peer_addr);

    // IP address
    assert!(log.starts_with("192.168.1.1 - - ["), "log should start with IP: {}", log);

    // Timestamp bracket: "IP - - [DD/Mon/YYYY:HH:MM:SS +0000]"
    let open = log.find('[').unwrap();
    let close = log.find(']').unwrap();
    let timestamp = &log[open + 1..close];
    // format: DD/Mon/YYYY:HH:MM:SS +0000  (26 chars)
    assert_eq!(timestamp.len(), 26, "unexpected timestamp length: {}", timestamp);
    assert!(timestamp.ends_with("+0000"), "timestamp should end with +0000: {}", timestamp);

    // Request line, status, size
    assert!(log.contains("\"GET /static/test.txt HTTP/1.1\""), "missing request line: {}", log);
    assert!(log.contains(" 200 "), "missing status 200: {}", log);
}
