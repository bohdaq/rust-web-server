use std::fmt::{Display, Formatter};
use crate::null::Null;

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
    pub array: Option<String>,
    pub bool: Option<bool>,
    pub null: Option<Null>,
}

impl JSONValue {
    pub fn new() -> JSONValue {
        JSONValue {
            f64: None,
            i128: None,
            string: None,
            object: None,
            array: None,
            bool: None,
            null: None,
        }
    }
}

impl Display for JSONValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.f64.is_some() {
            let f64 = self.f64.unwrap();
            let formatted : String = format!("{:.13}", f64);
            return f.write_str(formatted.as_str());
        }

        if self.i128.is_some() {
            let formatted = self.i128.unwrap().to_string();
            return f.write_str(formatted.as_str());
        }

        if self.string.is_some() {
            let formatted = self.string.as_ref().unwrap();
            return f.write_str(formatted.as_str());
        }

        if self.array.is_some() {
            let formatted = self.array.as_ref().unwrap();
            return f.write_str(formatted.as_str());
        }

        if self.null.is_some() {
            return f.write_str("null");
        }

        f.write_str("Something Went Wrong")

    }
}






