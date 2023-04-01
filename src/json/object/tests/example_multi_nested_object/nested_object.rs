use crate::json::{JSON_TYPE, JSONValue};
use crate::json::array::New;
use crate::json::object::{FromJSON, JSON, ToJSON};
use crate::json::object::tests::example_multi_nested_object::AnotherNestedObject;
use crate::json::property::JSONProperty;

// define your struct
pub struct NestedObject {
    pub prop_foo: bool,
    pub prop_baz: Option<AnotherNestedObject>
}

impl New for NestedObject {
    // initiate struct with default values
    fn new() -> Self {
        NestedObject { prop_foo: false, prop_baz: None }
    }
}

impl ToJSON for NestedObject {
    // here you need to list fields used in your struct
    fn list_properties() -> Vec<JSONProperty> {
        let mut list = vec![];

        let property = JSONProperty { property_name: "prop_foo".to_string(), property_type: JSON_TYPE.boolean.to_string() };
        list.push(property);

        let property = JSONProperty { property_name: "prop_baz".to_string(), property_type: JSON_TYPE.object.to_string() };
        list.push(property);

        list
    }

    // here you need to use fields used in your struct
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

    // change NestedObject to your struct, update nested if statements in for loop according to your struct fields
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

impl FromJSON for NestedObject {
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


// it is basically shortcut for instantiation and parse, replace NestedObject with your struct name, can be copy-pasted
//     let mut object = NestedObject::new();
//     let parse_result = object.parse(json);
impl NestedObject {
    pub fn _parse_json(json: &str) -> Result<NestedObject, String> {
        let mut some_object = NestedObject::new();
        let parse_result = some_object.parse(json.to_string());
        if parse_result.is_err() {
            let message = parse_result.err().unwrap();
            return Err(message);
        }

        Ok(some_object)
    }
}