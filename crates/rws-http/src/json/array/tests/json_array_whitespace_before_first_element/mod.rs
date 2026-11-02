use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_whitespace_before_first_element() {
    let array = "[ 123.76, -456, 0,7.5e4 ,8]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["123.76", "-456", "0", "7.5e4", "8"];
    assert_eq!(actual, expected);
}