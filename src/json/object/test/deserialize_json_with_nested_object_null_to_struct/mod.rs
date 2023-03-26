use crate::json::object::FromJSON;
use crate::json::object::test::deserialize_json_with_nested_object_null_to_struct::nested_object::NestedObject;
use crate::json::object::test::deserialize_json_with_nested_object_null_to_struct::some_object::SomeObject;

mod nested_object;
mod some_object;

#[test]
fn deserialize_json_with_nested_object_null_to_struct() {

    let json_string = "{\r\n  \"prop_a\": \"123abc\",\r\n  \"prop_b\": true,\r\n  \"prop_c\": false,\r\n  \"prop_d\": 4356257,\r\n  \"prop_e\": 4356.257,\r\n  \"prop_f\": null\r\n}";


    let mut deserealized_object = SomeObject {
        prop_a: "".to_string(),
        prop_b: false,
        prop_c: true,
        prop_d: 0,
        prop_e: 0.0,
        prop_f: Some(NestedObject{ prop_foo: true }),
    };
    deserealized_object.parse(json_string.to_string()).unwrap();

    assert_eq!("123abc", deserealized_object.prop_a);
    assert_eq!(true, deserealized_object.prop_b);
    assert_eq!(false, deserealized_object.prop_c);
    assert_eq!(4356257, deserealized_object.prop_d);
    assert_eq!(4356.257, deserealized_object.prop_e);
    assert!(deserealized_object.prop_f.is_none());
}