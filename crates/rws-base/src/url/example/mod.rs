use std::collections::HashMap;
use url_build_parse::{UrlAuthority, UrlComponents};
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

#[test]
fn build_url() {
    let mut hash : HashMap<String, String> = HashMap::new();
    hash.insert("key".to_string(), "value".to_string());
    hash.insert("key&=!@".to_string(), "%val*ue%".to_string());

    let url_components = UrlComponents {
        scheme: "https".to_string(),
        authority: Some(UrlAuthority{
            user_info: None,
            host: "localhost".to_string(),
            port: None,
        }),
        path: "/path".to_string(),
        query: Some(hash),
        fragment: Some("fragment-sample".to_string()),
    };

    let url : String = URL::build(url_components).unwrap();
    assert_eq!("https://localhost/path?key%26%3D%21%40=%25val%2Aue%25&key=value#fragment-sample", url);

}

#[test]
fn parse_url() {
    let url = "https://localhost/path?key%26%3D%21%40=%25val%2Aue%25&key=value#fragment-sample";

    let url_components: UrlComponents = URL::parse(url).unwrap();
    assert_eq!("https", url_components.scheme);

    assert_eq!("/path", url_components.path);

    let fragment = url_components.fragment.unwrap();
    assert_eq!("fragment-sample", fragment);

    let authority = url_components.authority.unwrap();
    assert_eq!("localhost", authority.host);

    let query_map : HashMap<String, String> = url_components.query.unwrap();
    assert_eq!("value", query_map.get("key").unwrap());
    assert_eq!("%val*ue%", query_map.get("key&=!@").unwrap());
}