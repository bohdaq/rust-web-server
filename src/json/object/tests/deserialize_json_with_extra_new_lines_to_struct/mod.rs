mod example_object;
mod example_nested_object;

use file_ext::FileExt;
use crate::core::New;
use crate::json::object::FromJSON;
use crate::json::object::tests::deserialize_json_with_extra_new_lines_to_struct::example_object::ExampleObject;

#[test]
fn parse_new_lines_carriage_returns() {
    // using .txt extension here to prevent IDE autoformatting during development
    // take a look at 'some-object-formatted.json' for properly indented human readable json
    let path = FileExt::build_path(&["src", "json", "object", "tests", "deserialize_json_with_extra_new_lines_to_struct", "some-object.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let file_as_bytes = FileExt::read_file(file_path.as_str()).unwrap();
    let json_string = String::from_utf8(file_as_bytes).unwrap();

    let mut deserealized_object = ExampleObject::new();
    deserealized_object.parse(json_string.to_string()).unwrap();

    assert_eq!("123abc", deserealized_object.prop_a);
    assert_eq!(true, deserealized_object.prop_b);
    assert_eq!(false, deserealized_object.prop_c);
    assert_eq!(4356257, deserealized_object.prop_d);
    assert_eq!(4356.257, deserealized_object.prop_e);
}