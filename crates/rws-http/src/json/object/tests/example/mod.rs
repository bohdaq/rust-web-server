use file_ext::FileExt;
use crate::json::object::tests::example::some_object::SomeObject;
use crate::json::object::{ToJSON};

mod some_object;

#[test]
fn parse_json() {
    // retrieve json string, in this example it is done via reading a file
    let path = FileExt::build_path(&["src", "json", "object", "tests", "example", "some-object.json"]);
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
    // initiate struct
    let some_object = SomeObject{ prop_a: "example".to_string(), prop_b: false };
    // call to_json_string
    let _json = some_object.to_json_string();
}