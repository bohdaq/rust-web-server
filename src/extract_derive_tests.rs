//! `#[derive(FromRequest)]` (rws-macros) against the extractors in
//! `rust_web_server::extract`: the derive names rust-web-server paths, so
//! these tests live in the facade, not in rws-http with the extractors.

use crate::extract::{Body, BodyText, FromRequest, Query};
use crate::http::VERSION;
use crate::request::Request;

fn make_req(uri: &str, body: &[u8]) -> Request {
    Request {
        method: "POST".to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: body.to_vec().into(),
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
