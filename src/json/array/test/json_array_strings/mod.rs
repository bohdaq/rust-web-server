use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_strings() {
    let array = "[\"a\", \"b\", \"c\",\"d\" ,\"e\"]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["\"a\"", "\"b\"", "\"c\"", "\"d\"", "\"e\""];
    assert_eq!(actual, expected);
}