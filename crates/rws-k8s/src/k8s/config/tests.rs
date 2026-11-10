use std::collections::BTreeMap;

use k8s_openapi::api::core::v1::{ConfigMap, Secret};
use k8s_openapi::ByteString;

use super::*;

#[test]
fn data_is_read_from_both_maps() {
    let cm = ConfigMap {
        data: Some(BTreeMap::from([("a".to_string(), "1".to_string())])),
        binary_data: Some(BTreeMap::from([("b".to_string(), ByteString(b"two".to_vec())), ("bin".to_string(), ByteString(vec![0xff, 0xfe]))])),
        ..Default::default()
    };
    assert_eq!(BTreeMap::from([("a".to_string(), "1".to_string()), ("b".to_string(), "two".to_string())]), config_map_data(&cm));
    let secret = Secret {
        data: Some(BTreeMap::from([("password".to_string(), ByteString(b"s3cret".to_vec()))])),
        string_data: Some(BTreeMap::from([("user".to_string(), "app".to_string())])),
        ..Default::default()
    };
    assert_eq!(Some("s3cret"), secret_data(&secret).get("password").map(String::as_str));
    assert_eq!(Some("app"), secret_data(&secret).get("user").map(String::as_str));
}

#[test]
fn secret_references_parse() {
    assert_eq!(SecretRef { namespace: "prod".into(), name: "db".into(), key: "password".into() }, SecretRef::parse("k8s://prod/db#password").unwrap());
    assert_eq!("db", SecretRef::parse("k8s://db#password").unwrap().name);
    for bad in ["vault://x#y", "k8s://prod/db", "k8s://prod/#k", "k8s:///db#k", "k8s://prod/db#", "k8s://a/b/c#k"] {
        assert!(SecretRef::parse(bad).is_err(), "{bad}");
    }
}
