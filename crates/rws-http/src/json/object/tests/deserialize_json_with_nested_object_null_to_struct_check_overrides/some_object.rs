use crate::json::{JSON_TYPE};
use crate::core::New;
use crate::json::object::{FromJSON, JSON, ToJSON};
use crate::json::object::tests::deserialize_json_with_nested_object_null_to_struct_check_overrides::nested_object::NestedObject;
use crate::json::property::{JSONProperty, JSONValue};

pub struct SomeObject {
    pub prop_a: String,
    pub prop_b: bool,
    pub prop_c: bool,
    pub prop_d: i128,
    pub prop_e: f64,
    pub prop_f: Option<NestedObject>
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
