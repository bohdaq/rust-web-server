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
            if _char.is_whitespace() {
                return Err("pattern contains whitespace".to_string())
            }

            buffer.push(_char);

            if _char == '[' && previous_char.is_some() && previous_char.unwrap() == '[' {
                if buffer.len() != 0 {
                    let without_square_brackets = buffer.len() - 2;
                    let pattern : String = buffer[0..without_square_brackets].into_iter().collect();
                    if pattern.len() > 0 {
                        let part = Part {
                            is_static: true,
                            name: None,
                            value: None,
                            static_pattern: Some(pattern),
                        };
                        part_list.push(part);
                    }
                }
                buffer = vec![];
                is_static_part = false;

                let previous_part_is_token = part_list.last().is_some() && part_list.last().unwrap().is_static == false;
                if previous_part_is_token {
                    return Err("two consecutive tokens one after another".to_string())
                }
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
        let boxed_parts = UrlPath::extract_parts_from_pattern(_pattern);
        if boxed_parts.is_err() {
            return Err(boxed_parts.err().unwrap());
        }

        let parts : Vec<Part> = boxed_parts.unwrap();

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