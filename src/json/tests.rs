use crate::json::{JSON_TYPE, JSONValue};
use crate::null::Null;

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
fn to_string() {
    let mut json_value = JSONValue::new();
    json_value.f64 = Some(49.2569999999996);
    let to_string : String = json_value.to_string();
    assert_eq!("49.2569999999996", to_string);
}

#[test]
fn to_string_2() {
    let mut json_value = JSONValue::new();
    json_value.f64 = Some(0.0);
    let to_string : String = json_value.to_string();
    assert_eq!("0.0000000000000", to_string);
}

#[test]
fn to_string_3() {
    let mut json_value = JSONValue::new();
    json_value.f64 = Some(-11.1);
    let to_string : String = json_value.to_string();
    assert_eq!("-11.1000000000000", to_string);
}