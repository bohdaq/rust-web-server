use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_wrong_element() {
    let array = "[ asdfg, 456, 6,7 ,8]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).err().unwrap();
    let expected = "unknown type: a in [ asdfg, 456, 6,7 ,8]";
    assert_eq!(actual, expected);
}