use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_multiple_elements() {
    let array = "[true,0, null, -1, 2.0, \"text\", false]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["true", "0", "null", "-1", "2.0", "\"text\"", "false"];
    assert_eq!(actual, expected);
}