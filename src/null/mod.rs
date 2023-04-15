#[cfg(test)]
mod tests;

use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;
use crate::core::New;

pub const NULL: &'static Null = &Null{};

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