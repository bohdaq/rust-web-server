use crate::json::array::RawUnprocessedJSONArray;

#[test]
fn json_array_strings_multichar() {
    let array = "[\"ab\", \"bb\", \"bc\",\"db\" ,\"eb\"]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["\"ab\"", "\"bb\"", "\"bc\"", "\"db\"", "\"eb\""];
    assert_eq!(actual, expected);
}