use crate::json::array::{RawUnprocessedJSONArray};


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
