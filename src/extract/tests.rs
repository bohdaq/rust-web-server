use super::*;
use crate::http::VERSION;

fn request_with_body(body: Vec<u8>) -> Request {
    Request {
        method: "POST".to_string(),
        request_uri: "/test".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body,
    }
}

fn request_with_uri(uri: &str) -> Request {
    Request {
        method: "GET".to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    }
}

#[test]
fn body_clones_raw_bytes() {
    let bytes = vec![1u8, 2, 3, 4];
    let req = request_with_body(bytes.clone());
    let Body(extracted) = Body::from_request(&req).unwrap();
    assert_eq!(bytes, extracted);
}

#[test]
fn body_empty_request_gives_empty_vec() {
    let req = request_with_body(vec![]);
    let Body(extracted) = Body::from_request(&req).unwrap();
    assert!(extracted.is_empty());
}

#[test]
fn body_text_valid_utf8() {
    let req = request_with_body("hello world".as_bytes().to_vec());
    let BodyText(text) = BodyText::from_request(&req).unwrap();
    assert_eq!("hello world", text);
}

#[test]
fn body_text_invalid_utf8_returns_400() {
    let req = request_with_body(vec![0xFF, 0xFE]);
    let err = BodyText::from_request(&req).unwrap_err();
    assert_eq!(400, err.status_code);
}

#[test]
fn query_no_params_gives_empty_map() {
    let req = request_with_uri("/search");
    let Query(params) = Query::from_request(&req).unwrap();
    assert!(params.is_empty());
}

#[test]
fn query_single_param() {
    let req = request_with_uri("/search?q=rust");
    let q = Query::from_request(&req).unwrap();
    assert_eq!(Some(&"rust".to_string()), q.get("q"));
}

#[test]
fn query_multiple_params() {
    let req = request_with_uri("/search?q=rust&page=2");
    let q = Query::from_request(&req).unwrap();
    assert_eq!(Some(&"rust".to_string()), q.get("q"));
    assert_eq!(Some(&"2".to_string()), q.get("page"));
}

#[test]
fn query_ignores_path_before_question_mark() {
    let req = request_with_uri("/api/v1/users?id=42");
    let q = Query::from_request(&req).unwrap();
    assert_eq!(Some(&"42".to_string()), q.get("id"));
    assert_eq!(None, q.get("api"));
}

#[test]
fn request_headers_get_case_insensitive() {
    let req = Request {
        method: "GET".to_string(),
        request_uri: "/".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![Header { name: "Content-Type".to_string(), value: "text/plain".to_string() }],
        body: vec![],
    };
    let headers = RequestHeaders::from_request(&req).unwrap();
    assert_eq!(Some("text/plain"), headers.get("content-type"));
    assert_eq!(Some("text/plain"), headers.get("CONTENT-TYPE"));
    assert_eq!(None, headers.get("Authorization"));
}

#[test]
fn request_headers_empty_request() {
    let req = request_with_uri("/");
    let headers = RequestHeaders::from_request(&req).unwrap();
    assert_eq!(None, headers.get("content-type"));
}

// ── #[derive(FromRequest)] ────────────────────────────────────────────────────

#[cfg(feature = "macros")]
mod derive {
    use crate::extract::{Body, BodyText, FromRequest, Query};
    use crate::http::VERSION;
    use crate::request::Request;

    fn make_req(uri: &str, body: &[u8]) -> Request {
        Request {
            method: "POST".to_string(),
            request_uri: uri.to_string(),
            http_version: VERSION.http_1_1.to_string(),
            headers: vec![],
            body: body.to_vec(),
        }
    }

    #[derive(Debug, rust_web_server::FromRequest)]
    struct Payload {
        body: BodyText,
        query: Query,
    }

    #[test]
    fn all_fields_extracted() {
        let req = make_req("/items?page=3", b"hello");
        let p = Payload::from_request(&req).unwrap();
        assert_eq!("hello", p.body.as_str());
        assert_eq!(Some(&"3".to_string()), p.query.get("page"));
    }

    #[test]
    fn first_failure_short_circuits() {
        let req = make_req("/", &[0xFF, 0xFE]); // invalid UTF-8 → BodyText fails → 400
        let err = Payload::from_request(&req).unwrap_err();
        assert_eq!(400, err.status_code);
    }

    #[derive(rust_web_server::FromRequest)]
    struct JustBody {
        body: Body,
    }

    #[test]
    fn single_field_body() {
        let req = make_req("/", b"raw bytes");
        let j = JustBody::from_request(&req).unwrap();
        assert_eq!(b"raw bytes".to_vec(), j.body.into_bytes());
    }

    #[derive(rust_web_server::FromRequest)]
    struct Empty {}

    #[test]
    fn empty_struct_ok() {
        let req = make_req("/", b"");
        assert!(Empty::from_request(&req).is_ok());
    }

    #[derive(rust_web_server::FromRequest)]
    struct MultiQuery {
        query: Query,
    }

    #[test]
    fn query_field_no_params_gives_empty_map() {
        let req = make_req("/path", b"");
        let m = MultiQuery::from_request(&req).unwrap();
        assert!(m.query.get("missing").is_none());
    }
}
