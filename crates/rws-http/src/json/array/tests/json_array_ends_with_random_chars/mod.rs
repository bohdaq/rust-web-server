use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_ends_with_random_chars() {
    let array = " [ 123, 456, 6,7 ,8 ] adgsfdg";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).err().unwrap();
    let expected = "after array there are some characters: a in  [ 123, 456, 6,7 ,8 ] adgsfdg";
    assert_eq!(actual, expected);
}