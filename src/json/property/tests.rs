use crate::json::{JSON_TYPE};
use crate::json::property::{JSONProperty};

#[test]
fn parse_raw_property_string() {
    let property_key = "key";
    let property_value = "some data";
    let property_type = JSON_TYPE.string;

    let raw_string = format!("\"{}\": \"{}\"", property_key, property_value);
    let (key, value) = JSONProperty::parse(&raw_string).unwrap();

    assert_eq!(key.property_name, property_key);
    assert_eq!(key.property_type, property_type);
    assert_eq!(value.string.unwrap(), property_value);
}

#[test]
fn parse_raw_property_null() {
    let property_key = "key";
    let property_value = "null";
    let property_type = JSON_TYPE.string;

    let raw_string = format!("\"{}\": {}", property_key, property_value);
    let (key, value) = JSONProperty::parse(&raw_string).unwrap();

    assert_eq!(key.property_name, property_key);
    assert_eq!(key.property_type, property_type);
    assert!(value.null.is_some());
}

#[test]
fn parse_raw_property_number_integer() {
    let property_key = "key";
    let property_value = "255";
    let property_type = JSON_TYPE.integer;

    let raw_string = format!("\"{}\": {}", property_key, property_value);
    let (key, value) = JSONProperty::parse(&raw_string).unwrap();

    assert_eq!(key.property_name, property_key);
    assert_eq!(key.property_type, property_type);
    assert_eq!(value.i128.unwrap(), property_value.parse::<i128>().unwrap());
}

#[test]
fn parse_raw_property_number_float() {
    let property_key = "key";
    let property_value = "255.200";
    let property_type = JSON_TYPE.number;

    let raw_string = format!("\"{}\": {}", property_key, property_value);
    let (key, value) = JSONProperty::parse(&raw_string).unwrap();

    assert_eq!(key.property_name, property_key);
    assert_eq!(key.property_type, property_type);
    assert_eq!(value.f64.unwrap(), property_value.parse::<f64>().unwrap());
}

#[test]
fn parse_raw_property_boolean_true() {
    let property_key = "key";
    let property_value = "true";
    let property_type = JSON_TYPE.boolean;

    let raw_string = format!("\"{}\": {}", property_key, property_value);
    let (key, value) = JSONProperty::parse(&raw_string).unwrap();

    assert_eq!(key.property_name, property_key);
    assert_eq!(key.property_type, property_type);
    assert_eq!(value.bool.unwrap(), property_value.parse::<bool>().unwrap());
}

#[test]
fn parse_raw_property_boolean_false() {
    let property_key = "key";
    let property_value = "false";
    let property_type = JSON_TYPE.boolean;

    let raw_string = format!("\"{}\": {}", property_key, property_value);
    let (key, value) = JSONProperty::parse(&raw_string).unwrap();

    assert_eq!(key.property_name, property_key);
    assert_eq!(key.property_type, property_type);
    assert_eq!(value.bool.unwrap(), property_value.parse::<bool>().unwrap());
}

#[test]
fn parse_raw_property_number_float_parse_error() {
    let property_key = "key";
    let property_value = "255.200asdf";

    let raw_string = format!("\"{}\": {}", property_key, property_value);
    let error_message = JSONProperty::parse(&raw_string).err().unwrap();

    assert_eq!("unable to parse number: \"key\": 255.200asdf", error_message);
}

#[test]
fn parse_raw_property_number_integer_parse_error() {
    let property_key = "key";
    let property_value = "255200asdf";

    let raw_string = format!("\"{}\": {}", property_key, property_value);
    let error_message = JSONProperty::parse(&raw_string).err().unwrap();

    assert_eq!("unable to parse number: \"key\": 255200asdf", error_message);
}