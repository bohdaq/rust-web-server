mod example_object;
mod example_nested_object;

use file_ext::FileExt;
use crate::json::object::{FromJSON};
use crate::json::object::tests::deserialize_json_missing_comma_after_boolean_true::example_object::ExampleObject;

#[test]
fn deserialize_json_missing_comma_after_boolean_true() {
    let path = FileExt::build_path(&["src", "json", "object", "tests", "deserialize_json_missing_comma_after_boolean_true", "example-object.json"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();
    let json_missing_comma = String::from_utf8(file_as_bytes).unwrap();

    let mut parsed_json = ExampleObject {
        prop_a: "qwerty".to_string(),
        prop_b: false,
        prop_c: false,
        prop_d: 0,
        prop_e: 0.0,
        prop_f: None,
        prop_g: None,
    };
    let json_without_comma = parsed_json.parse(json_missing_comma);
    assert!(json_without_comma.is_err());

    let message = json_without_comma.err().unwrap();
    assert_eq!("before comma there are some unexpected characters: \r\n  \"prop_c\": false,", message);


}