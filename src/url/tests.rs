use std::collections::HashMap;
use url_build_parse::{UrlAuthority, UrlComponents};
use crate::url::URL;

// ── percent encoding edge cases ───────────────────────────────────────────────

#[test]
fn encode_decode() {
    let component = "\r\n \"%!#$&'()*+,/:;=?@[]][@?=;:/,+*)('&$#!%\" \r\n";
    let mut _result = URL::percent_encode(component);
    assert_eq!("%0D%0A%20%22%25%21%23%24%26%27%28%29%2A%2B%2C%2F%3A%3B%3D?%40%5B%5D%5D%5B%40?%3D%3B%3A%2F%2C%2B%2A%29%28%27%26%24%23%21%25%22%20%0D%0A", _result);
    _result = URL::percent_decode(_result.as_str());
    assert_eq!(component, _result);
}

#[test]
fn encode_empty_string() {
    assert_eq!("", URL::percent_encode(""));
}

#[test]
fn decode_empty_string() {
    assert_eq!("", URL::percent_decode(""));
}

#[test]
fn encode_plain_alphanumeric_unchanged() {
    let s = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    assert_eq!(s, URL::percent_encode(s));
}

#[test]
fn decode_already_decoded_string_unchanged() {
    let s = "hello world"; // no percent sequences
    // This should round-trip cleanly even if not encoded
    let encoded = URL::percent_encode(s);
    assert_eq!(s, URL::percent_decode(&encoded));
}

// ── parse_query edge cases ────────────────────────────────────────────────────

#[test]
fn parse_query_empty_string_returns_empty_map() {
    let map = URL::parse_query("");
    assert!(map.is_empty(), "empty query string should yield empty map");
}

#[test]
fn parse_query_single_key_value() {
    let map = URL::parse_query("foo=bar");
    assert_eq!(Some(&"bar".to_string()), map.get("foo"));
}

#[test]
fn parse_query_multiple_pairs() {
    let map = URL::parse_query("a=1&b=2&c=3");
    assert_eq!("1", map.get("a").unwrap());
    assert_eq!("2", map.get("b").unwrap());
    assert_eq!("3", map.get("c").unwrap());
}

// ── build_query edge cases ────────────────────────────────────────────────────

#[test]
fn build_query_empty_map_returns_empty_string() {
    let map: HashMap<String, String> = HashMap::new();
    let q = URL::build_query(map);
    assert_eq!("", q);
}

#[test]
fn build_query_single_entry() {
    let mut map = HashMap::new();
    map.insert("key".to_string(), "value".to_string());
    let q = URL::build_query(map);
    assert_eq!("key=value", q);
}

// ── URL::parse edge cases ─────────────────────────────────────────────────────

#[test]
fn parse_url_extracts_scheme_host_path() {
    let components = URL::parse("http://example.com/hello").unwrap();
    assert_eq!("http", components.scheme);
    assert_eq!("/hello", components.path);
    let auth = components.authority.unwrap();
    assert_eq!("example.com", auth.host);
}

#[test]
fn parse_url_with_port() {
    let components = URL::parse("http://localhost:8080/api").unwrap();
    let auth = components.authority.unwrap();
    assert_eq!("localhost", auth.host);
    assert_eq!(Some(8080), auth.port);
}

#[test]
fn parse_url_with_fragment() {
    let components = URL::parse("https://example.com/page#section").unwrap();
    assert_eq!(Some("section".to_string()), components.fragment);
}

#[test]
fn parse_url_without_query_has_none() {
    let components = URL::parse("https://example.com/path").unwrap();
    assert!(components.query.is_none());
}

// ── URL::build edge cases ─────────────────────────────────────────────────────

#[test]
fn build_url_minimal_no_query_no_fragment() {
    let url = URL::build(UrlComponents {
        scheme: "http".to_string(),
        authority: Some(UrlAuthority { user_info: None, host: "example.com".to_string(), port: None }),
        path: "/".to_string(),
        query: None,
        fragment: None,
    }).unwrap();
    assert_eq!("http://example.com/", url);
}

#[test]
fn build_url_with_port() {
    let url = URL::build(UrlComponents {
        scheme: "http".to_string(),
        authority: Some(UrlAuthority { user_info: None, host: "localhost".to_string(), port: Some(3000) }),
        path: "/api".to_string(),
        query: None,
        fragment: None,
    }).unwrap();
    assert_eq!("http://localhost:3000/api", url);
}

#[test]
fn build_parse_roundtrip() {
    let original = "https://api.example.com/v1/users";
    let components = URL::parse(original).unwrap();
    let rebuilt = URL::build(components).unwrap();
    assert_eq!(original, rebuilt);
}