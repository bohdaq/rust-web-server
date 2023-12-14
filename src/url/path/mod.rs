use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use crate::json::object::{FromJSON, ToJSON};
use crate::symbol::SYMBOL;

#[cfg(test)]
mod tests;

pub struct UrlPath;

#[derive(Clone, Debug)]
pub struct Part {
    pub is_static: bool,
    pub name: Option<String>,
    pub value: Option<String>,
    pub static_pattern: Option<String>
}

impl Display for Part {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        println!("{:?} {:?} {:?} {:?}", &self.is_static, &self.name, &self.value, &self.static_pattern);
        Ok(())
    }
}

impl UrlPath {
    pub fn extract_parts_from_pattern(_pattern: &str) -> Result<Vec<Part>, String>{
        let mut part_list: Vec<Part> = vec![];
        let mut buffer: Vec<char> = vec![];
        let mut is_static_part = true;
        let mut previous_char: Option<char> = None;
        let mut is_opened_token = false;

        for _char in _pattern.chars() {
            if _char.is_whitespace() || _char.is_control() {
                return Err("path contains control character or whitespace".to_string())
            }

            buffer.push(_char);

            if _char == '[' && previous_char.is_some() && previous_char.unwrap() == '[' {
                if is_opened_token {
                    return Err("at least one extra [ char".to_string());
                }
                is_opened_token = true;
                if buffer.len() != 0 && buffer.len() >= 2 {
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
                is_opened_token = false;
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

        if buffer.len() != 0 {
            let static_ending : String = buffer.into_iter().collect();

            let part = Part {
                is_static: true,
                name: None,
                value: None,
                static_pattern: Some(static_ending),
            };
            part_list.push(part);
            buffer = vec![];
        }

        Ok(part_list)
    }

    pub fn is_matching(_path: &str, _pattern: &str) -> Result<bool, String> {
        let is_matching = true;
        let is_not_matching = false;

        for char in _path.chars() {
            if char.is_whitespace() || char.is_control() {
                return Err("path contains control character or whitespace".to_string());
            }
        }

        let boxed_parts = UrlPath::extract_parts_from_pattern(_pattern);
        if boxed_parts.is_err() {
            return Err(boxed_parts.err().unwrap());
        }

        let parts : Vec<Part> = boxed_parts.unwrap();
        let mut populated_parts : Vec<Part> = vec![];

        let mut url_path = _path.to_string();
        let number_of_parts = parts.len();
        println!("number_of_parts, {}", number_of_parts);

        for (index, part) in parts.iter().enumerate() {
            let part = part;
            println!("1, {}", part);
            let is_there_next_part = index < number_of_parts - 1;
            if part.is_static {
                let static_pattern = part.static_pattern.clone().unwrap();
                if !url_path.starts_with(&static_pattern) {
                    return Ok(is_not_matching)
                }

                let _part = part.clone();
                populated_parts.push(_part);

                url_path = url_path.replacen(static_pattern.as_str(), SYMBOL.empty_string, 1);
            } else {
                if !is_there_next_part {
                    let _part = Part {
                        is_static: false,
                        name: part.name.clone(),
                        value: Some(url_path.clone()),
                        static_pattern: None,
                    };
                    let _part = part.clone();
                    populated_parts.push(_part);

                    println!("2, {}", part);

                } else {
                    let next_part = parts.get(index + 1).unwrap();
                    println!("3, {}", part);
                    let delimiter = next_part.static_pattern.clone().unwrap().chars().next().unwrap();
                    let occurence = url_path.find(delimiter);
                    if occurence.is_none() {
                        return Ok(is_not_matching)
                    }
                    let occurence_place = occurence.unwrap();
                    let token = url_path[..occurence_place].to_string();
                    let _part = Part {
                        is_static: false,
                        name: part.name.clone(),
                        value: Some(token.clone()),
                        static_pattern: None,
                    };
                    println!("4, {}", part);

                    populated_parts.push(_part.clone());

                    url_path = url_path.chars().skip(occurence_place).collect();

                    println!("123")
                }
            }
        }
        
        Ok(is_matching)
    }

    pub fn extract(_path: &str, _pattern: &str) -> Result<HashMap<String, String>, String> {
        //TODO

        let boxed_parts = UrlPath::extract_parts_from_pattern(_pattern);
        if boxed_parts.is_err() {
            return Err(boxed_parts.err().unwrap());
        }

        let parts : Vec<Part> = boxed_parts.unwrap();
        let mut resulting_parts : Vec<Part> = vec![];

        let mut path = _path.to_string();
        let mut previous_part: Option<Part> = None;
        for part in parts.iter() {
            // println!("path: {:?}", path);
            // println!("part: {:?} {:?} {:?}", part.name, part.value, part.static_pattern);

            if part.is_static {
                if previous_part.is_some() {
                    let unboxed_previous_part = previous_part.clone().unwrap();
                    // println!("previous_part: {:?} {:?} {:?}", unboxed_previous_part.name, unboxed_previous_part.value, unboxed_previous_part.static_pattern);

                    // read until first char of static pattern
                    // add to map
                    let static_pattern = part.static_pattern.clone().unwrap();
                    let first_char_to_stop = static_pattern.chars().next().unwrap();
                    let mut buffer = vec![];

                    for char in path.clone().chars() {
                        if char == first_char_to_stop {
                            // println!("found {:?}", char);
                            // read the rest of static pattern
                            break;
                        } else {
                            let removed_char = path.remove(0);
                            buffer.push(removed_char);
                            // println!("removed char: {:?}", removed_char);
                        }
                    }
                    let token : String = buffer.iter().collect();
                    // println!("dynamic part {:?}", token);
                    let mut processed_part = previous_part.clone().unwrap();
                    processed_part.value = Some(token);
                    resulting_parts.push(processed_part.clone());
                    println!("{:?}", processed_part)
                } else {
                    // read the rest of static pattern
                }

                let static_pattern = part.static_pattern.clone().unwrap();
                // println!("static pattern {:?}", static_pattern);
                // println!("path {:?}", path);
                path = path.strip_prefix(static_pattern.as_str()).unwrap().to_string();
            } else {
                // continue, unless the part is last,
                // if so read to the end of path and add to map
            }

            previous_part = Some(part.clone());
        }

        let mut map = HashMap::new();
        for part in resulting_parts {
            let key = part.name.unwrap();
            let value = part.value.unwrap();

            map.insert(key, value);
        }

        Ok(map)
    }

    pub fn build(_params: HashMap<String, String>, _pattern: &str) -> Result<String, String> {
        let boxed_parts = UrlPath::extract_parts_from_pattern(_pattern);
        if boxed_parts.is_err() {
            return Err(boxed_parts.err().unwrap());
        }

        let parts : Vec<Part> = boxed_parts.unwrap();


        let mut strings_array = vec![];
        for part  in parts {
            if part.is_static {
                strings_array.push(part.static_pattern.unwrap())
            } else {
                let key = part.name.unwrap().to_string();
                let boxed_value = _params.get(key.as_str());
                if boxed_value.is_none() {
                    return Err(format!("specified parameter {} is not found", key))
                }
                let value = boxed_value.unwrap();
                strings_array.push(value.clone());
            }
        }
        let populated_pattern : String = strings_array.join(SYMBOL.empty_string);

        Ok(populated_pattern)

    }
}