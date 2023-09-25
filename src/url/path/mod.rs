use std::collections::HashMap;
use crate::json::object::{FromJSON, ToJSON};

#[cfg(test)]
mod tests;

pub struct UrlPath;

pub struct Part {
    pub is_static: bool,
    pub name: Option<String>,
    pub value: Option<String>,
    pub static_pattern: Option<String>
}

impl UrlPath {
    pub fn extract_parts_from_pattern(_pattern: &str) -> Result<Vec<Part>, String>{
        let mut part_list: Vec<Part> = vec![];
        let mut buffer: Vec<char> = vec![];
        let mut is_static_part = true;
        let mut previous_char: Option<char> = None;

        for _char in _pattern.chars() {

            buffer.push(_char);

            if _char == '[' && previous_char.is_some() && previous_char.unwrap() == '[' {
                if buffer.len() != 0 {
                    let without_square_brackets = buffer.len() - 2;
                    let pattern : String = buffer[0..without_square_brackets].into_iter().collect();
                    let part = Part {
                        is_static: true,
                        name: None,
                        value: None,
                        static_pattern: Some(pattern),
                    };
                    part_list.push(part);
                }
                buffer = vec![];
                is_static_part = false;
            }

            if _char == ']' && previous_char.is_some() && previous_char.unwrap() == ']' {
                let without_square_brackets = buffer.len() - 2;
                let key : String = buffer[0..without_square_brackets].into_iter().collect();
                let part = Part {
                    is_static: false,
                    name: Some(key),
                    value: None,
                    static_pattern: None,
                };
                part_list.push(part);

                is_static_part = true;
                buffer = vec![];

            }

            previous_char = Some(_char.clone());
        }

        Ok(part_list)
    }

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