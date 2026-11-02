use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_true_element() {
    let array = "[true]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["true"];
    assert_eq!(actual, expected);
}