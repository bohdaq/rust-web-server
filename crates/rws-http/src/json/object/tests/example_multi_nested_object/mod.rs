use file_ext::FileExt;
use crate::json::object::tests::example_multi_nested_object::another_nested_object::AnotherNestedObject;
use crate::json::object::tests::example_multi_nested_object::nested_object::NestedObject;
use crate::json::object::tests::example_multi_nested_object::some_object::SomeObject;
use crate::json::object::{ToJSON};

pub mod another_nested_object;
pub mod nested_object;
pub mod some_object;


#[test]
fn parse_json() {
    // retrieve json string, in this example it is done via reading a file
    let path = FileExt::build_path(&["src", "json", "object", "tests", "example_multi_nested_object", "some-object.to.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();
    let json = String::from_utf8(file_as_bytes).unwrap();

    // parse json
    let parse_result = SomeObject::parse_json(json.as_str());
    if parse_result.is_err() {
        // error handler in case of malformed input json
    }
    // now _some_object represents json
    let _some_object : SomeObject = parse_result.unwrap();
}

#[test]
fn to_json() {
    // data modeling
    // multi nested object starts from inner most object
    let another_nested_obj = AnotherNestedObject {
        prop_bar: 2.2
    };

    // in this example root obj has nested object which itself has nested object
    let nested_obj = NestedObject
    {
        prop_foo: true,
        prop_baz: Some(another_nested_obj)
    };

    // root object
    let obj = SomeObject {
        prop_a: "123abc".to_string(),
        prop_b: true,
        prop_c: false,
        prop_d: 4356257,
        prop_e: 4356.257,
        prop_f: Some(nested_obj),
    };

    // 2. after construction, simply call `to_json_string`
    let _json_string : String = obj.to_json_string();
}

