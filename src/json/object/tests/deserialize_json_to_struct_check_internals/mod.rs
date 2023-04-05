use file_ext::FileExt;
use crate::json::JSON_TYPE;
use crate::json::object::{FromJSON, ToJSON};
use crate::json::object::tests::deserialize_json_to_struct_check_internals::some_object::SomeObject;

mod some_object;

#[test]
fn deserialize_json_to_struct_check_internals() {
    let mut obj = SomeObject { prop_a: "123abc".to_string(), prop_b: true };
    let json_string = obj.to_json_string();

    let path = FileExt::build_path(&["src", "json", "object", "tests", "deserialize_json_to_struct_check_internals", "some-object.json"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();
    let expected_json_string = String::from_utf8(file_as_bytes).unwrap();

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