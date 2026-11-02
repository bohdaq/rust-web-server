use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_starts_with_random_chars() {
    let array = "adgsfdg [ 123, 456, 6,7 ,8 ] ";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).err().unwrap();
    let expected = "input string does not start with opening square bracket: a in adgsfdg [ 123, 456, 6,7 ,8 ] ";
    assert_eq!(actual, expected);
}