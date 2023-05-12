use std::collections::HashMap;
use crate::url::URL;

#[test]
fn uri_encode() {
    let text = "some text to encode &=";
    let encoded = URL::percent_encode(text);
    assert_eq!("some%20text%20to%20encode%20%26%3D", encoded);
}

#[test]
fn uri_decode() {
    let text = "some%20text%20to%20encode%20%26%3D";
    let decoded = URL::percent_decode(text);
    assert_eq!("some text to encode &=", decoded);
}

#[test]
fn build_query() {
    let mut hash : HashMap<String, String> = HashMap::new();
    hash.insert("key".to_string(), "value".to_string());
    hash.insert("key&=!@".to_string(), "%val*ue%".to_string());

    let query : String = URL::build_query(hash);
    assert_eq!("key%26%3D%21%40=%25val%2Aue%25&key=value", query);
}

#[test]
fn parse_query() {
    let mut hash : HashMap<String, String> = HashMap::new();
    hash.insert("key".to_string(), "value".to_string());
    hash.insert("key&=!@".to_string(), "%val*ue%".to_string());

    let query = "key%26%3D%21%40=%25val%2Aue%25&key=value";
    let hash : HashMap<String, String> = URL::parse_query(query);

    assert_eq!("value", hash.get("key").unwrap());
    assert_eq!("%val*ue%", hash.get("key&=!@").unwrap());
}
