use crate::json::array::boolean::JSONArrayOfBooleans;

#[test]
fn json_to_vector() {
    let json_array = "[true, false]".to_string();

    let boxed_parse = JSONArrayOfBooleans::parse_as_list_bool(json_array);
    if boxed_parse.is_err() {
        // handle error
    }


    let _list : Vec<bool> = boxed_parse.unwrap();

}

#[test]
fn vector_to_json() {
    let json_array: Vec<bool> = vec![true, false];

    let result = JSONArrayOfBooleans::to_json_from_list_bool(&json_array);
    if result.is_err() {
        // handle error
    }

    let _json_array = result.unwrap();
}
