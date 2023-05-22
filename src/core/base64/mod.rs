use std::collections::HashMap;
use crate::symbol::SYMBOL;

#[cfg(test)]
mod tests;

pub struct Base64;

//WIP
impl Base64 {

    pub fn encode(bytes: &[u8]) -> Result<String, String> {


        let mut result : Vec<String> = vec![];

        // WIP encode in 3 bytes sequence
        let boxed_encode = Base64::encode_sequence(bytes);
        if boxed_encode.is_err() {
            return Err(boxed_encode.err().unwrap());
        }

        let encoded = boxed_encode.unwrap();
        result.push(encoded);

        let encoded_string = result.join(SYMBOL.empty_string);
        Ok(encoded_string)
    }

    pub fn decode(_text: String) -> Result<Vec<u8>, String> {
        // WIP decode 4 chars at once
        Base64::decode_sequence(_text)
    }

    pub fn decode_sequence(text: String) -> Result<Vec<u8>, String> {
        let result : Vec<u8> = vec![];

        let number_of_equal_signs = text.matches(SYMBOL.equals).count();

        if number_of_equal_signs == 2 {
            let boxed_first_byte = text.chars().nth(0);
            if boxed_first_byte.is_none() {
                return Err("unexpected error, unable to get char at position 0".to_string());
            }
            let first_byte = boxed_first_byte.unwrap() as u8;
            let _first_byte_as_string = format!("{first_byte:b}");

            let converted_first_byte = Base64::convert_base64_char_to_number(first_byte as char).unwrap();
            let shifted_converted_first_byte = converted_first_byte << 2;
            let _shifted_converted_first_byte_as_string = format!("{converted_first_byte:b}");



            let boxed_second_byte = text.chars().nth(1);
            if boxed_second_byte.is_none() {
                return Err("unexpected error, unable to get char at position 0".to_string());
            }
            let second_byte = boxed_second_byte.unwrap() as u8;
            let _second_byte_as_string = format!("{second_byte:b}");

            let converted_second_byte = Base64::convert_base64_char_to_number(second_byte as char).unwrap();


            let shifted_converted_second_byte = converted_second_byte >> 4;
            let _shifted_second_byte_as_string = format!("{shifted_converted_second_byte:b}");


            let resulted_byte = shifted_converted_first_byte | shifted_converted_second_byte;
            return Ok(vec![resulted_byte]);

        }

        if number_of_equal_signs == 1 {
            let boxed_first_byte = text.chars().nth(0);
            if boxed_first_byte.is_none() {
                return Err("unexpected error, unable to get char at position 0".to_string());
            }
            let first_byte = boxed_first_byte.unwrap() as u8;
            let _first_byte_as_string = format!("{first_byte:b}");

            let converted_first_byte = Base64::convert_base64_char_to_number(first_byte as char).unwrap();
            let shifted_converted_first_byte = converted_first_byte << 2;
            let _shifted_converted_first_byte_as_string = format!("{converted_first_byte:b}");



            let boxed_second_byte = text.chars().nth(1);
            if boxed_second_byte.is_none() {
                return Err("unexpected error, unable to get char at position 1".to_string());
            }
            let second_byte = boxed_second_byte.unwrap() as u8;
            let _second_byte_as_string = format!("{second_byte:b}");

            let converted_second_byte = Base64::convert_base64_char_to_number(second_byte as char).unwrap();


            let shifted_converted_second_byte = converted_second_byte >> 4;
            let _shifted_second_byte_as_string = format!("{shifted_converted_second_byte:b}");


            let first_char_as_byte = shifted_converted_first_byte | shifted_converted_second_byte;




            // second char
            let second_char_part_one = (converted_second_byte & 0b00001111) << 4;
            let boxed_third_byte = text.chars().nth(2);
            if boxed_third_byte.is_none() {
                return Err("unexpected error, unable to get char at position 2".to_string());
            }
            let third_byte = boxed_third_byte.unwrap() as u8;
            let converted_third_byte = Base64::convert_base64_char_to_number(third_byte as char).unwrap();
            let shifted_third_byte = (0b00111100 & converted_third_byte)  >> 2;

            let second_char_as_byte = shifted_third_byte | second_char_part_one;

            return Ok(vec![first_char_as_byte, second_char_as_byte]);

        }

        Ok(result)
    }

    pub fn encode_sequence(bytes: &[u8]) -> Result<String, String> {
        if bytes.len() > 3 {
            return Err("sequence encodes at most 3 bytes at once".to_string());
        }

        if bytes.len() == 0 {
            return Err("sequence encodes at least 1 byte".to_string());
        }

        if bytes.len() == 1 {
            let boxed_byte = bytes.get(0);
            if boxed_byte.is_none() {
                return Err("byte at pos 1 is empty".to_string());
            }

            let byte = boxed_byte.unwrap();
            let _byte_as_string = format!("{byte:b}");
            let shifted_first_sextet = byte >> 2;
            let _shifted_first_sextet_as_string = format!("{shifted_first_sextet:b}");

            let shifted_second_sextet = (byte & 0b00000011) << 4;
            let _shifted_first_sextet_as_string = format!("{shifted_second_sextet:b}");

            let mut result_buffer: Vec<String> = vec![];

            let boxed_encoded_char = Base64::convert_number_to_base64_char(shifted_first_sextet);
            if boxed_encoded_char.is_err() {
                return Err(boxed_encoded_char.err().unwrap());
            }

            result_buffer.push(boxed_encoded_char.unwrap().to_string());

            let boxed_encoded_char = Base64::convert_number_to_base64_char(shifted_second_sextet);
            if boxed_encoded_char.is_err() {
                return Err(boxed_encoded_char.err().unwrap());
            }

            result_buffer.push(boxed_encoded_char.unwrap().to_string());

            result_buffer.push(SYMBOL.equals.to_string());
            result_buffer.push(SYMBOL.equals.to_string());

            let result : String = result_buffer.join(SYMBOL.empty_string);
            return Ok(result);
        }

        if bytes.len() == 2 {
            let boxed_byte = bytes.get(0);
            if boxed_byte.is_none() {
                return Err("byte at pos 1 is empty".to_string());
            }

            let byte = boxed_byte.unwrap();
            let _byte_as_string = format!("{byte:b}");
            let shifted_first_sextet = byte >> 2;
            let _shifted_first_sextet_as_string = format!("{shifted_first_sextet:b}");



            let mut result_buffer: Vec<String> = vec![];

            let boxed_encoded_char = Base64::convert_number_to_base64_char(shifted_first_sextet);
            if boxed_encoded_char.is_err() {
                return Err(boxed_encoded_char.err().unwrap());
            }

            let char : String =  boxed_encoded_char.unwrap().to_string();
            result_buffer.push(char);


            // base64 second sextet part 1 (from first u8)
            let shifted_second_sextet_part_one = (byte & 0b00000011) << 4;
            let _shifted_second_sextet_as_string = format!("{shifted_second_sextet_part_one:b}");


            // base64 second sextet part 2 (from second u8)
            let boxed_byte = bytes.get(1);
            if boxed_byte.is_none() {
                return Err("byte at pos 1 is empty".to_string());
            }

            let second_byte = boxed_byte.unwrap();
            let shifted_second_byte_part_two = second_byte >> 4;


            let second_sextet = shifted_second_sextet_part_one | shifted_second_byte_part_two;
            let boxed_second_encoded_char = Base64::convert_number_to_base64_char(second_sextet);
            if boxed_second_encoded_char.is_err() {
                return Err(boxed_second_encoded_char.err().unwrap());
            }

            let char : String =  boxed_second_encoded_char.unwrap().to_string();
            result_buffer.push(char);


            // base64 third char
            let base64_third_char = (second_byte & 0b00001111) << 2;
            let boxed_third_encoded_char = Base64::convert_number_to_base64_char(base64_third_char);
            if boxed_third_encoded_char.is_err() {
                return Err(boxed_third_encoded_char.err().unwrap());
            }
            let char : String =  boxed_third_encoded_char.unwrap().to_string();
            result_buffer.push(char);



            result_buffer.push(SYMBOL.equals.to_string());

            let result : String = result_buffer.join(SYMBOL.empty_string);
            return Ok(result);
        }

        Ok("".to_string())
    }

    pub fn convert_base64_char_to_number(_char: char) -> Result<u8, String> {
        let base64_char_list : Vec<char> = Base64::get_base64_char_list();
        let mut map : HashMap<char, u8> = HashMap::new();

        for (index, char) in base64_char_list.iter().enumerate() {
            map.insert(*char, index as u8);
        }

        let index = map.get(&_char).unwrap();

        Ok(*index)
    }

    pub fn get_base64_char_list() -> Vec<char> {
        let mut base64_table : Vec<char> = vec![];

        let mut uppercase = ('A'..'Z').into_iter().collect::<Vec<char>>();
        base64_table.append(&mut uppercase);

        let mut lowercase = ('a'..'z').into_iter().collect::<Vec<char>>();
        base64_table.append(&mut lowercase);

        let mut numbers = ('0'..'9').into_iter().collect::<Vec<char>>();
        base64_table.append(&mut numbers);

        base64_table.push('+');
        base64_table.push('/');

        base64_table
    }

    pub fn convert_number_to_base64_char(number: u8) -> Result<char, String> {
        if number > 63 {
            return Err("number exceeds range 0 - 63".to_string());
        }

        let base64_table : Vec<char> = Base64::get_base64_char_list();

        let boxed_get : Option<char> = base64_table.get(number as usize).copied();
        if boxed_get.is_none() {
            return Err("unknown error".to_string())
        }

        let char: char = boxed_get.unwrap();
        Ok(char)
    }
}