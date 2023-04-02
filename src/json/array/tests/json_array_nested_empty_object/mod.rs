use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_nested_empty_object() {
    let array = "[ {} ]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["{}"];
    assert_eq!(actual, expected);
}