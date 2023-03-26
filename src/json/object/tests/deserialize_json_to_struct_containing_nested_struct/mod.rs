use file_ext::FileExt;
use crate::json::object::tests::deserialize_json_to_struct_containing_nested_struct::nested_object::NestedObject;
use crate::json::object::tests::deserialize_json_to_struct_containing_nested_struct::some_object::SomeObject;
use crate::json::object::{FromJSON, ToJSON};

mod some_object;
mod nested_object;

#[test]
fn deserialize_json_to_struct_containing_nested_struct() {
    let nested_obj = NestedObject { prop_foo: true };
    let obj = SomeObject {
        prop_a: "123abc".to_string(),
        prop_b: true,
        prop_c: false,
        prop_d: 4356257,
        prop_e: 4356.257,
        prop_f: Some(nested_obj),
    };

    let json_string = obj.to_json_string();
    // 'to_json_string' does not perform indentation on nested objects and arrays
    // take a look at 'some-object-formatted.json' for properly indented human readable json
    let path = FileExt::build_path(&["src", "json", "object", "tests", "deserialize_json_to_struct_containing_nested_struct", "some-object.json"]);
    let pwd = FileExt::working_directory().unwrap();

    let file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let file_as_bytes = FileExt::read_file(file_path.as_str()).unwrap();
    let expected_json_string = String::from_utf8(file_as_bytes).unwrap();

    assert_eq!(expected_json_string, json_string);

    let mut deserialized_object = SomeObject {
        prop_a: "".to_string(),
        prop_b: false,
        prop_c: true,
        prop_d: 0,
        prop_e: 0.0,
        prop_f: None,
    };
    deserialized_object.parse(json_string.to_string()).unwrap();

    assert_eq!("123abc", deserialized_object.prop_a);
    assert_eq!(true, deserialized_object.prop_b);
    assert_eq!(false, deserialized_object.prop_c);
    assert_eq!(4356257, deserialized_object.prop_d);
    assert_eq!(4356.257, deserialized_object.prop_e);
    assert_eq!(true, deserialized_object.prop_f.unwrap().prop_foo);
}