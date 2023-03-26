use file_ext::FileExt;
use crate::json::object::FromJSON;
use crate::json::object::tests::deserialize_json_with_null_fields_to_struct::some_object::SomeObject;

mod some_object;

#[test]
fn parse_null() {

    let path = FileExt::build_path(&["src", "json", "object", "tests", "deserialize_json_with_null_fields_to_struct", "some-object.json"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let file_as_bytes = FileExt::read_file(absolute_file_path.as_str()).unwrap();
    let json_string_with_null = String::from_utf8(file_as_bytes).unwrap();

    let mut desirealized_object = SomeObject {
        prop_a: "default".to_string(),
        prop_b: true,
        prop_c: false,
        prop_d: 100,
        prop_e: 100.1,
    };
    desirealized_object.parse(json_string_with_null.to_string()).unwrap();

    assert_eq!("default", desirealized_object.prop_a);
    assert_eq!(true, desirealized_object.prop_b);
    assert_eq!(false, desirealized_object.prop_c);
    assert_eq!(100, desirealized_object.prop_d);
    assert_eq!(100.1, desirealized_object.prop_e);
}