use crate::json::array::float::JSONArrayOfFloats;

#[test]
fn json_to_vector() {
    let json_array = "[-2.2, 0.0 , 5.5]".to_string();

    let boxed_parse = JSONArrayOfFloats::parse_as_list_f32(json_array);
    if boxed_parse.is_err() {
        // handle error
    }

    let list : Vec<f32> = boxed_parse.unwrap();

    let element : f32 =  *list.get(0).unwrap();
    assert_eq!( element, -2.2);

    let element : f32 =  *list.get(1).unwrap();
    assert_eq!( element, 0.0);

    let element : f32 =  *list.get(2).unwrap();
    assert_eq!( element, 5.5);
}

#[test]
fn vector_to_json() {
    let json_array: Vec<f32> = vec![-2.2, 0.0, 5.5];

    let result = JSONArrayOfFloats::to_json_from_list_f32(&json_array);
    if result.is_err() {
        // handle error
    }

    let json_array = result.unwrap();
    assert_eq!("[-2.2,0.0,5.5]", json_array);
}
