use crate::json::object::FromJSON;
use crate::json::object::test::deserialize_json_with_nested_object_null_field_to_struct::some_object::SomeObject;

mod nested_object;
mod some_object;

#[test]
fn parse_nested_object_property_null() {
    let json_string = "{\r\n  \"prop_a\": \"123abc\",\r\n  \"prop_b\": true,\r\n  \"prop_c\": false,\r\n  \"prop_d\": 4356257,\r\n  \"prop_e\": 4356.257,\r\n  \"prop_f\": {\r\n  \"prop_foo\": null\r\n}\r\n}";


    let mut deserealized_object = SomeObject {
        prop_a: "".to_string(),
        prop_b: false,
        prop_c: true,
        prop_d: 0,
        prop_e: 0.0,
        prop_f: None,
    };
    deserealized_object.parse(json_string.to_string()).unwrap();

    assert_eq!("123abc", deserealized_object.prop_a);
    assert_eq!(true, deserealized_object.prop_b);
    assert_eq!(false, deserealized_object.prop_c);
    assert_eq!(4356257, deserealized_object.prop_d);
    assert_eq!(4356.257, deserealized_object.prop_e);
    assert_eq!(false, deserealized_object.prop_f.unwrap().prop_foo);
}