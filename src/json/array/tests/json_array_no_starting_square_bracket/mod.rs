use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_no_starting_square_bracket() {
    let array = "  123, 456, 6,7 ,8  ]";
    let result = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string());
    assert!(result.is_err());

    let message = result.err().unwrap();
    assert_eq!("input string does not start with opening square bracket: 1 in   123, 456, 6,7 ,8  ]", message);
}