use crate::json::key_value::parse_json_property;

#[cfg(test)]
mod tests;
pub mod key_value;
pub mod array;
pub mod object;

// TODO: wip

pub struct JSONType {
    pub string: &'static str,
    pub boolean: &'static str,
    pub object: &'static str,
    pub array: &'static str,
    pub integer: &'static str,
    pub number: &'static str,
    pub null: &'static str,
}

pub const JSON_TYPE: JSONType = JSONType{
    string: "String",
    boolean: "bool",
    object: "object",
    array: "array",
    integer: "i128",
    number: "f64",
    null: "null",
};

pub struct JSONProperty {
    pub property_name: String,
    pub property_type: String,
}

impl JSONProperty {
    pub fn parse(raw_string: &str) -> Result<(JSONProperty, JSONValue), String> {
        parse_json_property(raw_string)
    }
}

pub struct JSONValue {
    pub f64: Option<f64>,
    pub i128: Option<i128>,
    pub string: Option<String>,
    pub object: Option<String>,
    pub bool: Option<bool>,
    pub null: Option<Null>,
}

pub struct Null {}

pub trait ToJSON {
    fn list_properties() -> Vec<JSONProperty>;
    fn get_property(&self, property_name: String) -> JSONValue;
    fn to_json_string(&self) -> String;
}

pub trait FromJSON {
    fn parse_json_to_properties(&self, json_string: String) -> Result<Vec<(JSONProperty, JSONValue)>, String>;
    fn set_properties(&mut self, properties: Vec<(JSONProperty, JSONValue)>) -> Result<(), String>;
    fn parse(&mut self, json_string: String) -> Result<(), String>;
}

