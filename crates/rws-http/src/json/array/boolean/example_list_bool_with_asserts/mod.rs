use crate::json::array::boolean::JSONArrayOfBooleans;

#[test]
fn json_to_vector() {
    let json_array = "[true, false]".to_string();

    let boxed_parse = JSONArrayOfBooleans::parse_as_list_bool(json_array);
    if boxed_parse.is_err() {
        // handle error
    }


    let list : Vec<bool> = boxed_parse.unwrap();

    let element : &bool =  list.get(0).unwrap();
    assert_eq!( element, &true);

    let element : &bool =  list.get(1).unwrap();
    assert_eq!( *element, false);


}

#[test]
fn vector_to_json() {
    let json_array: Vec<bool> = vec![true, false];

    let result = JSONArrayOfBooleans::to_json_from_list_bool(&json_array);
    if result.is_err() {
        // handle error
    }

    let json_array = result.unwrap();
    assert_eq!("[true,false]", json_array);
}
