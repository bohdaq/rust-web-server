use std::collections::HashMap;
use crate::json::object::{FromJSON, ToJSON};

#[cfg(test)]
mod tests;

pub struct UrlPath;

impl UrlPath {
    pub fn is_matching(_path: &str, _pattern: &str) -> Result<bool, String> {
        //TODO

        // extract static parts and name of keys
        for _char in _path.chars() {
            if _char.is_whitespace() || _char.is_ascii_control() {
                return Err("path contains control character or whitespace".to_string())
            }
        }

        for _char in _pattern.chars() {
            if _char.is_whitespace() || _char.is_ascii_control() {
                return Err("path contains control character or whitespace".to_string())
            }
        }

        let parts: Vec<String> = vec![];
        let mut buffer: Vec<char> = vec![];
        let mut is_static_part = true;
        let mut previous_char: Option<char> = None;

        for _char in _pattern.chars() {

            if _char == '[' && previous_char.is_some() && previous_char.unwrap() == '[' {
                buffer = vec![];
                is_static_part = false;
                return Err("path contains control character or whitespace".to_string())
            } else if is_static_part {
                buffer.push(_char)
            } else if _char == ']' && previous_char.is_some() && previous_char.unwrap() == ']' {
                is_static_part = true;
            }

            previous_char = Some(_char.clone());
        }

        Ok(true)
    }

    pub fn extract<T: FromJSON + ToJSON>(_path: &str, _pattern: &str) -> Result<HashMap<String, T>, String> {
        //TODO

        let map = HashMap::new();
        Ok(map)
    }

    pub fn build<T: FromJSON + ToJSON>(_params: HashMap<String, T>, _pattern: &str) -> Result<String, String> {
        //TODO

        Ok("generated path here".to_string())
    }
}