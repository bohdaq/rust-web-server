use crate::json::array::JSONArrayOfIntegers;

#[test]
fn json_to_vector() {
    let json_array = "[-2, 0 , 5]".to_string();

    let boxed_parse = JSONArrayOfIntegers::parse_as_list_i128(json_array);
    if boxed_parse.is_err() {
        // handle error
    }

    let _list : Vec<i128> = boxed_parse.unwrap();
}

#[test]
fn vector_to_json() {
    let _json_array : Vec<i128> = vec![-2, 0, 5];

    //TODO
}
