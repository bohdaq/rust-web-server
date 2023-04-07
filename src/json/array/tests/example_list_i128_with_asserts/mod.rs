use crate::json::array::JSONArrayOfIntegers;

#[test]
fn json_to_vector() {
    let json_array = "[-2, 0 , 5]".to_string();

    let boxed_parse = JSONArrayOfIntegers::parse_as_list_i128(json_array);
    if boxed_parse.is_err() {
        // handle error
    }

    let list : Vec<i128> = boxed_parse.unwrap();

    let element : i128 =  *list.get(0).unwrap();
    assert_eq!( element, -2);

    let element : i128 =  *list.get(1).unwrap();
    assert_eq!( element, 0);

    let element : i128 =  *list.get(2).unwrap();
    assert_eq!( element, 5);
}
