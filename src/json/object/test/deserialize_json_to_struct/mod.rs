use crate::json::JSON_TYPE;
use crate::json::object::test::deserialize_json_to_struct::some_object::SomeObject;
use crate::json::object::{FromJSON, ToJSON};

mod some_object;

#[test]
fn deserialize_json_to_struct() {
    let mut obj = SomeObject { prop_a: "123abc".to_string(), prop_b: true };

    let json_string = obj.to_json_string();
    let expected_json_string = "{\r\n  \"prop_a\": \"123abc\",\r\n  \"prop_b\": true\r\n}";

    assert_eq!(expected_json_string, json_string);

    let properties  = obj.parse_json_to_properties(json_string.to_string()).unwrap();
    assert_eq!(properties.len(), 2);

    let (prop_a_type, prop_a_value) = properties.get(0).unwrap();
    assert_eq!(prop_a_type.property_type, JSON_TYPE.string);
    assert_eq!(prop_a_type.property_name, "prop_a");
    assert_eq!(prop_a_value.string.clone().unwrap(), "123abc");


    let (prop_b_type, prop_b_value) = properties.get(1).unwrap();
    assert_eq!(prop_b_type.property_type, JSON_TYPE.boolean);
    assert_eq!(prop_b_type.property_name, "prop_b");
    assert_eq!(prop_b_value.bool.unwrap(), true);

    obj.set_properties(properties).unwrap();
    assert_eq!("123abc", obj.prop_a);
    assert_eq!(true, obj.prop_b);
}