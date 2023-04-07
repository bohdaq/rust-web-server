mod example_nested_object;
mod example_object;

use file_ext::FileExt;
use crate::json::array::{New};
use crate::json::array::tests::example_multi_nested_object::example_nested_object::ExampleNestedObject;
use crate::json::array::tests::example_multi_nested_object::example_object::ExampleObject;

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


    let actual = ExampleObject::to_json_list(list).unwrap();
    let expected = "[{\r\n  \"prop_a\": \"\",\r\n  \"prop_b\": false,\r\n  \"prop_c\": false,\r\n  \"prop_d\": 0,\r\n  \"prop_e\": 0.0\r\n},\r\n{\r\n  \"prop_a\": \"test\",\r\n  \"prop_b\": true,\r\n  \"prop_c\": false,\r\n  \"prop_d\": 10,\r\n  \"prop_e\": 2.2,\r\n  \"prop_f\": [{\r\n  \"prop_a\": \"test\",\r\n  \"prop_b\": false,\r\n  \"prop_d\": 1,\r\n  \"prop_e\": 2.2\r\n},\r\n{\r\n  \"prop_a\": \"test string\",\r\n  \"prop_b\": true,\r\n  \"prop_d\": 11,\r\n  \"prop_e\": 21.12\r\n}],\r\n  \"prop_g\": {\r\n  \"prop_a\": \"test\",\r\n  \"prop_b\": false,\r\n  \"prop_d\": 1,\r\n  \"prop_e\": 2.2\r\n}\r\n}]".to_string();


    assert_eq!(actual, expected);
}


#[test]
fn json_to_vector() {
    // 1. retrieve json string, in this example it is done via reading a file
    let path = FileExt::build_path(&["src", "json", "array", "tests", "example_multi_nested_object", "list.example_object.from.formatted.json"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();
    let json = String::from_utf8(file_as_bytes).unwrap();

    // 2. parse json to vector
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

    assert!(parsed_obj.prop_g.is_some());
    let nested_obj = parsed_obj.prop_g.as_ref().unwrap();
    assert_eq!(nested_obj.prop_a, "test");
    assert_eq!(nested_obj.prop_b, false);
    assert_eq!(nested_obj.prop_c, 1);
    assert_eq!(nested_obj.prop_d, 2.2);


    let nested_list = parsed_obj.prop_f.as_ref().unwrap();
    assert_eq!(2, nested_list.len());

    let nested_obj =  nested_list.get(0).unwrap();
    assert_eq!("test", nested_obj.prop_a);
    assert_eq!(false, nested_obj.prop_b);
    assert_eq!(1, nested_obj.prop_c);
    assert_eq!(2.2, nested_obj.prop_d);

    let nested_obj =  nested_list.get(1).unwrap();
    assert_eq!("test string", nested_obj.prop_a);
    assert_eq!(true, nested_obj.prop_b);
    assert_eq!(11, nested_obj.prop_c);
    assert_eq!(21.12, nested_obj.prop_d);
}