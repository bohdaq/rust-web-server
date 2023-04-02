use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_null_element() {
    let array = "[null]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["null"];
    assert_eq!(actual, expected);
}