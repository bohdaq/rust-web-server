use crate::json::{JSON_TYPE, Null};

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