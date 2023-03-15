use crate::json::{JSONValue, JSON_TYPE};
use crate::json::property::JSONProperty;
use crate::json::object::{FromJSON, JSON, ToJSON};
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

    let mut obj = SomeObject { prop_a: "123abc".to_string(), prop_b: true };

    let json_string = obj.to_json_string();
    let expected_json_string = "{\r\n  \"prop_a\": \"123abc\",\r\n  \"prop_b\": true\r\n}";

    assert_eq!(expected_json_string, json_string);

    let properties  = obj.parse_json_to_properties(json_string.to_string()).unwrap();
    assert_eq!(properties.len(), 2);

    let (prop_a_type, prop_a_value) = properties.get(0).unwrap();
    assert_eq!(prop_a_type.property_type, JSON_TYPE.string);
    assert_eq!(prop_a_type.property_name, "prop_a");
    assert_eq!(prop_a_value.string.clone().unwrap(), "123abc");


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
        prop_d: i128,
        prop_e: f64
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

                if property.property_name == "prop_c" {
                    self.prop_c = value.bool.unwrap();
                }

                if property.property_name == "prop_d" {
                    self.prop_d = value.i128.unwrap();
                }

                if property.property_name == "prop_e" {
                    self.prop_e = value.f64.unwrap();
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

            let property = JSONProperty { property_name: "prop_c".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_d".to_string(), property_type: JSON_TYPE.integer.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_e".to_string(), property_type: JSON_TYPE.number.to_string() };
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

            if property_name == "prop_c".to_string() {
                let boolean : bool = self.prop_c;
                value.bool = Some(boolean);
            }

            if property_name == "prop_d".to_string() {
                let integer : i128 = self.prop_d;
                value.i128 = Some(integer);
            }

            if property_name == "prop_e".to_string() {
                let floating_point_number: f64 = self.prop_e;
                value.f64 = Some(floating_point_number);
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

    let obj = SomeObject {
        prop_a: "123abc".to_string(),
        prop_b: true,
        prop_c: false,
        prop_d: 4356257,
        prop_e: 4356.257,
    };

    let json_string = obj.to_json_string();
    let expected_json_string = "{\r\n  \"prop_a\": \"123abc\",\r\n  \"prop_b\": true,\r\n  \"prop_c\": false,\r\n  \"prop_d\": 4356257,\r\n  \"prop_e\": 4356.257\r\n}";

    assert_eq!(expected_json_string, json_string);

    let mut deserealized_object = SomeObject {
        prop_a: "".to_string(),
        prop_b: false,
        prop_c: true,
        prop_d: 0,
        prop_e: 0.0,
    };
    deserealized_object.parse(json_string.to_string()).unwrap();

    assert_eq!("123abc", deserealized_object.prop_a);
    assert_eq!(true, deserealized_object.prop_b);
    assert_eq!(false, deserealized_object.prop_c);
    assert_eq!(4356257, deserealized_object.prop_d);
    assert_eq!(4356.257, deserealized_object.prop_e);
}


#[test]
fn parse_null() {
    struct SomeObject {
        prop_a: String,
        prop_b: bool,
        prop_c: bool,
        prop_d: i128,
        prop_e: f64
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
                        self.prop_a = value.string.unwrap();
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

                if property.property_name == "prop_e" {
                    if value.null.is_none() {
                        self.prop_e = value.f64.unwrap();
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

            let property = JSONProperty { property_name: "prop_a".to_string(), property_type: JSON_TYPE.string.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_b".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_c".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_d".to_string(), property_type: JSON_TYPE.integer.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_e".to_string(), property_type: JSON_TYPE.number.to_string() };
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

            if property_name == "prop_c".to_string() {
                let boolean : bool = self.prop_c;
                value.bool = Some(boolean);
            }

            if property_name == "prop_d".to_string() {
                let integer : i128 = self.prop_d;
                value.i128 = Some(integer);
            }

            if property_name == "prop_e".to_string() {
                let integer : f64 = self.prop_e;
                value.f64 = Some(integer);
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

    let json_string_with_null = "{\r\n  \"prop_a\": null,\r\n  \"prop_b\": null,\r\n  \"prop_c\": null,\r\n  \"prop_d\": null,\r\n  \"prop_e\": null\r\n}";


    let mut desirealized_object = SomeObject {
        prop_a: "default".to_string(),
        prop_b: true,
        prop_c: false,
        prop_d: 100,
        prop_e: 100.1,
    };
    desirealized_object.parse(json_string_with_null.to_string()).unwrap();

    assert_eq!("default", desirealized_object.prop_a);
    assert_eq!(true, desirealized_object.prop_b);
    assert_eq!(false, desirealized_object.prop_c);
    assert_eq!(100, desirealized_object.prop_d);
    assert_eq!(100.1, desirealized_object.prop_e);
}

#[test]
fn parse_nested_object() {
    pub struct NestedObject {
        prop_foo: bool
    }

    impl FromJSON for NestedObject {
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
                if property.property_name == "prop_foo" {
                    self.prop_foo = value.bool.unwrap();
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

    impl ToJSON for NestedObject {
        fn list_properties() -> Vec<JSONProperty> {
            let mut list = vec![];

            let property = JSONProperty { property_name: "prop_foo".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            list
        }

        fn get_property(&self, property_name: String) -> JSONValue {
            let mut value = JSONValue::new();

            if property_name == "prop_foo".to_string() {
                let boolean : bool = self.prop_foo;
                value.bool = Some(boolean);
            }

            value
        }

        fn to_json_string(&self) -> String {
            let mut processed_data = vec![];

            let properties = NestedObject::list_properties();
            for property in properties {
                let value = self.get_property(property.property_name.to_string());
                processed_data.push((property, value));

            }

            JSON::to_json_string(processed_data)
        }
    }

    struct SomeObject {
        prop_a: String,
        prop_b: bool,
        prop_c: bool,
        prop_d: i128,
        prop_e: f64,
        prop_f: Option<NestedObject>
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

                if property.property_name == "prop_c" {
                    self.prop_c = value.bool.unwrap();
                }

                if property.property_name == "prop_d" {
                    self.prop_d = value.i128.unwrap();
                }

                if property.property_name == "prop_e" {
                    self.prop_e = value.f64.unwrap();
                }

                if property.property_name == "prop_f" {
                    let mut prop_f = NestedObject { prop_foo: false };
                    if value.object.is_some() {
                        let unparsed_object = value.object.unwrap();
                        let boxed_parse = prop_f.parse(unparsed_object);
                        if boxed_parse.is_err() {
                            let message = boxed_parse.err().unwrap();
                            return Err(message);
                        }
                        self.prop_f = Some(prop_f);
                    } else {
                        self.prop_f = None;
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

            let property = JSONProperty { property_name: "prop_a".to_string(), property_type: JSON_TYPE.string.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_b".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_c".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_d".to_string(), property_type: JSON_TYPE.integer.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_e".to_string(), property_type: JSON_TYPE.number.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_f".to_string(), property_type: JSON_TYPE.object.to_string() };
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

            if property_name == "prop_c".to_string() {
                let boolean : bool = self.prop_c;
                value.bool = Some(boolean);
            }

            if property_name == "prop_d".to_string() {
                let integer : i128 = self.prop_d;
                value.i128 = Some(integer);
            }

            if property_name == "prop_e".to_string() {
                let floating_point_number: f64 = self.prop_e;
                value.f64 = Some(floating_point_number);
            }

            if property_name == "prop_f".to_string() {
                let prop_f = self.prop_f.as_ref().unwrap();
                let serialized_nested_object = prop_f.to_json_string();
                value.object = Some(serialized_nested_object);
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

    let nested_obj = NestedObject { prop_foo: true };
    let obj = SomeObject {
        prop_a: "123abc".to_string(),
        prop_b: true,
        prop_c: false,
        prop_d: 4356257,
        prop_e: 4356.257,
        prop_f: Some(nested_obj),
    };

    let json_string = obj.to_json_string();
    let expected_json_string = "{\r\n  \"prop_a\": \"123abc\",\r\n  \"prop_b\": true,\r\n  \"prop_c\": false,\r\n  \"prop_d\": 4356257,\r\n  \"prop_e\": 4356.257,\r\n  \"prop_f\": {\r\n  \"prop_foo\": true\r\n}\r\n}";

    assert_eq!(expected_json_string, json_string);

    let mut deserealized_object = SomeObject {
        prop_a: "".to_string(),
        prop_b: false,
        prop_c: true,
        prop_d: 0,
        prop_e: 0.0,
        prop_f: None,
    };
    deserealized_object.parse(json_string.to_string()).unwrap();

    assert_eq!("123abc", deserealized_object.prop_a);
    assert_eq!(true, deserealized_object.prop_b);
    assert_eq!(false, deserealized_object.prop_c);
    assert_eq!(4356257, deserealized_object.prop_d);
    assert_eq!(4356.257, deserealized_object.prop_e);
    assert_eq!(true, deserealized_object.prop_f.unwrap().prop_foo);
}


#[test]
fn parse_nested_object_none() {
    pub struct NestedObject {
        prop_foo: bool
    }

    impl FromJSON for NestedObject {
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
                if property.property_name == "prop_foo" {
                    self.prop_foo = value.bool.unwrap();
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

    impl ToJSON for NestedObject {
        fn list_properties() -> Vec<JSONProperty> {
            let mut list = vec![];

            let property = JSONProperty { property_name: "prop_foo".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            list
        }

        fn get_property(&self, property_name: String) -> JSONValue {
            let mut value = JSONValue::new();

            if property_name == "prop_foo".to_string() {
                let boolean : bool = self.prop_foo;
                value.bool = Some(boolean);
            }

            value
        }

        fn to_json_string(&self) -> String {
            let mut processed_data = vec![];

            let properties = NestedObject::list_properties();
            for property in properties {
                let value = self.get_property(property.property_name.to_string());
                processed_data.push((property, value));

            }

            JSON::to_json_string(processed_data)
        }
    }

    struct SomeObject {
        prop_a: String,
        prop_b: bool,
        prop_c: bool,
        prop_d: i128,
        prop_e: f64,
        prop_f: Option<NestedObject>
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

                if property.property_name == "prop_c" {
                    self.prop_c = value.bool.unwrap();
                }

                if property.property_name == "prop_d" {
                    self.prop_d = value.i128.unwrap();
                }

                if property.property_name == "prop_e" {
                    self.prop_e = value.f64.unwrap();
                }

                if property.property_name == "prop_f" {
                    let mut prop_f = NestedObject { prop_foo: false };
                    if value.object.is_some() {
                        let unparsed_object = value.object.unwrap();
                        let boxed_parse = prop_f.parse(unparsed_object);
                        if boxed_parse.is_err() {
                            let message = boxed_parse.err().unwrap();
                            return Err(message);
                        }
                        self.prop_f = Some(prop_f);
                    } else {
                        self.prop_f = None;
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

            let property = JSONProperty { property_name: "prop_a".to_string(), property_type: JSON_TYPE.string.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_b".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_c".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_d".to_string(), property_type: JSON_TYPE.integer.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_e".to_string(), property_type: JSON_TYPE.number.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_f".to_string(), property_type: JSON_TYPE.object.to_string() };
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

            if property_name == "prop_c".to_string() {
                let boolean : bool = self.prop_c;
                value.bool = Some(boolean);
            }

            if property_name == "prop_d".to_string() {
                let integer : i128 = self.prop_d;
                value.i128 = Some(integer);
            }

            if property_name == "prop_e".to_string() {
                let floating_point_number: f64 = self.prop_e;
                value.f64 = Some(floating_point_number);
            }

            if property_name == "prop_f".to_string() {
                if self.prop_f.is_some() {
                    let prop_f = self.prop_f.as_ref().unwrap();
                    let serialized_nested_object = prop_f.to_json_string();
                    value.object = Some(serialized_nested_object);
                }
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

    let obj = SomeObject {
        prop_a: "123abc".to_string(),
        prop_b: true,
        prop_c: false,
        prop_d: 4356257,
        prop_e: 4356.257,
        prop_f: None,
    };

    let json_string = obj.to_json_string();
    let expected_json_string = "{\r\n  \"prop_a\": \"123abc\",\r\n  \"prop_b\": true,\r\n  \"prop_c\": false,\r\n  \"prop_d\": 4356257,\r\n  \"prop_e\": 4356.257\r\n}";

    assert_eq!(expected_json_string, json_string);

    let mut deserealized_object = SomeObject {
        prop_a: "".to_string(),
        prop_b: false,
        prop_c: true,
        prop_d: 0,
        prop_e: 0.0,
        prop_f: None,
    };
    deserealized_object.parse(json_string.to_string()).unwrap();

    assert_eq!("123abc", deserealized_object.prop_a);
    assert_eq!(true, deserealized_object.prop_b);
    assert_eq!(false, deserealized_object.prop_c);
    assert_eq!(4356257, deserealized_object.prop_d);
    assert_eq!(4356.257, deserealized_object.prop_e);
    assert!(deserealized_object.prop_f.is_none());
}


#[test]
fn parse_nested_object_property_null() {
    pub struct NestedObject {
        prop_foo: bool
    }

    impl FromJSON for NestedObject {
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
                if property.property_name == "prop_foo" {
                    if value.bool.is_some() {
                        self.prop_foo = value.bool.unwrap();
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

    impl ToJSON for NestedObject {
        fn list_properties() -> Vec<JSONProperty> {
            let mut list = vec![];

            let property = JSONProperty { property_name: "prop_foo".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            list
        }

        fn get_property(&self, property_name: String) -> JSONValue {
            let mut value = JSONValue::new();

            if property_name == "prop_foo".to_string() {
                let boolean : bool = self.prop_foo;
                value.bool = Some(boolean);
            }

            value
        }

        fn to_json_string(&self) -> String {
            let mut processed_data = vec![];

            let properties = NestedObject::list_properties();
            for property in properties {
                let value = self.get_property(property.property_name.to_string());
                processed_data.push((property, value));

            }

            JSON::to_json_string(processed_data)
        }
    }

    struct SomeObject {
        prop_a: String,
        prop_b: bool,
        prop_c: bool,
        prop_d: i128,
        prop_e: f64,
        prop_f: Option<NestedObject>
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

                if property.property_name == "prop_c" {
                    self.prop_c = value.bool.unwrap();
                }

                if property.property_name == "prop_d" {
                    self.prop_d = value.i128.unwrap();
                }

                if property.property_name == "prop_e" {
                    self.prop_e = value.f64.unwrap();
                }

                if property.property_name == "prop_f" {
                    let mut prop_f = NestedObject { prop_foo: false };
                    if value.object.is_some() {
                        let unparsed_object = value.object.unwrap();
                        let boxed_parse = prop_f.parse(unparsed_object);
                        if boxed_parse.is_err() {
                            let message = boxed_parse.err().unwrap();
                            return Err(message);
                        }
                        self.prop_f = Some(prop_f);
                    } else {
                        self.prop_f = None;
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

            let property = JSONProperty { property_name: "prop_a".to_string(), property_type: JSON_TYPE.string.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_b".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_c".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_d".to_string(), property_type: JSON_TYPE.integer.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_e".to_string(), property_type: JSON_TYPE.number.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_f".to_string(), property_type: JSON_TYPE.object.to_string() };
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

            if property_name == "prop_c".to_string() {
                let boolean : bool = self.prop_c;
                value.bool = Some(boolean);
            }

            if property_name == "prop_d".to_string() {
                let integer : i128 = self.prop_d;
                value.i128 = Some(integer);
            }

            if property_name == "prop_e".to_string() {
                let floating_point_number: f64 = self.prop_e;
                value.f64 = Some(floating_point_number);
            }

            if property_name == "prop_f".to_string() {
                if self.prop_f.is_some() {
                    let prop_f = self.prop_f.as_ref().unwrap();
                    let serialized_nested_object = prop_f.to_json_string();
                    value.object = Some(serialized_nested_object);
                }
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




    let json_string = "{\r\n  \"prop_a\": \"123abc\",\r\n  \"prop_b\": true,\r\n  \"prop_c\": false,\r\n  \"prop_d\": 4356257,\r\n  \"prop_e\": 4356.257,\r\n  \"prop_f\": {\r\n  \"prop_foo\": null\r\n}\r\n}";


    let mut deserealized_object = SomeObject {
        prop_a: "".to_string(),
        prop_b: false,
        prop_c: true,
        prop_d: 0,
        prop_e: 0.0,
        prop_f: None,
    };
    deserealized_object.parse(json_string.to_string()).unwrap();

    assert_eq!("123abc", deserealized_object.prop_a);
    assert_eq!(true, deserealized_object.prop_b);
    assert_eq!(false, deserealized_object.prop_c);
    assert_eq!(4356257, deserealized_object.prop_d);
    assert_eq!(4356.257, deserealized_object.prop_e);
    assert_eq!(false, deserealized_object.prop_f.unwrap().prop_foo);
}


#[test]
fn parse_nested_object_null() {
    pub struct NestedObject {
        prop_foo: bool
    }

    impl FromJSON for NestedObject {
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
                if property.property_name == "prop_foo" {
                    if value.bool.is_some() {
                        self.prop_foo = value.bool.unwrap();
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

    impl ToJSON for NestedObject {
        fn list_properties() -> Vec<JSONProperty> {
            let mut list = vec![];

            let property = JSONProperty { property_name: "prop_foo".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            list
        }

        fn get_property(&self, property_name: String) -> JSONValue {
            let mut value = JSONValue::new();

            if property_name == "prop_foo".to_string() {
                let boolean : bool = self.prop_foo;
                value.bool = Some(boolean);
            }

            value
        }

        fn to_json_string(&self) -> String {
            let mut processed_data = vec![];

            let properties = NestedObject::list_properties();
            for property in properties {
                let value = self.get_property(property.property_name.to_string());
                processed_data.push((property, value));

            }

            JSON::to_json_string(processed_data)
        }
    }

    struct SomeObject {
        prop_a: String,
        prop_b: bool,
        prop_c: bool,
        prop_d: i128,
        prop_e: f64,
        prop_f: Option<NestedObject>
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

                if property.property_name == "prop_c" {
                    self.prop_c = value.bool.unwrap();
                }

                if property.property_name == "prop_d" {
                    self.prop_d = value.i128.unwrap();
                }

                if property.property_name == "prop_e" {
                    self.prop_e = value.f64.unwrap();
                }

                if property.property_name == "prop_f" {
                    let mut prop_f = NestedObject { prop_foo: false };
                    if value.object.is_some() {
                        let unparsed_object = value.object.unwrap();
                        let boxed_parse = prop_f.parse(unparsed_object);
                        if boxed_parse.is_err() {
                            let message = boxed_parse.err().unwrap();
                            return Err(message);
                        }
                        self.prop_f = Some(prop_f);
                    } else {
                        self.prop_f = None;
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

            let property = JSONProperty { property_name: "prop_a".to_string(), property_type: JSON_TYPE.string.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_b".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_c".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_d".to_string(), property_type: JSON_TYPE.integer.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_e".to_string(), property_type: JSON_TYPE.number.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_f".to_string(), property_type: JSON_TYPE.object.to_string() };
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

            if property_name == "prop_c".to_string() {
                let boolean : bool = self.prop_c;
                value.bool = Some(boolean);
            }

            if property_name == "prop_d".to_string() {
                let integer : i128 = self.prop_d;
                value.i128 = Some(integer);
            }

            if property_name == "prop_e".to_string() {
                let floating_point_number: f64 = self.prop_e;
                value.f64 = Some(floating_point_number);
            }

            if property_name == "prop_f".to_string() {
                if self.prop_f.is_some() {
                    let prop_f = self.prop_f.as_ref().unwrap();
                    let serialized_nested_object = prop_f.to_json_string();
                    value.object = Some(serialized_nested_object);
                }
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




    let json_string = "{\r\n  \"prop_a\": \"123abc\",\r\n  \"prop_b\": true,\r\n  \"prop_c\": false,\r\n  \"prop_d\": 4356257,\r\n  \"prop_e\": 4356.257,\r\n  \"prop_f\": null\r\n}";


    let mut deserealized_object = SomeObject {
        prop_a: "".to_string(),
        prop_b: false,
        prop_c: true,
        prop_d: 0,
        prop_e: 0.0,
        prop_f: Some(NestedObject{ prop_foo: true }),
    };
    deserealized_object.parse(json_string.to_string()).unwrap();

    assert_eq!("123abc", deserealized_object.prop_a);
    assert_eq!(true, deserealized_object.prop_b);
    assert_eq!(false, deserealized_object.prop_c);
    assert_eq!(4356257, deserealized_object.prop_d);
    assert_eq!(4356.257, deserealized_object.prop_e);
    assert!(deserealized_object.prop_f.is_none());
}


#[test]
fn nested_object_none_to_string() {
    pub struct NestedObject {
        prop_foo: bool
    }

    impl FromJSON for NestedObject {
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
                if property.property_name == "prop_foo" {
                    if value.bool.is_some() {
                        self.prop_foo = value.bool.unwrap();
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

    impl ToJSON for NestedObject {
        fn list_properties() -> Vec<JSONProperty> {
            let mut list = vec![];

            let property = JSONProperty { property_name: "prop_foo".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            list
        }

        fn get_property(&self, property_name: String) -> JSONValue {
            let mut value = JSONValue::new();

            if property_name == "prop_foo".to_string() {
                let boolean : bool = self.prop_foo;
                value.bool = Some(boolean);
            }

            value
        }

        fn to_json_string(&self) -> String {
            let mut processed_data = vec![];

            let properties = NestedObject::list_properties();
            for property in properties {
                let value = self.get_property(property.property_name.to_string());
                processed_data.push((property, value));

            }

            JSON::to_json_string(processed_data)
        }
    }

    struct SomeObject {
        prop_a: String,
        prop_b: bool,
        prop_c: bool,
        prop_d: i128,
        prop_e: f64,
        prop_f: Option<NestedObject>
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

                if property.property_name == "prop_c" {
                    self.prop_c = value.bool.unwrap();
                }

                if property.property_name == "prop_d" {
                    self.prop_d = value.i128.unwrap();
                }

                if property.property_name == "prop_e" {
                    self.prop_e = value.f64.unwrap();
                }

                if property.property_name == "prop_f" {
                    let mut prop_f = NestedObject { prop_foo: false };
                    if value.object.is_some() {
                        let unparsed_object = value.object.unwrap();
                        let boxed_parse = prop_f.parse(unparsed_object);
                        if boxed_parse.is_err() {
                            let message = boxed_parse.err().unwrap();
                            return Err(message);
                        }
                        self.prop_f = Some(prop_f);
                    } else {
                        self.prop_f = None;
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

            let property = JSONProperty { property_name: "prop_a".to_string(), property_type: JSON_TYPE.string.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_b".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_c".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_d".to_string(), property_type: JSON_TYPE.integer.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_e".to_string(), property_type: JSON_TYPE.number.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_f".to_string(), property_type: JSON_TYPE.object.to_string() };
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

            if property_name == "prop_c".to_string() {
                let boolean : bool = self.prop_c;
                value.bool = Some(boolean);
            }

            if property_name == "prop_d".to_string() {
                let integer : i128 = self.prop_d;
                value.i128 = Some(integer);
            }

            if property_name == "prop_e".to_string() {
                let floating_point_number: f64 = self.prop_e;
                value.f64 = Some(floating_point_number);
            }

            if property_name == "prop_f".to_string() {
                if self.prop_f.is_some() {
                    let prop_f = self.prop_f.as_ref().unwrap();
                    let serialized_nested_object = prop_f.to_json_string();
                    value.object = Some(serialized_nested_object);
                }
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




    let json_string = "{\r\n  \"prop_a\": \"123abc\",\r\n  \"prop_b\": true,\r\n  \"prop_c\": false,\r\n  \"prop_d\": 4356257,\r\n  \"prop_e\": 4356.257,\r\n  \"prop_f\": null\r\n}";


    let mut deserealized_object = SomeObject {
        prop_a: "".to_string(),
        prop_b: false,
        prop_c: true,
        prop_d: 0,
        prop_e: 0.0,
        prop_f: Some(NestedObject{ prop_foo: true }),
    };
    deserealized_object.parse(json_string.to_string()).unwrap();

    assert_eq!("123abc", deserealized_object.prop_a);
    assert_eq!(true, deserealized_object.prop_b);
    assert_eq!(false, deserealized_object.prop_c);
    assert_eq!(4356257, deserealized_object.prop_d);
    assert_eq!(4356.257, deserealized_object.prop_e);
    assert!(deserealized_object.prop_f.is_none());

    let json = deserealized_object.to_json_string();
    let expected_json = "{\r\n  \"prop_a\": \"123abc\",\r\n  \"prop_b\": true,\r\n  \"prop_c\": false,\r\n  \"prop_d\": 4356257,\r\n  \"prop_e\": 4356.257\r\n}";
    assert_eq!(json, expected_json);
}


#[test]
fn parse_multi_nested_object() {
    pub struct NestedObject {
        prop_foo: bool,
        prop_baz: Option<AnotherNestedObject>
    }

    impl FromJSON for NestedObject {
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
                if property.property_name == "prop_foo" {
                    self.prop_foo = value.bool.unwrap();
                }
                if property.property_name == "prop_baz" {
                    let mut prop_baz = AnotherNestedObject { prop_bar: 1.1 };
                    if value.object.is_some() {
                        let unparsed_object = value.object.unwrap();
                        let boxed_parse = prop_baz.parse(unparsed_object);
                        if boxed_parse.is_err() {
                            let message = boxed_parse.err().unwrap();
                            return Err(message);
                        }
                        self.prop_baz = Some(prop_baz);
                    } else {
                        self.prop_baz = None;
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

    impl ToJSON for NestedObject {
        fn list_properties() -> Vec<JSONProperty> {
            let mut list = vec![];

            let property = JSONProperty { property_name: "prop_foo".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_baz".to_string(), property_type: JSON_TYPE.object.to_string() };
            list.push(property);

            list
        }

        fn get_property(&self, property_name: String) -> JSONValue {
            let mut value = JSONValue::new();

            if property_name == "prop_foo".to_string() {
                let boolean : bool = self.prop_foo;
                value.bool = Some(boolean);
            }

            if property_name == "prop_baz".to_string() {
                let prop_baz = self.prop_baz.as_ref().unwrap();
                let serialized_nested_object = prop_baz.to_json_string();
                value.object = Some(serialized_nested_object);
            }



            value
        }

        fn to_json_string(&self) -> String {
            let mut processed_data = vec![];

            let properties = NestedObject::list_properties();
            for property in properties {
                let value = self.get_property(property.property_name.to_string());
                processed_data.push((property, value));

            }

            JSON::to_json_string(processed_data)
        }
    }

    pub struct AnotherNestedObject {
        prop_bar: f64
    }

    impl FromJSON for AnotherNestedObject {
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
                if property.property_name == "prop_bar" {
                    self.prop_bar = value.f64.unwrap();
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

    impl ToJSON for AnotherNestedObject {
        fn list_properties() -> Vec<JSONProperty> {
            let mut list = vec![];

            let property = JSONProperty { property_name: "prop_bar".to_string(), property_type: JSON_TYPE.number.to_string() };
            list.push(property);

            list
        }

        fn get_property(&self, property_name: String) -> JSONValue {
            let mut value = JSONValue::new();

            if property_name == "prop_bar".to_string() {
                let number : f64 = self.prop_bar;
                value.f64 = Some(number);
            }

            value
        }

        fn to_json_string(&self) -> String {
            let mut processed_data = vec![];

            let properties = AnotherNestedObject::list_properties();
            for property in properties {
                let value = self.get_property(property.property_name.to_string());
                processed_data.push((property, value));

            }

            JSON::to_json_string(processed_data)
        }
    }

    struct SomeObject {
        prop_a: String,
        prop_b: bool,
        prop_c: bool,
        prop_d: i128,
        prop_e: f64,
        prop_f: Option<NestedObject>
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

                if property.property_name == "prop_c" {
                    self.prop_c = value.bool.unwrap();
                }

                if property.property_name == "prop_d" {
                    self.prop_d = value.i128.unwrap();
                }

                if property.property_name == "prop_e" {
                    self.prop_e = value.f64.unwrap();
                }

                if property.property_name == "prop_f" {
                    let mut prop_f = NestedObject { prop_foo: false, prop_baz: None };
                    if value.object.is_some() {
                        let unparsed_object = value.object.unwrap();
                        let boxed_parse = prop_f.parse(unparsed_object);
                        if boxed_parse.is_err() {
                            let message = boxed_parse.err().unwrap();
                            return Err(message);
                        }
                        self.prop_f = Some(prop_f);
                    } else {
                        self.prop_f = None;
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

            let property = JSONProperty { property_name: "prop_a".to_string(), property_type: JSON_TYPE.string.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_b".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_c".to_string(), property_type: JSON_TYPE.boolean.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_d".to_string(), property_type: JSON_TYPE.integer.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_e".to_string(), property_type: JSON_TYPE.number.to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_f".to_string(), property_type: JSON_TYPE.object.to_string() };
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

            if property_name == "prop_c".to_string() {
                let boolean : bool = self.prop_c;
                value.bool = Some(boolean);
            }

            if property_name == "prop_d".to_string() {
                let integer : i128 = self.prop_d;
                value.i128 = Some(integer);
            }

            if property_name == "prop_e".to_string() {
                let floating_point_number: f64 = self.prop_e;
                value.f64 = Some(floating_point_number);
            }

            if property_name == "prop_f".to_string() {
                let prop_f = self.prop_f.as_ref().unwrap();
                let serialized_nested_object = prop_f.to_json_string();
                value.object = Some(serialized_nested_object);
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

    let nested_obj = NestedObject
    {
        prop_foo: true,
        prop_baz: Some(AnotherNestedObject {
            prop_bar: 2.2
        })
    };

    let obj = SomeObject {
        prop_a: "123abc".to_string(),
        prop_b: true,
        prop_c: false,
        prop_d: 4356257,
        prop_e: 4356.257,
        prop_f: Some(nested_obj),
    };

    let json_string = obj.to_json_string();
    let expected_json_string = "{\r\n  \"prop_a\": \"123abc\",\r\n  \"prop_b\": true,\r\n  \"prop_c\": false,\r\n  \"prop_d\": 4356257,\r\n  \"prop_e\": 4356.257,\r\n  \"prop_f\": {\r\n  \"prop_foo\": true,\r\n  \"prop_baz\": {\r\n  \"prop_bar\": 2.2\r\n}\r\n}\r\n}";

    assert_eq!(expected_json_string, json_string);

    let mut deserealized_object = SomeObject {
        prop_a: "".to_string(),
        prop_b: false,
        prop_c: true,
        prop_d: 0,
        prop_e: 0.0,
        prop_f: None,
    };
    deserealized_object.parse(json_string.to_string()).unwrap();

    assert_eq!("123abc", deserealized_object.prop_a);
    assert_eq!(true, deserealized_object.prop_b);
    assert_eq!(false, deserealized_object.prop_c);
    assert_eq!(4356257, deserealized_object.prop_d);
    assert_eq!(4356.257, deserealized_object.prop_e);

    let nested_obj = deserealized_object.prop_f.unwrap();
    assert_eq!(true, nested_obj.prop_foo);

    let another_nested_obj = nested_obj.prop_baz.unwrap();
    assert_eq!(another_nested_obj.prop_bar, 2.2);
}

