use std::num::ParseIntError;
use crate::json::array::RawUnprocessedJSONArray;
use crate::symbol::SYMBOL;

#[cfg(test)]
mod example_list_i128;
#[cfg(test)]
mod example_list_i128_with_asserts;
#[cfg(test)]
mod example_list_i64_with_asserts;
#[cfg(test)]
mod example_list_i64;
#[cfg(test)]
mod example_list_i32_with_asserts;
#[cfg(test)]
mod example_list_i32;
#[cfg(test)]
mod example_list_i16_with_asserts;
#[cfg(test)]
mod example_list_i16;
#[cfg(test)]
mod example_list_i8_with_asserts;
#[cfg(test)]
mod example_list_i8;
#[cfg(test)]
mod example_list_u128_with_asserts;
#[cfg(test)]
mod example_list_u128;
#[cfg(test)]
mod example_list_u64_with_asserts;
#[cfg(test)]
mod example_list_u64;
#[cfg(test)]
mod example_list_u32_with_asserts;
#[cfg(test)]
mod example_list_u32;
#[cfg(test)]
mod example_list_u16_with_asserts;
#[cfg(test)]
mod example_list_u16;
#[cfg(test)]
mod example_list_u8_with_asserts;
#[cfg(test)]
mod example_list_u8;

pub struct JSONArrayOfIntegers;
impl JSONArrayOfIntegers {
    pub fn parse_as_list_i128(json : String) -> Result<Vec<i128>, String> {
        let items = RawUnprocessedJSONArray::split_into_vector_of_strings(json).unwrap();
        let mut list: Vec<i128> = vec![];
        for item in items {
            let boxed_parse = item.parse::<i128>();
            if boxed_parse.is_err() {
                let message = boxed_parse.err().unwrap().to_string();
                return Err(message);
            }
            let num : i128 = boxed_parse.unwrap();
            list.push(num);
        }
        Ok(list)
    }

    pub fn to_json_from_list_i128(items : &Vec<i128>) -> Result<String, String> {
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

    pub fn parse_as_list_i64(json : String) -> Result<Vec<i64>, String> {
        let items = RawUnprocessedJSONArray::split_into_vector_of_strings(json).unwrap();
        let mut list: Vec<i64> = vec![];
        for item in items {
            let boxed_parse = item.parse::<i64>();
            if boxed_parse.is_err() {
                let message = boxed_parse.err().unwrap().to_string();
                return Err(message);
            }
            let num : i64 = boxed_parse.unwrap();
            list.push(num);
        }
        Ok(list)
    }

    pub fn to_json_from_list_i64(items : &Vec<i64>) -> Result<String, String> {
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

    pub fn parse_as_list_i32(json : String) -> Result<Vec<i32>, String> {
        let items = RawUnprocessedJSONArray::split_into_vector_of_strings(json).unwrap();
        let mut list: Vec<i32> = vec![];
        for item in items {
            let boxed_parse = item.parse::<i32>();
            if boxed_parse.is_err() {
                let message = boxed_parse.err().unwrap().to_string();
                return Err(message);
            }
            let num : i32 = boxed_parse.unwrap();
            list.push(num);
        }
        Ok(list)
    }

    pub fn to_json_from_list_i32(items : &Vec<i32>) -> Result<String, String> {
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

    pub fn parse_as_list_i16(json : String) -> Result<Vec<i16>, String> {
        let items = RawUnprocessedJSONArray::split_into_vector_of_strings(json).unwrap();
        let mut list: Vec<i16> = vec![];
        for item in items {
            let boxed_parse = item.parse::<i16>();
            if boxed_parse.is_err() {
                let message = boxed_parse.err().unwrap().to_string();
                return Err(message);
            }
            let num : i16 = boxed_parse.unwrap();
            list.push(num);
        }
        Ok(list)
    }

    pub fn to_json_from_list_i16(items : &Vec<i16>) -> Result<String, String> {
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

    pub fn parse_as_list_i8(json : String) -> Result<Vec<i8>, String> {
        let items = RawUnprocessedJSONArray::split_into_vector_of_strings(json).unwrap();
        let mut list: Vec<i8> = vec![];
        for item in items {
            let boxed_parse = item.parse::<i8>();
            if boxed_parse.is_err() {
                let message = boxed_parse.err().unwrap().to_string();
                return Err(message);
            }
            let num : i8 = boxed_parse.unwrap();
            list.push(num);
        }
        Ok(list)
    }

    pub fn to_json_from_list_i8(items : &Vec<i8>) -> Result<String, String> {
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

    pub fn parse_as_list_u128(json : String) -> Result<Vec<u128>, String> {
        let items = RawUnprocessedJSONArray::split_into_vector_of_strings(json).unwrap();
        let mut list: Vec<u128> = vec![];
        for item in items {
            let boxed_parse = item.parse::<u128>();
            if boxed_parse.is_err() {
                let message = boxed_parse.err().unwrap().to_string();
                return Err(message);
            }
            let num : u128 = boxed_parse.unwrap();
            list.push(num);
        }
        Ok(list)
    }

    pub fn to_json_from_list_u128(items : &Vec<u128>) -> Result<String, String> {
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

    pub fn parse_as_list_u64(json : String) -> Result<Vec<u64>, String> {
        let items = RawUnprocessedJSONArray::split_into_vector_of_strings(json).unwrap();
        let mut list: Vec<u64> = vec![];
        for item in items {
            let boxed_parse : Result<u64, ParseIntError> = item.parse();
            if boxed_parse.is_err() {
                let message = boxed_parse.err().unwrap().to_string();
                return Err(message);
            }
            let num : u64 = boxed_parse.unwrap();
            list.push(num);
        }
        Ok(list)
    }

    pub fn to_json_from_list_u64(items : &Vec<u64>) -> Result<String, String> {
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

    pub fn parse_as_list_u32(json : String) -> Result<Vec<u32>, String> {
        let items = RawUnprocessedJSONArray::split_into_vector_of_strings(json).unwrap();
        let mut list: Vec<u32> = vec![];
        for item in items {
            let boxed_parse : Result<u32, ParseIntError> = item.parse();
            if boxed_parse.is_err() {
                let message = boxed_parse.err().unwrap().to_string();
                return Err(message);
            }
            let num : u32 = boxed_parse.unwrap();
            list.push(num);
        }
        Ok(list)
    }

    pub fn to_json_from_list_u32(items : &Vec<u32>) -> Result<String, String> {
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

    pub fn parse_as_list_u16(json : String) -> Result<Vec<u16>, String> {
        let items = RawUnprocessedJSONArray::split_into_vector_of_strings(json).unwrap();
        let mut list: Vec<u16> = vec![];
        for item in items {
            let boxed_parse : Result<u16, ParseIntError> = item.parse();
            if boxed_parse.is_err() {
                let message = boxed_parse.err().unwrap().to_string();
                return Err(message);
            }
            let num : u16 = boxed_parse.unwrap();
            list.push(num);
        }
        Ok(list)
    }

    pub fn to_json_from_list_u16(items : &Vec<u16>) -> Result<String, String> {
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

    pub fn parse_as_list_u8(json : String) -> Result<Vec<u8>, String> {
        let items = RawUnprocessedJSONArray::split_into_vector_of_strings(json).unwrap();
        let mut list: Vec<u8> = vec![];
        for item in items {
            let boxed_parse : Result<u8, ParseIntError> = item.parse();
            if boxed_parse.is_err() {
                let message = boxed_parse.err().unwrap().to_string();
                return Err(message);
            }
            let num : u8 = boxed_parse.unwrap();
            list.push(num);
        }
        Ok(list)
    }

    pub fn to_json_from_list_u8(items : &Vec<u8>) -> Result<String, String> {
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
