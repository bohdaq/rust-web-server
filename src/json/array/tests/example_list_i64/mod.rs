use crate::json::array::integer::JSONArrayOfIntegers;

#[test]
fn json_to_vector() {
    let json_array = "[-2, 0 , 5]".to_string();

    let boxed_parse = JSONArrayOfIntegers::parse_as_list_i64(json_array);
    if boxed_parse.is_err() {
        // handle error
    }

    let _list : Vec<i64> = boxed_parse.unwrap();

}

#[test]
fn vector_to_json() {
    let json_array: Vec<i64> = vec![-2, 0, 5];

    let result = JSONArrayOfIntegers::to_json_from_list_i64(&json_array);
    if result.is_err() {
        // handle error
    }

    let _json_array = result.unwrap();
}
