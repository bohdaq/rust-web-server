use crate::core::New;
use crate::json::array::RawUnprocessedJSONArray;
use crate::json::object::{FromJSON, ToJSON};
use crate::symbol::SYMBOL;

#[cfg(test)]
mod example_multi_nested_object;
#[cfg(test)]
mod example_multi_nested_object_with_asserts;

pub struct JSONArrayOfObjects<T> {
    _item: T, // added to eliminate compiler error
}

impl<T: New> JSONArrayOfObjects<T> {
    pub fn new() -> T {
        T::new()
    }
}

impl<T: ToJSON> JSONArrayOfObjects<T> {
    pub fn to_json(items : &Vec<T>) -> Result<String, String> {
        let mut json_vec = vec![];
        json_vec.push(SYMBOL.opening_square_bracket.to_string());
        for (pos, item) in items.iter().enumerate() {
            json_vec.push(item.to_json_string());
            if pos != items.len() - 1 {
                json_vec.push(SYMBOL.comma.to_string());
                json_vec.push(SYMBOL.new_line_carriage_return.to_string());
            }
        }
        json_vec.push(SYMBOL.closing_square_bracket.to_string());

        let result = json_vec.join(SYMBOL.empty_string);
        Ok(result)
    }
}

impl<T: FromJSON + New> JSONArrayOfObjects<T> {
    pub fn from_json(json : String) -> Result<Vec<T>, String> {
        let items = RawUnprocessedJSONArray::split_into_vector_of_strings(json).unwrap();
        let mut list: Vec<T> = vec![];
        for item in items {
            let mut object = T::new();
            object.parse(item).unwrap();
            list.push(object);
        }
        Ok(list)
    }
}
