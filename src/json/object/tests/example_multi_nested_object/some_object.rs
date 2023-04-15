use crate::json::{JSON_TYPE};
use crate::core::New;
use crate::json::object::{FromJSON, JSON, ToJSON};
use crate::json::object::tests::example_multi_nested_object::nested_object::NestedObject;
use crate::json::property::{JSONProperty, JSONValue};

// define your struct
pub struct SomeObject {
    pub prop_a: String,
    pub prop_b: bool,
    pub prop_c: bool,
    pub prop_d: i128,
    pub prop_e: f64,
    pub prop_f: Option<NestedObject>
}

impl New for SomeObject {
    // initiate struct with default values
    fn new() -> Self {
        SomeObject {
            prop_a: "".to_string(),
            prop_b: false,
            prop_c: false,
            prop_d: 0,
            prop_e: 0.0,
            prop_f: None
        }
    }
}

impl ToJSON for SomeObject {
    // here you need to list fields used in your struct
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

    // here you need to use fields used in your struct
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

    // change SomeObject to your struct, update nested if statements in for loop according to your struct fields
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

impl FromJSON for SomeObject {
    // can be copy-pasted
    fn parse_json_to_properties(&self, json_string: String) -> Result<Vec<(JSONProperty, JSONValue)>, String> {
        let boxed_parse = JSON::parse_as_properties(json_string);
        if boxed_parse.is_err() {
            let message = boxed_parse.err().unwrap();
            return Err(message)
        }
        let properties = boxed_parse.unwrap();
        Ok(properties)
    }

    // here you need to change if statements inside for loop corresponding to your struct fields
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

    // can be copy-pasted
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


impl SomeObject {
    // it is basically shortcut for instantiation and parse, replace SomeObject with your struct name, can be copy-pasted
    //     let mut some_object = SomeObject::new();
    //     let parse_result = some_object.parse(json);
    pub fn parse_json(json: &str) -> Result<SomeObject, String> {
        let mut some_object = SomeObject::new();
        let parse_result = some_object.parse(json.to_string());
        if parse_result.is_err() {
            let message = parse_result.err().unwrap();
            return Err(message);
        }

        Ok(some_object)
    }
}