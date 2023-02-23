use crate::json::{JSON_TYPE, JSONType, Null};
use crate::json::key_value::parse_json_property;

#[test]
fn parse_raw_property_string() {
    let property_key = "key";
    let property_value = "some data";
    let property_type = JSON_TYPE.string;

    let raw_string = format!("\"{}\": \"{}\"", property_key, property_value);
    let (key, value) = parse_json_property(&raw_string).unwrap();

    assert_eq!(key.property_name, property_key);
    assert_eq!(key.property_type, property_type);
    assert_eq!(value.String.unwrap(), property_value);
}

#[test]
fn parse_raw_property_null() {
    let property_key = "key";
    let property_value = "null";
    let property_type = JSON_TYPE.string;

    let raw_string = format!("\"{}\": {}", property_key, property_value);
    let (key, value) = parse_json_property(&raw_string).unwrap();

    assert_eq!(key.property_name, property_key);
    assert_eq!(key.property_type, property_type);
    assert!(value.null.is_some());
}