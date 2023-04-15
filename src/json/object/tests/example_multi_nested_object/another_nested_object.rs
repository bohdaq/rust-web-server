use crate::json::{JSON_TYPE};
use crate::core::New;
use crate::json::object::{FromJSON, JSON, ToJSON};
use crate::json::property::{JSONProperty, JSONValue};

// define your struct
pub struct AnotherNestedObject {
    pub prop_bar: f64
}

impl New for AnotherNestedObject {
    // initiate struct with default values
    fn new() -> Self {
        AnotherNestedObject { prop_bar: 0.0 }
    }
}

impl ToJSON for AnotherNestedObject {
    // here you need to list fields used in your struct
    fn list_properties() -> Vec<JSONProperty> {
        let mut list = vec![];

        let property = JSONProperty { property_name: "prop_bar".to_string(), property_type: JSON_TYPE.number.to_string() };
        list.push(property);

        list
    }

    // here you need to use fields used in your struct
    fn get_property(&self, property_name: String) -> JSONValue {
        let mut value = JSONValue::new();

        if property_name == "prop_bar".to_string() {
            let number : f64 = self.prop_bar;
            value.f64 = Some(number);
        }

        value
    }

    // change AnotherNestedObject to your struct, update nested if statements in for loop according to your struct fields
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

impl FromJSON for AnotherNestedObject {
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
            if property.property_name == "prop_bar" {
                self.prop_bar = value.f64.unwrap();
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