use crate::json::array::{JSONArrayOfObjects, New};
use crate::json::example_object::ExampleObject;

#[test]
fn vector_to_json() {
    let obj = ExampleObject::new();
    let obj2 = ExampleObject {
        prop_a: "test".to_string(),
        prop_b: true,
        prop_c: false,
        prop_d: 10,
        prop_e: 2.2,
    };

    let actual = JSONArrayOfObjects::<ExampleObject>::to_json(vec![obj, obj2]).unwrap();
    let expected = "[{\r\n  \"prop_a\": \"\",\r\n  \"prop_b\": false,\r\n  \"prop_c\": false,\r\n  \"prop_d\": 0,\r\n  \"prop_e\": 0\r\n},\r\n{\r\n  \"prop_a\": \"test\",\r\n  \"prop_b\": true,\r\n  \"prop_c\": false,\r\n  \"prop_d\": 10,\r\n  \"prop_e\": 2.2\r\n}]".to_string();
    assert_eq!(actual, expected);
}

#[test]
fn json_to_vector() {
    let json = "[{\r\n  \"prop_a\": \"\",\r\n  \"prop_b\": false,\r\n  \"prop_c\": false,\r\n  \"prop_d\": 0,\r\n  \"prop_e\": 0\r\n},\r\n{\r\n  \"prop_a\": \"test\",\r\n  \"prop_b\": true,\r\n  \"prop_c\": false,\r\n  \"prop_d\": 10,\r\n  \"prop_e\": 2.2\r\n}]".to_string();

    let parsed_list : Vec<ExampleObject> = JSONArrayOfObjects::<ExampleObject>::from_json(json).unwrap();
    assert_eq!(2, parsed_list.len());

    let parsed_obj = parsed_list.get(0).unwrap();
    assert_eq!(parsed_obj.prop_a, "");
    assert_eq!(parsed_obj.prop_b, false);
    assert_eq!(parsed_obj.prop_c, false);
    assert_eq!(parsed_obj.prop_d, 0);
    assert_eq!(parsed_obj.prop_e, 0.0);

    let parsed_obj = parsed_list.get(1).unwrap();
    assert_eq!(parsed_obj.prop_a, "test");
    assert_eq!(parsed_obj.prop_b, true);
    assert_eq!(parsed_obj.prop_c, false);
    assert_eq!(parsed_obj.prop_d, 10);
    assert_eq!(parsed_obj.prop_e, 2.2);
}