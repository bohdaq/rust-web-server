use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_multiple_null_elements() {
    let array = "[null ,null]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["null", "null"];
    assert_eq!(actual, expected);
}