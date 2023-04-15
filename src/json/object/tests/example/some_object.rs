use crate::json::{JSON_TYPE};
use crate::core::New;
use crate::json::object::{FromJSON, JSON, ToJSON};
use crate::json::property::{JSONProperty, JSONValue};
use crate::symbol::SYMBOL;

// define your struct
pub struct SomeObject {
    pub prop_a: String,
    pub prop_b: bool
}

impl New for SomeObject {
    // initiate struct with default values
    fn new() -> Self {
        SomeObject { prop_a: "".to_string(), prop_b: false }
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

        value
    }

    // change SomeObject to your struct, update nested if statements in for loop according to your struct fields
    fn to_json_string(&self) -> String {
        let mut json_list = vec![];
        json_list.push(SYMBOL.opening_curly_bracket.to_string());


        let mut properties_list = vec![];

        let properties = SomeObject::list_properties();
        for property in properties {
            let value = self.get_property(property.property_name.to_string());

            if &property.property_type == JSON_TYPE.string {
                let raw_value = value.string.unwrap();
                let formatted_property = format!("  \"{}\": \"{}\"", &property.property_name, raw_value);
                properties_list.push(formatted_property.to_string());
            }

            if &property.property_type == JSON_TYPE.boolean {
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