use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_wrong_element_duplicate_exponent() {
    let array = "[ 6e2e2]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).err().unwrap();
    let expected = "unable to parse number: 6e2 in [ 6e2e2]";
    assert_eq!(actual, expected);
}