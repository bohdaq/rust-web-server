use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_empty_array() {
    let array = " [  ] ";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected : Vec<String> = vec![];
    assert_eq!(actual, expected);
}