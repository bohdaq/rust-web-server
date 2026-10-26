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
            name: Header::_ACCEPT_ENCODING.into(),
            value: "gzip, deflate".into(),
        }],
        body: Default::default(),
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
        body: Default::default(),
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
        name: Header::_VARY.into(),
        value: "Origin".into(),
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
        name: Header::_VARY.into(),
        value: "Accept-Encoding".into(),
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
    response.headers.push(Header { name: "content-encoding".into(), value: "br".into() });
    apply_gzip(&make_request_with_gzip(), &mut response);
    assert_eq!(response.headers.iter().filter(|h| h.name.eq_ignore_ascii_case("content-encoding")).count(), 1);
    assert_eq!(response.content_range_list[0].body, body.as_bytes());
}


fn request_accepting(value: &str) -> Request {
    let mut r = make_request_with_gzip();
    r.headers = vec![Header { name: Header::_ACCEPT_ENCODING.into(), value: value.to_string().into() }];
    r
}

#[test]
fn accept_encoding_is_parsed_per_coding() {
    let html = "<p>compress me</p>".repeat(64);
    for (value, expect) in [("GZIP", true), ("br, gzip;q=0.5", true), ("gzip;q=0", false), ("gzip; q=0.0", false), ("x-gzip-not", false), ("br", false)] {
        let mut response = make_response(html.as_bytes(), MimeType::TEXT_HTML);
        apply_gzip(&request_accepting(value), &mut response);
        assert_eq!(expect, is_gzipped(&response), "Accept-Encoding: {value}");
    }
}

#[test]
fn content_type_match_ignores_case() {
    let html = "<p>compress me</p>".repeat(64);
    let mut response = make_response(html.as_bytes(), "Text/HTML; charset=utf-8");
    apply_gzip(&make_request_with_gzip(), &mut response);
    assert!(is_gzipped(&response));
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

// ── 18.1.0: brotli and zstd, negotiated ─────────────────────────────────────

use crate::compression::{apply_compression, encode, negotiate, Encoding, Level, DYNAMIC_PREFERENCE, STATIC_PREFERENCE};

fn accepting(value: &str) -> Request {
    let mut request = make_request_with_gzip();
    request.headers = vec![Header { name: Header::_ACCEPT_ENCODING.into(), value: value.to_string().into() }];
    request
}

fn decode(encoding: Encoding, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    match encoding {
        Encoding::Gzip => {
            GzDecoder::new(body).read_to_end(&mut out).unwrap();
        }
        #[cfg(feature = "brotli")]
        Encoding::Brotli => {
            brotli::Decompressor::new(body, 4096).read_to_end(&mut out).unwrap();
        }
        #[cfg(feature = "zstd")]
        Encoding::Zstd => out = zstd::bulk::decompress(body, 10 << 20).unwrap(),
        #[allow(unreachable_patterns)]
        _ => panic!("{encoding:?} not compiled in"),
    }
    out
}

#[test]
fn negotiation_follows_q_values_then_preference() {
    let pick = |value: &str, preference: &[Encoding]| negotiate(&accepting(value), preference);
    let all = cfg!(all(feature = "brotli", feature = "zstd"));
    // No header: nothing.
    assert_eq!(None, negotiate(&Request { headers: vec![], ..make_request_with_gzip() }, DYNAMIC_PREFERENCE));
    assert_eq!(Some(Encoding::Gzip), pick("gzip", DYNAMIC_PREFERENCE));
    assert_eq!(None, pick("gzip;q=0", DYNAMIC_PREFERENCE));
    assert_eq!(None, pick("deflate, identity", DYNAMIC_PREFERENCE));
    if all {
        // A browser's list: equal weights, so the server's order decides.
        assert_eq!(Some(Encoding::Zstd), pick("gzip, deflate, br, zstd", DYNAMIC_PREFERENCE));
        assert_eq!(Some(Encoding::Brotli), pick("gzip, deflate, br, zstd", STATIC_PREFERENCE));
        // A higher q wins over the server's order.
        assert_eq!(Some(Encoding::Gzip), pick("gzip;q=1, br;q=0.5, zstd;q=0.4", DYNAMIC_PREFERENCE));
        assert_eq!(Some(Encoding::Brotli), pick("BR;Q=0.9, zstd;q=0.8", DYNAMIC_PREFERENCE));
        // `*` stands for codings not listed; an explicit q=0 still refuses.
        assert_eq!(Some(Encoding::Zstd), pick("*", DYNAMIC_PREFERENCE));
        assert_eq!(Some(Encoding::Brotli), pick("*;q=0.5, zstd;q=0", DYNAMIC_PREFERENCE));
        assert_eq!(None, pick("*;q=0", DYNAMIC_PREFERENCE));
        // An unparsable or out-of-range q counts as 1.
        assert_eq!(Some(Encoding::Zstd), pick("gzip;q=0.9, zstd;q=abc", DYNAMIC_PREFERENCE));
    }
    // Restricting the preference restricts the result.
    assert_eq!(Some(Encoding::Gzip), pick("gzip, br, zstd", &[Encoding::Gzip]));
}

#[test]
fn every_compiled_encoding_round_trips_at_both_levels() {
    let text = "<p>The quick brown fox jumps over the lazy dog.</p>\n".repeat(400);
    for encoding in [Encoding::Gzip, Encoding::Brotli, Encoding::Zstd] {
        if !encoding.available() {
            assert_eq!(None, encode(encoding, text.as_bytes(), Level::Dynamic));
            continue;
        }
        for level in [Level::Dynamic, Level::Static] {
            let encoded = encode(encoding, text.as_bytes(), level).unwrap();
            assert!(encoded.len() < text.len() / 5, "{encoding:?} {level:?}: {} bytes", encoded.len());
            assert_eq!(text.as_bytes(), &decode(encoding, &encoded)[..], "{encoding:?} {level:?}");
        }
    }
}

#[test]
fn per_request_compression_uses_the_fast_levels() {
    // Brotli's default quality (11) is ~100x slower than gzip; on-the-fly
    // compression must stay at the fast levels.
    assert_eq!([4, 6], crate::compression::BROTLI_QUALITY);
    assert_eq!([3, 6], crate::compression::ZSTD_LEVEL);
}

#[test]
fn apply_compression_sends_the_negotiated_encoding() {
    let html = "<html>hello world</html>".repeat(64);
    let expected = if cfg!(feature = "zstd") { Encoding::Zstd } else if cfg!(feature = "brotli") { Encoding::Brotli } else { Encoding::Gzip };
    let mut response = make_response(html.as_bytes(), MimeType::TEXT_HTML);
    apply_compression(&accepting("gzip, deflate, br, zstd"), &mut response);
    let header = |name: &str| response.headers.iter().find(|h| h.name.eq_ignore_ascii_case(name)).map(|h| h.value.to_string());
    assert_eq!(Some(expected.token().to_string()), header("Content-Encoding"));
    assert_eq!(Some("Accept-Encoding".to_string()), header("Vary"));
    let body = &response.content_range_list[0].body;
    assert_eq!(html.as_bytes(), &decode(expected, body)[..]);
    assert_eq!(body.len().to_string(), response.content_range_list[0].size);

    // apply_gzip still only ever gzips.
    let mut gzip_only = make_response(html.as_bytes(), MimeType::TEXT_HTML);
    apply_gzip(&accepting("br, zstd, gzip;q=0.1"), &mut gzip_only);
    assert!(gzip_only.headers.iter().any(|h| h.name.eq_ignore_ascii_case("Content-Encoding") && &*h.value == "gzip"));
}

#[test]
fn a_compressed_body_gets_its_own_etag() {
    let html = "<html>hello world</html>".repeat(64);
    for (etag, expected) in [("\"abc\"", "\"abc-gzip\""), ("W/\"abc\"", "W/\"abc-gzip\"")] {
        let mut response = make_response(html.as_bytes(), MimeType::TEXT_HTML);
        response.headers.push(Header { name: Header::_ETAG.into(), value: etag.into() });
        apply_compression(&accepting("gzip"), &mut response);
        let tag = response.headers.iter().find(|h| h.name.eq_ignore_ascii_case("ETag")).map(|h| h.value.to_string());
        assert_eq!(Some(expected.to_string()), tag);
    }
    // Not compressed (no Accept-Encoding): the ETag is left alone.
    let mut plain = make_response(html.as_bytes(), MimeType::TEXT_HTML);
    plain.headers.push(Header { name: Header::_ETAG.into(), value: "\"abc\"".into() });
    apply_compression(&Request { headers: vec![], ..make_request_with_gzip() }, &mut plain);
    assert!(plain.headers.iter().any(|h| &*h.value == "\"abc\""));
}

#[test]
fn a_parameter_starting_with_a_multibyte_character_is_not_a_panic() {
    // `p[..2]` on "aé" splits the "é": found by the accept_encoding fuzzer;
    // the same check was in 17.x's gzip negotiation.
    for value in ["gzip;aé", "gzip;é", "br; \u{1F600}x", "zstd;q\u{e9}=1"] {
        let request = accepting(value);
        let _ = negotiate(&request, DYNAMIC_PREFERENCE);
        let mut response = make_response("<p>x</p>".repeat(200).as_bytes(), MimeType::TEXT_HTML);
        apply_compression(&request, &mut response);
    }
    assert_eq!(Some(Encoding::Gzip), negotiate(&accepting("gzip;aé"), &[Encoding::Gzip]));
}
