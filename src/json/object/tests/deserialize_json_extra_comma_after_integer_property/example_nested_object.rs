use crate::json::{JSON_TYPE, JSONValue};
use crate::json::array::New;
use crate::json::object::{FromJSON, JSON, ToJSON};
use crate::json::property::JSONProperty;

pub struct ExampleNestedObject {
    pub prop_a: String,
    pub prop_b: bool,
    pub prop_c: i128,
    pub prop_d: f64
}

impl New for ExampleNestedObject {
    fn new() -> Self {
        ExampleNestedObject {
            prop_a: "".to_string(),
            prop_b: false,
            prop_c: 0,
            prop_d: 0.0,
        }
    }
}

impl FromJSON for ExampleNestedObject {
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
                if value.string.is_some() {
                    self.prop_a = value.string.unwrap();
                }
            }
            if property.property_name == "prop_b" {
                if value.bool.is_some() {
                    self.prop_b = value.bool.unwrap();
                }
            }


            if property.property_name == "prop_d" {
                if value.i128.is_some() {
                    self.prop_c = value.i128.unwrap();
                }
            }

            if property.property_name == "prop_e" {
                if value.f64.is_some() {
                    self.prop_d = value.f64.unwrap();

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

impl ToJSON for ExampleNestedObject {
    fn list_properties() -> Vec<JSONProperty> {
        let mut list = vec![];

        let property = JSONProperty { property_name: "prop_a".to_string(), property_type: JSON_TYPE.string.to_string() };
        list.push(property);

        let property = JSONProperty { property_name: "prop_b".to_string(), property_type: JSON_TYPE.boolean.to_string() };
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


        if property_name == "prop_d".to_string() {
            let integer : i128 = self.prop_c;
            value.i128 = Some(integer);
        }

        if property_name == "prop_e".to_string() {
            let floating_point_number: f64 = self.prop_d;
            value.f64 = Some(floating_point_number);
        }

        value
    }

    fn to_json_string(&self) -> String {
        let mut processed_data = vec![];

        let properties = ExampleNestedObject::list_properties();
        for property in properties {
            let value = self.get_property(property.property_name.to_string());
            processed_data.push((property, value));

        }

        JSON::to_json_string(processed_data)
    }
}
