use serde::{Deserialize, Serialize};

use crate::http::VERSION;
use crate::json::Json;
use crate::mime_type::MimeType;
use crate::request::{METHOD, Request};

fn post_json(body: &[u8]) -> Request {
    Request {
        method: METHOD.post.to_string(),
        request_uri: "/".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: body.to_vec(),
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
struct User {
    name: String,
    age: u32,
}

// ── from_request (deserialize) ────────────────────────────────────────────────

#[test]
fn deserializes_valid_json_body() {
    let req = post_json(br#"{"name":"Alice","age":30}"#);
    let Json(user) = Json::<User>::from_request(&req).unwrap();
    assert_eq!("Alice", user.name);
    assert_eq!(30, user.age);
}

#[test]
fn invalid_json_returns_400() {
    let req = post_json(b"not json");
    let err = Json::<User>::from_request(&req).unwrap_err();
    assert_eq!(400, err.status_code);
}

#[test]
fn wrong_type_returns_400() {
    let req = post_json(br#"{"name":123}"#); // name should be string
    let err = Json::<User>::from_request(&req).unwrap_err();
    assert_eq!(400, err.status_code);
}

#[test]
fn empty_body_returns_400() {
    let req = post_json(b"");
    let err = Json::<User>::from_request(&req).unwrap_err();
    assert_eq!(400, err.status_code);
}

#[test]
fn deref_gives_access_to_inner() {
    let req = post_json(br#"{"name":"Bob","age":25}"#);
    let json = Json::<User>::from_request(&req).unwrap();
    assert_eq!("Bob", json.name); // via Deref
}

// ── into_response (serialize) ─────────────────────────────────────────────────

#[test]
fn serializes_to_200_with_json_body() {
    let user = User { name: "Carol".to_string(), age: 40 };
    let response = Json(user).into_response();
    assert_eq!(200, response.status_code);
}

#[test]
fn serialized_body_is_valid_json() {
    let user = User { name: "Dave".to_string(), age: 50 };
    let response = Json(user).into_response();
    let body = &response.content_range_list[0].body;
    let parsed: User = serde_json::from_slice(body).unwrap();
    assert_eq!("Dave", parsed.name);
    assert_eq!(50, parsed.age);
}

#[test]
fn into_response_content_type_is_application_json() {
    let response = Json(User { name: "Eve".to_string(), age: 20 }).into_response();
    assert_eq!(MimeType::APPLICATION_JSON, response.content_range_list[0].content_type);
}

#[test]
fn roundtrip_serialize_deserialize() {
    let original = User { name: "Frank".to_string(), age: 35 };
    let response = Json(User { name: "Frank".to_string(), age: 35 }).into_response();
    let body = &response.content_range_list[0].body;
    let parsed: User = serde_json::from_slice(body).unwrap();
    assert_eq!(original, parsed);
}

// ── FromRequest impl ──────────────────────────────────────────────────────────

#[test]
fn from_request_trait_impl_works() {
    use crate::extract::FromRequest;
    let req = post_json(br#"{"name":"Grace","age":28}"#);
    // Invoke via the trait to verify the impl compiles and dispatches correctly.
    let Json(user) = <Json<User> as FromRequest>::from_request(&req).unwrap();
    assert_eq!("Grace", user.name);
}
