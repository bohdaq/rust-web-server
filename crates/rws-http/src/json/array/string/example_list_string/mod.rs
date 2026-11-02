use crate::json::array::string::JSONArrayOfStrings;

#[test]
fn json_to_vector() {
    let json_array = "[\"qwerty\", \"asdf\"]".to_string();

    let boxed_parse = JSONArrayOfStrings::parse_as_list_string(json_array);
    if boxed_parse.is_err() {
        // handle error
    }


    let _list : Vec<String> = boxed_parse.unwrap();

}

#[test]
fn vector_to_json() {
    let json_array: Vec<String> = vec!["one".to_string(), "two".to_string()];

    let result = JSONArrayOfStrings::to_json_from_list_string(&json_array);
    if result.is_err() {
        // handle error
    }

    let _json_array = result.unwrap();
}
