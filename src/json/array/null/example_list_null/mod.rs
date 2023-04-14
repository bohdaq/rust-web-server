use crate::json::array::null::JSONArrayOfNulls;
use crate::null::{Null, NULL};

#[test]
fn json_to_vector() {
    let json_array = "[null, null]".to_string();

    let boxed_parse = JSONArrayOfNulls::parse_as_list_null(json_array);
    if boxed_parse.is_err() {
        // handle error
    }

    let _list : Vec<Null> = boxed_parse.unwrap();


}

#[test]
fn vector_to_json() {
    let json_array: Vec<&Null> = vec![NULL, NULL];

    let result = JSONArrayOfNulls::to_json_from_list_null(&json_array);
    if result.is_err() {
        // handle error
    }

    let _json_array = result.unwrap();

}
