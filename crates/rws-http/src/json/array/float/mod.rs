use crate::json::array::RawUnprocessedJSONArray;
use crate::symbol::SYMBOL;

#[cfg(test)]
mod example_list_f64_with_asserts;
#[cfg(test)]
mod example_list_f64;
#[cfg(test)]
mod example_list_f32_with_asserts;
#[cfg(test)]
mod example_list_f32;

pub struct JSONArrayOfFloats;
impl JSONArrayOfFloats {
    pub fn parse_as_list_f64(json : String) -> Result<Vec<f64>, String> {
        let items = RawUnprocessedJSONArray::split_into_vector_of_strings(json).unwrap();
        let mut list: Vec<f64> = vec![];
        for item in items {
            let boxed_parse = item.parse::<f64>();
            if boxed_parse.is_err() {
                let message = boxed_parse.err().unwrap().to_string();
                return Err(message);
            }
            let num : f64 = boxed_parse.unwrap();
            list.push(num);
        }
        Ok(list)
    }

    pub fn to_json_from_list_f64(items : &Vec<f64>) -> Result<String, String> {
        let mut json_vec = vec![];
        json_vec.push(SYMBOL.opening_square_bracket.to_string());
        for (pos, item) in items.iter().enumerate() {
            let mut formatted = "0.0".to_string();
            if item != &0.0 {
                formatted = item.to_string();
            }
            json_vec.push(formatted);
            if pos != items.len() - 1 {
                json_vec.push(SYMBOL.comma.to_string());
            }
        }
        json_vec.push(SYMBOL.closing_square_bracket.to_string());

        let result = json_vec.join(SYMBOL.empty_string);
        Ok(result)
    }

    pub fn parse_as_list_f32(json : String) -> Result<Vec<f32>, String> {
        let items = RawUnprocessedJSONArray::split_into_vector_of_strings(json).unwrap();
        let mut list: Vec<f32> = vec![];
        for item in items {
            let boxed_parse = item.parse::<f32>();
            if boxed_parse.is_err() {
                let message = boxed_parse.err().unwrap().to_string();
                return Err(message);
            }
            let num : f32 = boxed_parse.unwrap();
            list.push(num);
        }
        Ok(list)
    }

    pub fn to_json_from_list_f32(items : &Vec<f32>) -> Result<String, String> {
        let mut json_vec = vec![];
        json_vec.push(SYMBOL.opening_square_bracket.to_string());
        for (pos, item) in items.iter().enumerate() {
            let mut formatted = "0.0".to_string();
            if item != &0.0 {
                formatted = item.to_string();
            }
            json_vec.push(formatted);
            if pos != items.len() - 1 {
                json_vec.push(SYMBOL.comma.to_string());
            }
        }
        json_vec.push(SYMBOL.closing_square_bracket.to_string());

        let result = json_vec.join(SYMBOL.empty_string);
        Ok(result)
    }
}
