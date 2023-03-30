use file_ext::FileExt;
use crate::json::array::New;
use crate::json::object::tests::example::some_object::SomeObject;
use crate::json::object::{FromJSON, ToJSON};

mod some_object;

#[test]
fn parse_json() {
    // 1. retrieve json string, in this example it is done via reading a file
    let path = FileExt::build_path(&["src", "json", "object", "tests", "example", "some-object.json"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();
    let json = String::from_utf8(file_as_bytes).unwrap();


    // 2. create instance of struct
    let mut some_object = SomeObject::new();
    // 3. parse json
    let parse_result = some_object.parse(json);
    if parse_result.is_err() {
        // 4. error handler in case of malformed input json
    }

}

#[test]
fn to_json() {
    // 1. initiate struct
    let some_object = SomeObject{ prop_a: "example".to_string(), prop_b: false };
    // 2. call to_json_string
    some_object.to_json_string();
}