use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_multiple_true_elements() {
    let array = "[ true,true]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["true", "true"];
    assert_eq!(actual, expected);
}