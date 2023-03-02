
#[cfg(test)]
mod tests;
mod key_value;

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

pub struct JSONValue {
    pub f64: Option<f64>,
    pub i128: Option<i128>,
    pub String: Option<String>,
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
}

