use crate::json::array::{RawUnprocessedJSONArray};







#[test]
fn json_array_strings() {
    let array = "[\"a\", \"b\", \"c\",\"d\" ,\"e\"]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["\"a\"", "\"b\"", "\"c\"", "\"d\"", "\"e\""];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_strings_multichar() {
    let array = "[\"ab\", \"bb\", \"bc\",\"db\" ,\"eb\"]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["\"ab\"", "\"bb\"", "\"bc\"", "\"db\"", "\"eb\""];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_wrong_element() {
    let array = "[ asdfg, 456, 6,7 ,8]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).err().unwrap();
    let expected = "unknown type: a in [ asdfg, 456, 6,7 ,8]";
    assert_eq!(actual, expected);
}

#[test]
fn json_array_wrong_element_duplicate_minus() {
    let array = "[ --35346, 456, 6,7 ,8]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).err().unwrap();
    let expected = "unable to parse number: - in [ --35346, 456, 6,7 ,8]";
    assert_eq!(actual, expected);
}

#[test]
fn json_array_wrong_element_duplicate_exponent() {
    let array = "[ 6e2e2]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).err().unwrap();
    let expected = "unable to parse number: 6e2 in [ 6e2e2]";
    assert_eq!(actual, expected);
}

#[test]
fn json_array_wrong_element_duplicate_point() {
    let array = "[ 6.2.2]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).err().unwrap();
    let expected = "unable to parse number: 6.2 in [ 6.2.2]";
    assert_eq!(actual, expected);
}

#[test]
fn json_array_wrong_element_rundom_char() {
    let array = "[ 6h2]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).err().unwrap();
    let expected = "unable to parse number: h in [ 6h2]";
    assert_eq!(actual, expected);
}

#[test]
fn json_array_whitespace_before_first_element() {
    let array = "[ 123.76, -456, 0,7.5e4 ,8]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["123.76", "-456", "0", "7.5e4", "8"];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_whitespace_after_last_element() {
    let array = "[ 123, 456, 6,7 ,8 ]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["123", "456", "6", "7", "8"];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_whitespace_before_array() {
    let array = " [ 123, 456, 6,7 ,8 ]";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["123", "456", "6", "7", "8"];
    assert_eq!(actual, expected);
}


#[test]
fn json_array_whitespace_after_array() {
    let array = " [ 123, 456, 6,7 ,8 ] ";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected = vec!["123", "456", "6", "7", "8"];
    assert_eq!(actual, expected);
}

#[test]
fn json_empty_array() {
    let array = " [  ] ";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).unwrap();
    let expected : Vec<String> = vec![];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_starts_with_random_chars() {
    let array = "adgsfdg [ 123, 456, 6,7 ,8 ] ";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).err().unwrap();
    let expected = "input string does not start with opening square bracket: a in adgsfdg [ 123, 456, 6,7 ,8 ] ";
    assert_eq!(actual, expected);
}

#[test]
fn json_array_ends_with_random_chars() {
    let array = " [ 123, 456, 6,7 ,8 ] adgsfdg";
    let actual = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string()).err().unwrap();
    let expected = "after array there are some characters: a in  [ 123, 456, 6,7 ,8 ] adgsfdg";
    assert_eq!(actual, expected);
}

#[test]
fn json_array_no_closing_square_bracket() {
    let array = " [ 123, 456, 6,7 ,8  ";
    let result = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string());
    assert!(result.is_err());

    let message = result.err().unwrap();
    assert_eq!("Missing comma between array items or closing square bracket at the end of array:  [ 123, 456, 6,7 ,8  ", message);
}

#[test]
fn json_array_no_starting_square_bracket() {
    let array = "  123, 456, 6,7 ,8  ]";
    let result = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string());
    assert!(result.is_err());

    let message = result.err().unwrap();
    assert_eq!("input string does not start with opening square bracket: 1 in   123, 456, 6,7 ,8  ]", message);
}

#[test]
fn json_array_whitespaces() {
    let array = "  ";
    let result = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string());
    assert!(result.is_err());

    let message = result.err().unwrap();
    assert_eq!("not proper start of the json array:   ", message);
}

#[test]
fn json_array_missing_comma() {
    let array = "[  123, 456 6,7 ,8  ]";
    let result = RawUnprocessedJSONArray::split_into_vector_of_strings(array.to_string());
    assert!(result.is_err());

    let message = result.err().unwrap();
    assert_eq!("Missing comma between array items or closing square bracket at the end of array: [  123, 456 6,7 ,8  ]", message);
}
