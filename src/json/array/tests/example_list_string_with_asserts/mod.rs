use crate::json::array::{JSONArrayOfNulls, JSONArrayOfStrings};
use crate::null::{Null, NULL};

#[test]
fn json_to_vector() {
    let json_array = "[\"qwerty\", \"asdf\"]".to_string();

    let boxed_parse = JSONArrayOfStrings::parse_as_list_string(json_array);
    if boxed_parse.is_err() {
        // handle error
    }


    let list : Vec<String> = boxed_parse.unwrap();

    let element : &String =  list.get(0).unwrap();
    assert_eq!( element, "qwerty");

    let element : &String =  list.get(1).unwrap();
    assert_eq!( element, "asdf");


}

#[test]
fn vector_to_json() {
    let json_array: Vec<&Null> = vec![NULL, NULL];

    let result = JSONArrayOfNulls::to_json_from_list_null(&json_array);
    if result.is_err() {
        // handle error
    }

    let json_array = result.unwrap();
    assert_eq!("[null,null]", json_array);
}
