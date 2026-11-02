use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_nested_object_nested_array() {
    let array = "[ {\"key\": [123, 456, 789, 10]} ]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["{\"key\": [123, 456, 789, 10]}"];
    assert_eq!(actual, expected);
}