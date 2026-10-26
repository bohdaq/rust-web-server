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
    let html = "<html>hello</html>".repeat(64);
    let mut response = make_response(html.as_bytes(), MimeType::TEXT_HTML);

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
    assert_eq!(decompressed, html);
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
    let json = format!("[{}]", vec!["{\"k\":\"v\"}"; 100].join(","));
    let mut response = make_response(json.as_bytes(), MimeType::APPLICATION_JSON);
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

fn is_gzipped(response: &Response) -> bool {
    response.headers.iter().any(|h| h.name == Header::_CONTENT_ENCODING)
}

#[test]
fn small_bodies_are_not_compressed() {
    let body = "a".repeat(crate::compression::MIN_COMPRESS_SIZE - 1);
    let mut response = make_response(body.as_bytes(), MimeType::TEXT_HTML);
    apply_gzip(&make_request_with_gzip(), &mut response);
    assert!(!is_gzipped(&response));
    assert_eq!(response.content_range_list[0].body, body.as_bytes());
}

#[test]
fn incompressible_bodies_are_sent_as_is() {
    // Pseudo-random bytes: gzip output would be larger than the input.
    let mut state: u32 = 0x1234_5678;
    let body: Vec<u8> = (0..2048).map(|_| { state ^= state << 13; state ^= state >> 17; state ^= state << 5; state as u8 }).collect();
    let mut response = make_response(&body, MimeType::TEXT_PLAIN);
    apply_gzip(&make_request_with_gzip(), &mut response);
    assert!(!is_gzipped(&response));
    assert_eq!(response.content_range_list[0].body, body);
}

#[test]
fn already_encoded_responses_are_not_compressed_again() {
    let body = "<p>already gzipped upstream</p>".repeat(64);
    let mut response = make_response(body.as_bytes(), MimeType::TEXT_HTML);
    response.headers.push(Header { name: "content-encoding".to_string(), value: "br".to_string() });
    apply_gzip(&make_request_with_gzip(), &mut response);
    assert_eq!(response.headers.iter().filter(|h| h.name.eq_ignore_ascii_case("content-encoding")).count(), 1);
    assert_eq!(response.content_range_list[0].body, body.as_bytes());
}

// ── #52: a partial body is never compressed ─────────────────────────────────

fn partial(ranges: &[(u64, u64)], text: &str) -> Response {
    let mut response = make_response(b"", MimeType::TEXT_HTML);
    response.status_code = *STATUS_CODE_REASON_PHRASE.n206_partial_content.status_code;
    response.reason_phrase = STATUS_CODE_REASON_PHRASE.n206_partial_content.reason_phrase.to_string();
    response.content_range_list = ranges.iter().map(|&(start, end)| {
        let mut cr = Range::get_content_range(text.as_bytes()[start as usize..=end as usize].to_vec(), MimeType::TEXT_HTML.to_string());
        cr.range.start = start;
        cr.range.end = end;
        cr.size = text.len().to_string();
        cr
    }).collect();
    response
}

#[test]
fn a_single_range_206_is_sent_as_is() {
    let text = "<p>partial content</p>".repeat(200);
    let mut response = partial(&[(0, 1200)], &text);
    apply_gzip(&make_request_with_gzip(), &mut response);
    assert!(!is_gzipped(&response));
    let cr = &response.content_range_list[0];
    assert_eq!(&text.as_bytes()[..=1200], &cr.body[..]);
    assert_eq!((0, 1200), (cr.range.start, cr.range.end));
    assert_eq!(text.len().to_string(), cr.size);
}

#[test]
fn a_multi_range_206_is_sent_as_is() {
    let text = "<p>partial content</p>".repeat(200);
    let mut response = partial(&[(0, 999), (2000, 2999)], &text);
    apply_gzip(&make_request_with_gzip(), &mut response);
    assert!(!is_gzipped(&response));
    assert_eq!(&text.as_bytes()[2000..=2999], &response.content_range_list[1].body[..]);
}
