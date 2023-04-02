use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_multiple_false_elements() {
    let array = "[false , false]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["false", "false"];
    assert_eq!(actual, expected);
}