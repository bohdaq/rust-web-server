use crate::json::array::RawUnprocessedJSONArray;
use crate::symbol::SYMBOL;

#[cfg(test)]
mod example_list_string_with_asserts;
#[cfg(test)]
mod example_list_string;

pub struct JSONArrayOfStrings;
impl JSONArrayOfStrings {
    pub fn parse_as_list_string(json : String) -> Result<Vec<String>, String> {
        let items = RawUnprocessedJSONArray::split_into_vector_of_strings(json).unwrap();
        let mut list: Vec<String> = vec![];
        for item in items {
            let boxed_parse = item.parse::<String>();
            let mut string: String = boxed_parse.unwrap().trim().to_string();
            let starts_with_quotation_mark = string.chars().next().unwrap() == '"';
            let ends_with_quotation_mark = string.chars().last().unwrap() == '"';
            if starts_with_quotation_mark && ends_with_quotation_mark {
                let number_of_characters = string.len() - 1;
                string = string[1..number_of_characters].to_string();
            } else {
                let message = format!("not a string: {}", item.to_string());
                return Err(message);
            }
            list.push(string);
        }
        Ok(list)
    }

    pub fn to_json_from_list_string(items : &Vec<String>) -> Result<String, String> {
        let mut json_vec = vec![];
        json_vec.push(SYMBOL.opening_square_bracket.to_string());
        for (pos, item) in items.iter().enumerate() {
            json_vec.push(SYMBOL.quotation_mark.to_string());
            json_vec.push(item.to_string());
            json_vec.push(SYMBOL.quotation_mark.to_string());
            if pos != items.len() - 1 {
                json_vec.push(SYMBOL.comma.to_string());
            }
        }
        json_vec.push(SYMBOL.closing_square_bracket.to_string());

        let result = json_vec.join(SYMBOL.empty_string);
        Ok(result)
    }

}
