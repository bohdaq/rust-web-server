use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_whitespaces() {
    let array = "  ";
    let result = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string());
    assert!(result.is_err());

    let message = result.err().unwrap();
    assert_eq!("not proper start of the json array:   ", message);
}