use crate::json::array::JSONArray;
use crate::json::{JSON_TYPE, JSONValue};
use crate::json::object::{FromJSON, JSON, ToJSON};
use crate::json::property::JSONProperty;
use crate::symbol::SYMBOL;

#[test]
fn json_array_true_element() {
    let array = "[true]";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["true"];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_false_element() {
    let array = "[false]";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["false"];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_nested_array() {
    let array = "[ [false] ]";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["[false]"];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_nested_object() {
    let array = "[ {\"prop_b\": true, \"prop_a\": \"123abc\"} ]";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["{\"prop_b\": true, \"prop_a\": \"123abc\"}"];
    assert_eq!(actual, expected);

    struct SomeObject {
        prop_a: String,
        prop_b: bool
    }

    impl FromJSON for SomeObject {
        fn parse_json_to_properties(&self, json_string: String) -> Result<Vec<(JSONProperty, JSONValue)>, String> {
            let boxed_parse = JSON::parse_as_properties(json_string);
            if boxed_parse.is_err() {
                let message = boxed_parse.err().unwrap();
                return Err(message)
            }
            let properties = boxed_parse.unwrap();
            Ok(properties)
        }
        fn set_properties(&mut self, properties: Vec<(JSONProperty, JSONValue)>) -> Result<(), String> {
            for (property, value) in properties {
                if property.property_name == "prop_a" {
                    self.prop_a = value.string.unwrap();
                }
                if property.property_name == "prop_b" {
                    self.prop_b = value.bool.unwrap();
                }
            }
            Ok(())
        }
        fn parse(&mut self, json_string: String) -> Result<(), String> {
            let boxed_properties = self.parse_json_to_properties(json_string);
            if boxed_properties.is_err() {
                let message = boxed_properties.err().unwrap();
                return Err(message);
            }
            let properties = boxed_properties.unwrap();
            let boxed_set = self.set_properties(properties);
            if boxed_set.is_err() {
                let message = boxed_set.err().unwrap();
                return Err(message);
            }
            Ok(())
        }
    }

    impl ToJSON for SomeObject {
        fn list_properties() -> Vec<JSONProperty> {
            let mut list = vec![];

            let property = JSONProperty { property_name: "prop_a".to_string(), property_type: JSON_TYPE.string.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_b".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            list
        }

        fn get_property(&self, property_name: String) -> JSONValue {
            let mut value = JSONValue::new();

            if property_name == "prop_a".to_string() {
                let string : String = self.prop_a.to_owned();
                value.string = Some(string);
            }

            if property_name == "prop_b".to_string() {
                let boolean : bool = self.prop_b;
                value.bool = Some(boolean);
            }

            value
        }

        fn to_json_string(&self) -> String {
            let mut json_list = vec![];
            json_list.push(SYMBOL.opening_curly_bracket.to_string());


            let mut properties_list = vec![];

            let properties = SomeObject::list_properties();
            for property in properties {
                let value = self.get_property(property.property_name.to_string());

                if &property.property_type == "String" {
                    let raw_value = value.string.unwrap();
                    let formatted_property = format!("  \"{}\": \"{}\"", &property.property_name, raw_value);
                    properties_list.push(formatted_property.to_string());
                }

                if &property.property_type == "bool" {
                    let raw_value = value.bool.unwrap();
                    let formatted_property = format!("  \"{}\": {}", &property.property_name, raw_value);
                    properties_list.push(formatted_property.to_string());
                }
            }


            let comma_new_line_carriage_return = format!("{}{}", SYMBOL.comma, SYMBOL.new_line_carriage_return);
            let properties = properties_list.join(&comma_new_line_carriage_return);

            json_list.push(properties);
            json_list.push(SYMBOL.closing_curly_bracket.to_string());
            let json= json_list.join(SYMBOL.new_line_carriage_return);
            json
        }
    }

    let mut obj = SomeObject { prop_a: "default".to_string(), prop_b: false };

    let json = expected.get(0).unwrap();
    obj.parse(json.to_string()).unwrap();

    assert_eq!("123abc", obj.prop_a);
    assert_eq!(true, obj.prop_b);

    let expected_json = "{\r\n  \"prop_a\": \"123abc\",\r\n  \"prop_b\": true\r\n}";
    assert_eq!(obj.to_json_string(), expected_json);
}

#[test]
fn json_array_nested_empty_object() {
    let array = "[ {} ]";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["{}"];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_nested_object_nested_array() {
    let array = "[ {\"key\": [123, 456, 789, 10]} ]";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["{\"key\": [123, 456, 789, 10]}"];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_nested_array_multiple_items() {
    let array = "[ [true,0, null, -1, 2.0, \"text\", false] ]";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["[true,0, null, -1, 2.0, \"text\", false]"];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_multi_nested_array_multiple_items() {
    let array = "[ [true,0, [null, -1], 2.0, \"text\", false] ]";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["[true,0, [null, -1], 2.0, \"text\", false]"];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_multiple_true_elements() {
    let array = "[ true,true]";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["true", "true"];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_multiple_false_elements() {
    let array = "[false , false]";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["false", "false"];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_null_element() {
    let array = "[null]";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["null"];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_multiple_null_elements() {
    let array = "[null ,null]";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["null", "null"];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_multiple_elements() {
    let array = "[true,0, null, -1, 2.0, \"text\", false]";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["true", "0", "null", "-1", "2.0", "\"text\"", "false"];
    assert_eq!(actual, expected);
}

#[test]
fn json_array() {
    let array = "[123, 456, 6,7 ,8]";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["123", "456", "6", "7", "8"];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_float() {
    let array = "[123.123, 456.456, 6.534e123,7 ,8.0]";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["123.123", "456.456", "6.534e123", "7", "8.0"];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_strings() {
    let array = "[\"a\", \"b\", \"c\",\"d\" ,\"e\"]";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["\"a\"", "\"b\"", "\"c\"", "\"d\"", "\"e\""];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_strings_multichar() {
    let array = "[\"ab\", \"bb\", \"bc\",\"db\" ,\"eb\"]";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["\"ab\"", "\"bb\"", "\"bc\"", "\"db\"", "\"eb\""];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_wrong_element() {
    let array = "[ asdfg, 456, 6,7 ,8]";
    let actual = JSONArray::parse(array.to_string()).err().unwrap();
    let expected = "unknown type: a in [ asdfg, 456, 6,7 ,8]";
    assert_eq!(actual, expected);
}

#[test]
fn json_array_wrong_element_duplicate_minus() {
    let array = "[ --35346, 456, 6,7 ,8]";
    let actual = JSONArray::parse(array.to_string()).err().unwrap();
    let expected = "unable to parse number: - in [ --35346, 456, 6,7 ,8]";
    assert_eq!(actual, expected);
}

#[test]
fn json_array_wrong_element_duplicate_exponent() {
    let array = "[ 6e2e2]";
    let actual = JSONArray::parse(array.to_string()).err().unwrap();
    let expected = "unable to parse number: 6e2 in [ 6e2e2]";
    assert_eq!(actual, expected);
}

#[test]
fn json_array_wrong_element_duplicate_point() {
    let array = "[ 6.2.2]";
    let actual = JSONArray::parse(array.to_string()).err().unwrap();
    let expected = "unable to parse number: 6.2 in [ 6.2.2]";
    assert_eq!(actual, expected);
}

#[test]
fn json_array_wrong_element_rundom_char() {
    let array = "[ 6h2]";
    let actual = JSONArray::parse(array.to_string()).err().unwrap();
    let expected = "unable to parse number: h in [ 6h2]";
    assert_eq!(actual, expected);
}

#[test]
fn json_array_whitespace_before_first_element() {
    let array = "[ 123.76, -456, 0,7.5e4 ,8]";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["123.76", "-456", "0", "7.5e4", "8"];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_whitespace_after_last_element() {
    let array = "[ 123, 456, 6,7 ,8 ]";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["123", "456", "6", "7", "8"];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_whitespace_before_array() {
    let array = " [ 123, 456, 6,7 ,8 ]";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["123", "456", "6", "7", "8"];
    assert_eq!(actual, expected);
}


#[test]
fn json_array_whitespace_after_array() {
    let array = " [ 123, 456, 6,7 ,8 ] ";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected = vec!["123", "456", "6", "7", "8"];
    assert_eq!(actual, expected);
}

#[test]
fn json_empty_array() {
    let array = " [  ] ";
    let actual = JSONArray::parse(array.to_string()).unwrap();
    let expected : Vec<String> = vec![];
    assert_eq!(actual, expected);
}

#[test]
fn json_array_starts_with_random_chars() {
    let array = "adgsfdg [ 123, 456, 6,7 ,8 ] ";
    let actual = JSONArray::parse(array.to_string()).err().unwrap();
    let expected = "input string does not start with opening square bracket: a in adgsfdg [ 123, 456, 6,7 ,8 ] ";
    assert_eq!(actual, expected);
}

#[test]
fn json_array_ends_with_random_chars() {
    let array = " [ 123, 456, 6,7 ,8 ] adgsfdg";
    let actual = JSONArray::parse(array.to_string()).err().unwrap();
    let expected = "after array there are some characters: a in  [ 123, 456, 6,7 ,8 ] adgsfdg";
    assert_eq!(actual, expected);
}

#[test]
fn json_array_no_closing_square_bracket() {
    let array = " [ 123, 456, 6,7 ,8  ";
    let result = JSONArray::parse(array.to_string());
    assert!(result.is_err());

    let message = result.err().unwrap();
    assert_eq!("Missing comma between array items or closing square bracket at the end of array:  [ 123, 456, 6,7 ,8  ", message);
}

#[test]
fn json_array_no_starting_square_bracket() {
    let array = "  123, 456, 6,7 ,8  ]";
    let result = JSONArray::parse(array.to_string());
    assert!(result.is_err());

    let message = result.err().unwrap();
    assert_eq!("input string does not start with opening square bracket: 1 in   123, 456, 6,7 ,8  ]", message);
}

#[test]
fn json_array_whitespaces() {
    let array = "  ";
    let result = JSONArray::parse(array.to_string());
    assert!(result.is_err());

    let message = result.err().unwrap();
    assert_eq!("not proper start of the json array:   ", message);
}

#[test]
fn json_array_missing_comma() {
    let array = "[  123, 456 6,7 ,8  ]";
    let result = JSONArray::parse(array.to_string());
    assert!(result.is_err());

    let message = result.err().unwrap();
    assert_eq!("Missing comma between array items or closing square bracket at the end of array: [  123, 456 6,7 ,8  ]", message);
}
