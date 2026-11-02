use crate::json::array::RawUnprocessedJSONArray;
use crate::symbol::SYMBOL;

#[cfg(test)]
mod example_list_bool_with_asserts;
#[cfg(test)]
mod example_list_bool;

pub struct JSONArrayOfBooleans;
impl JSONArrayOfBooleans {
    pub fn parse_as_list_bool(json : String) -> Result<Vec<bool>, String> {
        let items = RawUnprocessedJSONArray::split_into_vector_of_strings(json).unwrap();
        let mut list: Vec<bool> = vec![];
        for item in items {
            let boxed_parse = item.parse::<bool>();
            if boxed_parse.is_err() {
                let message = boxed_parse.err().unwrap().to_string();
                return Err(message);
            }
            let boolean: bool = boxed_parse.unwrap();
            list.push(boolean);
        }
        Ok(list)
    }

    pub fn to_json_from_list_bool(items : &Vec<bool>) -> Result<String, String> {
        let mut json_vec = vec![];
        json_vec.push(SYMBOL.opening_square_bracket.to_string());
        for (pos, item) in items.iter().enumerate() {
            json_vec.push(item.to_string());
            if pos != items.len() - 1 {
                json_vec.push(SYMBOL.comma.to_string());
            }
        }
        json_vec.push(SYMBOL.closing_square_bracket.to_string());

        let result = json_vec.join(SYMBOL.empty_string);
        Ok(result)
    }

}
