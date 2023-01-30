use std::collections::HashMap;
use url_search_params::parse_url_search_params;
use crate::symbol::SYMBOL;

pub struct FormUrlEncoded;

impl FormUrlEncoded {
    pub fn parse(data: Vec<u8>) -> Result<HashMap<String, String>, String> {
        let boxed_string = String::from_utf8(data);
        if boxed_string.is_err() {
            let message = boxed_string.err().unwrap().to_string();
            return Err(message)
        }
        let string = boxed_string.unwrap();
        let string = string.replace(|x : char | x.is_ascii_control(), SYMBOL.empty_string).trim().to_string();


        Ok(parse_url_search_params(&string))
    }
}