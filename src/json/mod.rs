#[cfg(test)]
mod tests;
pub mod property;
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

pub struct JSONValue {
    pub f64: Option<f64>,
    pub i128: Option<i128>,
    pub string: Option<String>,
    pub object: Option<String>,
    pub bool: Option<bool>,
    pub null: Option<Null>,
}

pub struct Null {}



