use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;
use crate::json::array::New;

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

pub struct Null {}

pub struct ParseNullError {
    pub message: String
}

impl Debug for ParseNullError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl Display for Null {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("null")
    }
}

impl PartialEq for Null {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Debug for Null {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("null")
    }
}

impl New for Null {
    fn new() -> Self {
        Null{}
    }
}

impl Clone for Null {
    fn clone(&self) -> Self {
        Null::new()
    }
}

impl FromStr for Null {
    type Err = ParseNullError;

    fn from_str(null: &str) -> Result<Self, Self::Err> {
        if null.trim() != "null" {
            let message = format!("error parsing null: {}", null);
            return Err(ParseNullError { message })
        }
        Ok(Null::new())
    }
}



