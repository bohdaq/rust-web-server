mod example_nested_object;
mod example_object;

use file_ext::FileExt;
use crate::core::New;
use crate::json::array::object::example_multi_nested_object::example_nested_object::ExampleNestedObject;
use crate::json::array::object::example_multi_nested_object::example_object::ExampleObject;

#[test]
fn vector_to_json() {
    // first object
    let first_object = ExampleObject::new();

    // second object contains nested object and nested list
    // nested object
    let nested_object = ExampleNestedObject {
        prop_a: "test".to_string(),
        prop_b: false,
        prop_c: 1,
        prop_d: 2.2,
    };

    // nested list
    let first_object_from_nested_list = ExampleNestedObject {
        prop_a: "test".to_string(),
        prop_b: false,
        prop_c: 1,
        prop_d: 2.2,
    };

    let second_object_from_nested_list = ExampleNestedObject {
        prop_a: "test string".to_string(),
        prop_b: true,
        prop_c: 11,
        prop_d: 21.12,
    };

    let nested_list = vec![first_object_from_nested_list, second_object_from_nested_list];

    // second object itself
    let second_object = ExampleObject {
        prop_a: "test".to_string(),
        prop_b: true,
        prop_c: false,
        prop_d: 10,
        prop_e: 2.2,
        prop_f: Some(nested_list),
        prop_g: Some(nested_object),
    };


    let list  = vec![first_object, second_object];


    let _json_array : String = ExampleObject::to_json_list(list).unwrap();

}


#[test]
fn json_to_vector() {
    // retrieve json string, in this example it is done via reading a file
    let path = FileExt::build_path(&["src", "json", "array", "object", "example_multi_nested_object", "list.example_object.from.formatted.json"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();
    let json = String::from_utf8(file_as_bytes).unwrap();

    // parse json to vector
    let _example_object_list : Vec<ExampleObject> = ExampleObject::from_json_list(json).unwrap();
}