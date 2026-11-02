use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_nested_array() {
    let array = "[ [false] ]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["[false]"];
    assert_eq!(actual, expected);
}