use crate::json::array::string::JSONArrayOfStrings;

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
    let json_array: Vec<String> = vec!["one".to_string(), "two".to_string()];

    let result = JSONArrayOfStrings::to_json_from_list_string(&json_array);
    if result.is_err() {
        // handle error
    }

    let json_array = result.unwrap();
    assert_eq!("[\"one\",\"two\"]", json_array);
}
