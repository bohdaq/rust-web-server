use crate::json::{JSON_TYPE};
use crate::core::New;
use crate::json::array::object::JSONArrayOfObjects;
use crate::json::object::{FromJSON, JSON, ToJSON};
use crate::json::object::tests::deserialize_json_missing_comma_after_null::example_nested_object::ExampleNestedObject;
use crate::json::property::{JSONProperty, JSONValue};

pub struct ExampleObject {
    pub prop_a: String,
    pub prop_b: bool,
    pub prop_c: bool,
    pub prop_d: i128,
    pub prop_e: f64,
    pub prop_f: Option<Vec<ExampleNestedObject>>,
    pub prop_g: Option<ExampleNestedObject>
}

impl New for ExampleObject {
    fn new() -> Self {
        ExampleObject {
            prop_a: "".to_string(),
            prop_b: false,
            prop_c: false,
            prop_d: 0,
            prop_e: 0.0,
            prop_f: None,
            prop_g: None,
        }
    }
}

impl FromJSON for ExampleObject {
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

            if property.property_name == "prop_c" {
                if value.bool.is_some() {
                    self.prop_c = value.bool.unwrap();
                }
            }

            if property.property_name == "prop_d" {
                if value.i128.is_some() {
                    self.prop_d = value.i128.unwrap();
                }
            }

            if property.property_name == "prop_e" {
                if value.f64.is_some() {
                    self.prop_e = value.f64.unwrap();

                }
            }

            if property.property_name == "prop_f" {
                if value.array.is_some() {
                    let boxed_array = JSONArrayOfObjects::<ExampleNestedObject>::from_json(value.array.unwrap());
                    if boxed_array.is_ok() {
                        self.prop_f = Some(boxed_array.unwrap());
                    }

                }
            }

            if property.property_name == "prop_g" {
                let mut prop_g = ExampleNestedObject::new();
                if value.object.is_some() {
                    let unparsed_object = value.object.unwrap();
                    let boxed_parse = prop_g.parse(unparsed_object);
                    if boxed_parse.is_err() {
                        let message = boxed_parse.err().unwrap();
                        return Err(message);
                    }
                    self.prop_g = Some(prop_g);
                } else {
                    self.prop_g = None;
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

impl ToJSON for ExampleObject {
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

        let property = JSONProperty { property_name: "prop_f".to_string(), property_type: JSON_TYPE.array.to_string() };
        list.push(property);

        let property = JSONProperty { property_name: "prop_g".to_string(), property_type: JSON_TYPE.object.to_string() };
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
                let array = self.prop_f.as_ref().unwrap();
                let boxed_json = JSONArrayOfObjects::<ExampleNestedObject>::to_json(array);
                if boxed_json.is_ok() {
                    let json = boxed_json.unwrap();
                    value.array = Some(json);
                }
            }
        }

        if property_name == "prop_g".to_string() {
            if self.prop_g.is_some() {
                let object = self.prop_g.as_ref().unwrap();
                let json = object.to_json_string();
                value.object = Some(json);
            }
        }

        value
    }

    fn to_json_string(&self) -> String {
        let mut processed_data = vec![];

        let properties = ExampleObject::list_properties();
        for property in properties {
            let value = self.get_property(property.property_name.to_string());
            processed_data.push((property, value));

        }

        JSON::to_json_string(processed_data)
    }
}