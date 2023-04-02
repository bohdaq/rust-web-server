use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_wrong_element_duplicate_point() {
    let array = "[ 6.2.2]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).err().unwrap();
    let expected = "unable to parse number: 6.2 in [ 6.2.2]";
    assert_eq!(actual, expected);
}