mod some_object;

use crate::json::array::RawUnprocessedJSONArray;
use crate::json::array::tests::json_array_nested_object::some_object::SomeObject;
use crate::json::object::{FromJSON, ToJSON};

#[test]
fn json_array_nested_object() {
    let array = "[ {\"prop_b\": true, \"prop_a\": \"123abc\"} ]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["{\"prop_b\": true, \"prop_a\": \"123abc\"}"];
    assert_eq!(actual, expected);



    let mut obj = SomeObject { prop_a: "default".to_string(), prop_b: false };

    let json = expected.get(0).unwrap();
    obj.parse(json.to_string()).unwrap();

    assert_eq!("123abc", obj.prop_a);
    assert_eq!(true, obj.prop_b);

    let expected_json = "{\r\n  \"prop_a\": \"123abc\",\r\n  \"prop_b\": true\r\n}";
    assert_eq!(obj.to_json_string(), expected_json);
}