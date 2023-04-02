use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array() {
    let array = "[123, 456, 6,7 ,8]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["123", "456", "6", "7", "8"];
    assert_eq!(actual, expected);
}