mod example_object;
mod example_nested_object;

use file_ext::FileExt;
use crate::json::object::{FromJSON, ToJSON};
use crate::json::object::tests::deserialize_json_to_struct_another_example::example_object::ExampleObject;

#[test]
fn deserialize_json_to_struct_another_example() {
    let obj = ExampleObject {
        prop_a: "123abc".to_string(),
        prop_b: true,
        prop_c: false,
        prop_d: 4356257,
        prop_e: 4356.257,
        prop_f: None,
        prop_g: None,
    };

    let json_string = obj.to_json_string();

    let path = FileExt::build_path(&["src", "json", "object", "tests", "deserialize_json_to_struct_another_example", "example-object.json"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();
    let expected_json_string = String::from_utf8(file_as_bytes).unwrap();

    assert_eq!(expected_json_string, json_string);

    let mut deserealized_object = ExampleObject {
        prop_a: "".to_string(),
        prop_b: false,
        prop_c: true,
        prop_d: 0,
        prop_e: 0.0,
        prop_f: None,
        prop_g: None,
    };
    deserealized_object.parse(json_string.to_string()).unwrap();

    assert_eq!("123abc", deserealized_object.prop_a);
    assert_eq!(true, deserealized_object.prop_b);
    assert_eq!(false, deserealized_object.prop_c);
    assert_eq!(4356257, deserealized_object.prop_d);
    assert_eq!(4356.257, deserealized_object.prop_e);
}