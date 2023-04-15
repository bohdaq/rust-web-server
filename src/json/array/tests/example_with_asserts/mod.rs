mod example_object;
mod example_nested_object;

use file_ext::FileExt;
use crate::core::New;
use crate::json::array::tests::example_with_asserts::example_object::ExampleObject;

#[test]
fn vector_to_json() {
    let first_object = ExampleObject::new();

    let second_object = ExampleObject {
        prop_a: "test".to_string(),
        prop_b: true,
        prop_c: false,
        prop_d: 10,
        prop_e: 2.2,
        prop_f: None,
        prop_g: None,
    };

    let list  = vec![first_object, second_object];


    let actual = ExampleObject::to_json_list(list).unwrap();


    // expected json string
    let path = FileExt::build_path(&["src", "json", "array", "tests", "example", "list.example_object.to.json"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();
    let expected = String::from_utf8(file_as_bytes).unwrap();


    assert_eq!(actual, expected);
}

#[test]
fn json_to_vector() {
    // retrieve json string, in this example it is done via reading a file
    let path = FileExt::build_path(&["src", "json", "array", "tests", "example", "list.example_object.from.formatted.json"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();
    let json = String::from_utf8(file_as_bytes).unwrap();


    //  parse json String
    let parsed_list : Vec<ExampleObject> = ExampleObject::from_json_list(json).unwrap();
    assert_eq!(2, parsed_list.len());

    let parsed_obj = parsed_list.get(0).unwrap();
    assert_eq!(parsed_obj.prop_a, "");
    assert_eq!(parsed_obj.prop_b, false);
    assert_eq!(parsed_obj.prop_c, false);
    assert_eq!(parsed_obj.prop_d, 0);
    assert_eq!(parsed_obj.prop_e, 0.0);

    let parsed_obj = parsed_list.get(1).unwrap();
    assert_eq!(parsed_obj.prop_a, "test");
    assert_eq!(parsed_obj.prop_b, true);
    assert_eq!(parsed_obj.prop_c, false);
    assert_eq!(parsed_obj.prop_d, 10);
    assert_eq!(parsed_obj.prop_e, 2.2);
}
