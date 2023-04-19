use std::collections::HashMap;
use url_search_params::{build_url_search_params, parse_url_search_params};
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

    pub fn generate(map: HashMap<String, String>) -> String {
        let search_params = build_url_search_params(map);

        let mut params_as_list : Vec<&str> = search_params.split("&").collect::<Vec<&str>>();
        params_as_list.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

        let params = params_as_list.join("&");

        params
    }
}