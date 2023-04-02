use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_missing_comma() {
    let array = "[  123, 456 6,7 ,8  ]";
    let result = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string());
    assert!(result.is_err());

    let message = result.err().unwrap();
    assert_eq!("Missing comma between array items or closing square bracket at the end of array: [  123, 456 6,7 ,8  ]", message);
}