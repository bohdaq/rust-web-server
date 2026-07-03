use flate2::read::GzDecoder;
use std::io::Read;
use crate::compression::apply_gzip;
use crate::header::Header;
use crate::http::VERSION;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

fn make_request_with_gzip() -> Request {
    Request {
        method: METHOD.get.to_string(),
        request_uri: "/".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![Header {
            name: Header::_ACCEPT_ENCODING.to_string(),
            value: "gzip, deflate".to_string(),
        }],
        body: vec![],
    }
}

fn make_response(body: &[u8], mime: &str) -> Response {
    Response {
        http_version: VERSION.http_1_1.to_string(),
        status_code: *STATUS_CODE_REASON_PHRASE.n200_ok.status_code,
        reason_phrase: STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string(),
        headers: vec![],
        content_range_list: vec![Range::get_content_range(body.to_vec(), mime.to_string())],
        stream_file: None,
        stream_pipe: None,
    }
}

#[test]
fn compresses_text_html_when_gzip_accepted() {
    let request = make_request_with_gzip();
    let mut response = make_response(b"<html>hello</html>", MimeType::TEXT_HTML);

    apply_gzip(&request, &mut response);

    let has_ce = response.headers.iter().any(|h| {
        h.name == Header::_CONTENT_ENCODING && h.value == "gzip"
    });
    assert!(has_ce, "Content-Encoding: gzip should be set");

    // body should decompress back to original
    let compressed = &response.content_range_list[0].body;
    let mut decoder = GzDecoder::new(compressed.as_slice());
    let mut decompressed = String::new();
    decoder.read_to_string(&mut decompressed).unwrap();
    assert_eq!(decompressed, "<html>hello</html>");
}

#[test]
fn does_not_compress_when_gzip_not_accepted() {
    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };
    let mut response = make_response(b"<html>hello</html>", MimeType::TEXT_HTML);
    let original_len = response.content_range_list[0].body.len();

    apply_gzip(&request, &mut response);

    assert_eq!(response.content_range_list[0].body.len(), original_len);
    let has_ce = response.headers.iter().any(|h| h.name == Header::_CONTENT_ENCODING);
    assert!(!has_ce);
}

#[test]
fn does_not_compress_binary_content() {
    let request = make_request_with_gzip();
    let mut response = make_response(&[0u8; 64], "image/png");
    let original_len = response.content_range_list[0].body.len();

    apply_gzip(&request, &mut response);

    assert_eq!(response.content_range_list[0].body.len(), original_len);
}

#[test]
fn appends_accept_encoding_to_existing_vary() {
    let request = make_request_with_gzip();
    let mut response = make_response(b"{}", MimeType::APPLICATION_JSON);
    response.headers.push(Header {
        name: Header::_VARY.to_string(),
        value: "Origin".to_string(),
    });

    apply_gzip(&request, &mut response);

    let vary = response.headers.iter().find(|h| h.name == Header::_VARY).unwrap();
    assert!(vary.value.contains("Accept-Encoding"), "Vary should include Accept-Encoding: {}", vary.value);
    assert!(vary.value.contains("Origin"), "Vary should still contain Origin");
}

#[test]
fn does_not_duplicate_accept_encoding_in_vary() {
    let request = make_request_with_gzip();
    let mut response = make_response(b"{}", MimeType::APPLICATION_JSON);
    response.headers.push(Header {
        name: Header::_VARY.to_string(),
        value: "Accept-Encoding".to_string(),
    });

    apply_gzip(&request, &mut response);

    let vary_count = response.headers.iter()
        .filter(|h| h.name == Header::_VARY)
        .count();
    let vary = response.headers.iter().find(|h| h.name == Header::_VARY).unwrap();
    let ae_count = vary.value.to_lowercase().matches("accept-encoding").count();
    assert_eq!(vary_count, 1);
    assert_eq!(ae_count, 1, "Accept-Encoding should not be duplicated in Vary");
}
