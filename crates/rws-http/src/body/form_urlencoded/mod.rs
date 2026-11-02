use std::collections::HashMap;
use crate::symbol::SYMBOL;
use crate::url::URL;

/// Parser and serialiser for `application/x-www-form-urlencoded` bodies.
pub struct FormUrlEncoded;

impl FormUrlEncoded {
    /// Parses a URL-encoded body into a `HashMap` of field name → value.
    pub fn parse(data: Vec<u8>) -> Result<HashMap<String, String>, String> {
        let boxed_string = String::from_utf8(data);
        if boxed_string.is_err() {
            let message = boxed_string.err().unwrap().to_string();
            return Err(message)
        }
        let string = boxed_string.unwrap();
        let string = string.replace(|x : char | x.is_ascii_control(), SYMBOL.empty_string).trim().to_string();


        Ok(URL::parse_query(&string))
    }

    pub fn generate(map: HashMap<String, String>) -> String {
        URL::build_query(map)
    }
}