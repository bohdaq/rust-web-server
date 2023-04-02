use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_wrong_element_random_char() {
    let array = "[ 6h2]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).err().unwrap();
    let expected = "unable to parse number: h in [ 6h2]";
    assert_eq!(actual, expected);
}