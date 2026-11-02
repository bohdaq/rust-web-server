use crate::json::{JSON_TYPE};
use crate::core::New;
use crate::json::property::JSONValue;
use crate::null::{Null, NULL};

#[test]
fn json_types() {
    assert_eq!(JSON_TYPE.object, "object");
    assert_eq!(JSON_TYPE.string, "String");
    assert_eq!(JSON_TYPE.boolean, "bool");
    assert_eq!(JSON_TYPE.array, "array");
    assert_eq!(JSON_TYPE.integer, "i128");
    assert_eq!(JSON_TYPE.number, "f64");
    assert_eq!(JSON_TYPE.null, "null");

    let null = Null {};
    assert!(Some(null).is_some());
}

#[test]
fn to_string_f64() {
    let mut json_value = JSONValue::new();
    json_value.f64 = Some(49.2569999999996);
    let to_string : String = json_value.to_string();
    assert_eq!("49.2569999999996", to_string);
}

#[test]
fn to_string_f64_2() {
    let mut json_value = JSONValue::new();
    json_value.f64 = Some(0.0);
    let to_string : String = json_value.to_string();
    assert_eq!("0.0000000000000", to_string);
}

#[test]
fn to_string_f64_3() {
    let mut json_value = JSONValue::new();
    json_value.f64 = Some(-11.1);
    let to_string : String = json_value.to_string();
    assert_eq!("-11.1000000000000", to_string);
}

#[test]
fn to_string_int() {
    let mut json_value = JSONValue::new();
    json_value.i128 = Some(-11);
    let to_string : String = json_value.to_string();
    assert_eq!("-11", to_string);
}

#[test]
fn to_string_string() {
    let mut json_value = JSONValue::new();
    json_value.string = Some("text".to_string());
    let to_string = json_value.to_string();
    assert_eq!("text", to_string);
}

#[test]
fn to_string_array() {
    let mut json_value = JSONValue::new();
    json_value.array = Some("[1, 2]".to_string());
    let to_string = json_value.to_string();
    assert_eq!("[1, 2]", to_string);
}

#[test]
fn to_string_null() {
    let mut json_value = JSONValue::new();
    json_value.null = Some(NULL.clone());
    let to_string = json_value.to_string();
    assert_eq!("null", to_string);
}

#[test]
fn to_string_object() {
    let mut json_value = JSONValue::new();
    json_value.object = Some("{ a: 1 }".to_string());
    let to_string = json_value.to_string();
    assert_eq!("{ a: 1 }", to_string);
}

#[test]
fn to_string_boolean() {
    let mut json_value = JSONValue::new();
    json_value.bool = Some(true);
    let to_string = json_value.to_string();
    assert_eq!("true", to_string);
}

#[test]
fn to_string_not_set() {
    let json_value = JSONValue::new();
    let to_string = json_value.to_string();
    assert_eq!("Something Went Wrong. There is no value for any type.", to_string);
}

#[test]
fn to_string_f64_precision() {
    let mut json_value = JSONValue::new();
    json_value.f64 = Some(-11.1);
    let to_string : String = json_value.float_number_with_precision(2);
    assert_eq!("-11.10", to_string);
}

#[test]
fn to_string_f64_precision_2() {
    let mut json_value = JSONValue::new();
    json_value.f64 = Some(1.100056);
    let to_string : String = json_value.float_number_with_precision(5);
    assert_eq!("1.10006", to_string);
}

#[test]
fn to_string_f64_precision_3() {
    let mut json_value = JSONValue::new();
    json_value.f64 = Some(0.0);
    let to_string : String = json_value.float_number_with_precision(5);
    assert_eq!("0.00000", to_string);
}