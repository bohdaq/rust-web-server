use crate::json::array::integer::JSONArrayOfIntegers;

#[test]
fn json_to_vector() {
    let json_array = "[2, 0 , 5]".to_string();

    let boxed_parse = JSONArrayOfIntegers::parse_as_list_u32(json_array);
    if boxed_parse.is_err() {
        // handle error
    }

    let list : Vec<u32> = boxed_parse.unwrap();

    let element : u32 =  *list.get(0).unwrap();
    assert_eq!( element, 2);

    let element : u32 =  *list.get(1).unwrap();
    assert_eq!( element, 0);

    let element : u32 =  *list.get(2).unwrap();
    assert_eq!( element, 5);
}

#[test]
fn vector_to_json() {
    let json_array: Vec<u32> = vec![2, 0, 5];

    let result = JSONArrayOfIntegers::to_json_from_list_u32(&json_array);
    if result.is_err() {
        // handle error
    }

    let json_array = result.unwrap();
    assert_eq!("[2,0,5]", json_array);
}
