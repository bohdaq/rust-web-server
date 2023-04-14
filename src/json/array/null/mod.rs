use crate::json::array::RawUnprocessedJSONArray;
use crate::null::Null;
use crate::symbol::SYMBOL;

#[cfg(test)]
mod example_list_null_with_asserts;
#[cfg(test)]
mod example_list_null;

// hard to think about real use case
pub struct JSONArrayOfNulls;
impl JSONArrayOfNulls {
    pub fn parse_as_list_null(json : String) -> Result<Vec<Null>, String> {
        let items = RawUnprocessedJSONArray::split_into_vector_of_strings(json).unwrap();
        let mut list: Vec<Null> = vec![];
        for item in items {
            let boxed_parse = item.parse::<Null>();
            if boxed_parse.is_err() {
                let message = boxed_parse.err().unwrap().message;
                return Err(message);
            }
            let num : Null = boxed_parse.unwrap();
            list.push(num);
        }
        Ok(list)
    }

    pub fn to_json_from_list_null(items : &Vec<&Null>) -> Result<String, String> {
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
