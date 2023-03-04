use std::io;
use std::io::{BufRead, Read};
use crate::json::{ToJSON, JSONProperty, JSONValue, FromJSON, JSONType, JSON_TYPE, JSON};
use crate::json::key_value::parse_json_property;
use crate::symbol::SYMBOL;

#[test]
fn parse() {
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
                    self.prop_a = value.String.unwrap();
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

            let property = JSONProperty { property_name: "prop_a".to_string(), property_type: "String".to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_b".to_string(), property_type: "bool".to_string() };
            list.push(property);

            list
        }

        fn get_property(&self, property_name: String) -> JSONValue {
            let mut value = JSONValue {
                f64: None,
                i128: None,
                String: None,
                bool: None,
                null: None,
            };

            if property_name == "prop_a".to_string() {
                let string : String = self.prop_a.to_owned();
                value.String = Some(string);
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
                    let raw_value = value.String.unwrap();
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

    let mut obj = SomeObject { prop_a: "123abc".to_string(), prop_b: true };

    let json_string = obj.to_json_string();
    let expected_json_string = "{\r\n  \"prop_a\": \"123abc\",\r\n  \"prop_b\": true\r\n}";

    assert_eq!(expected_json_string, json_string);

    let properties  = obj.parse_json_to_properties(json_string.to_string()).unwrap();
    assert_eq!(properties.len(), 2);

    let (prop_a_type, prop_a_value) = properties.get(0).unwrap();
    assert_eq!(prop_a_type.property_type, JSON_TYPE.string);
    assert_eq!(prop_a_type.property_name, "prop_a");
    assert_eq!(prop_a_value.String.clone().unwrap(), "123abc");


    let (prop_b_type, prop_b_value) = properties.get(1).unwrap();
    assert_eq!(prop_b_type.property_type, JSON_TYPE.boolean);
    assert_eq!(prop_b_type.property_name, "prop_b");
    assert_eq!(prop_b_value.bool.unwrap(), true);

    obj.set_properties(properties).unwrap();
    assert_eq!("123abc", obj.prop_a);
    assert_eq!(true, obj.prop_b);
}


#[test]
fn parse_direct() {
    struct SomeObject {
        prop_a: String,
        prop_b: bool,
        prop_c: bool,
        prop_d: i128
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
                    self.prop_a = value.String.unwrap();
                }
                if property.property_name == "prop_b" {
                    self.prop_b = value.bool.unwrap();
                }

                if property.property_name == "prop_c" {
                    self.prop_c = value.bool.unwrap();
                }

                if property.property_name == "prop_d" {
                    self.prop_d = value.i128.unwrap();
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

            let property = JSONProperty { property_name: "prop_a".to_string(), property_type: "String".to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_b".to_string(), property_type: "bool".to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_c".to_string(), property_type: "bool".to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_d".to_string(), property_type: "i128".to_string() };
            list.push(property);

            list
        }

        fn get_property(&self, property_name: String) -> JSONValue {
            let mut value = JSONValue {
                f64: None,
                i128: None,
                String: None,
                bool: None,
                null: None,
            };

            if property_name == "prop_a".to_string() {
                let string : String = self.prop_a.to_owned();
                value.String = Some(string);
            }

            if property_name == "prop_b".to_string() {
                let boolean : bool = self.prop_b;
                value.bool = Some(boolean);
            }

            if property_name == "prop_c".to_string() {
                let boolean : bool = self.prop_c;
                value.bool = Some(boolean);
            }

            if property_name == "prop_d".to_string() {
                let integer : i128 = self.prop_d;
                value.i128 = Some(integer);
            }

            value
        }

        fn to_json_string(&self) -> String {
            let mut processed_data = vec![];

            let properties = SomeObject::list_properties();
            for property in properties {
                let value = self.get_property(property.property_name.to_string());
                processed_data.push((property, value));

            }

            JSON::to_json_string(processed_data)
        }
    }

    let mut obj = SomeObject {
        prop_a: "123abc".to_string(),
        prop_b: true,
        prop_c: false,
        prop_d: 4356257,
    };

    let json_string = obj.to_json_string();
    let expected_json_string = "{\r\n  \"prop_a\": \"123abc\",\r\n  \"prop_b\": true,\r\n  \"prop_c\": false,\r\n  \"prop_d\": 4356257\r\n}";

    assert_eq!(expected_json_string, json_string);

    let mut deserealized_object = SomeObject {
        prop_a: "".to_string(),
        prop_b: false,
        prop_c: true,
        prop_d: 0
    };
    deserealized_object.parse(json_string.to_string()).unwrap();

    assert_eq!("123abc", deserealized_object.prop_a);
    assert_eq!(true, deserealized_object.prop_b);
    assert_eq!(false, deserealized_object.prop_c);
    assert_eq!(4356257, deserealized_object.prop_d);
}


#[test]
fn parse_null() {
    struct SomeObject {
        prop_a: String,
        prop_b: bool,
        prop_c: bool,
        prop_d: i128
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
                    if value.null.is_none() {
                        self.prop_a = value.String.unwrap();
                    }
                }
                if property.property_name == "prop_b" {
                    if value.null.is_none() {
                        self.prop_b = value.bool.unwrap();
                    }
                }

                if property.property_name == "prop_c" {
                    if value.null.is_none() {
                        self.prop_c = value.bool.unwrap();
                    }
                }

                if property.property_name == "prop_d" {
                    if value.null.is_none() {
                        self.prop_d = value.i128.unwrap();
                    }
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

            let property = JSONProperty { property_name: "prop_a".to_string(), property_type: "String".to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_b".to_string(), property_type: "bool".to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_c".to_string(), property_type: "bool".to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_d".to_string(), property_type: "i128".to_string() };
            list.push(property);

            list
        }

        fn get_property(&self, property_name: String) -> JSONValue {
            let mut value = JSONValue {
                f64: None,
                i128: None,
                String: None,
                bool: None,
                null: None,
            };

            if property_name == "prop_a".to_string() {
                let string : String = self.prop_a.to_owned();
                value.String = Some(string);
            }

            if property_name == "prop_b".to_string() {
                let boolean : bool = self.prop_b;
                value.bool = Some(boolean);
            }

            if property_name == "prop_c".to_string() {
                let boolean : bool = self.prop_c;
                value.bool = Some(boolean);
            }

            if property_name == "prop_d".to_string() {
                let integer : i128 = self.prop_d;
                value.i128 = Some(integer);
            }

            value
        }

        fn to_json_string(&self) -> String {
            let mut processed_data = vec![];

            let properties = SomeObject::list_properties();
            for property in properties {
                let value = self.get_property(property.property_name.to_string());
                processed_data.push((property, value));

            }

            JSON::to_json_string(processed_data)
        }
    }

    let json_string_with_null = "{\r\n  \"prop_a\": null,\r\n  \"prop_b\": null,\r\n  \"prop_c\": null,\r\n  \"prop_d\": null\r\n}";


    let mut deserealized_object = SomeObject {
        prop_a: "default".to_string(),
        prop_b: true,
        prop_c: false,
        prop_d: 100,
    };
    deserealized_object.parse(json_string_with_null.to_string()).unwrap();

    assert_eq!("default", deserealized_object.prop_a);
    assert_eq!(true, deserealized_object.prop_b);
    assert_eq!(false, deserealized_object.prop_c);
    assert_eq!(100, deserealized_object.prop_d);
}



