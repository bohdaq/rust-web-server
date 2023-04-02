use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_float() {
    let array = "[123.123, 456.456, 6.534e123,7 ,8.0]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["123.123", "456.456", "6.534e123", "7", "8.0"];
    assert_eq!(actual, expected);
}