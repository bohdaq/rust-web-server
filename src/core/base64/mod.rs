
use crate::symbol::SYMBOL;

#[cfg(test)]
mod tests;

pub struct Base64;

impl Base64 {
    //WIP
    pub fn encode(bytes: &[u8]) -> Result<String, String> {


        let mut result : Vec<String> = vec![];

        //encode in 3 bytes sequence
        let boxed_encode = Base64::encode_sequence(bytes);
        if boxed_encode.is_err() {
            return Err(boxed_encode.err().unwrap());
        }

        let encoded = boxed_encode.unwrap();
        result.push(encoded);

        let encoded_string = result.join(SYMBOL.empty_string);
        Ok(encoded_string)
    }

    pub fn decode(_text: String) -> Vec<u8> {
        vec![]
    }

    pub fn encode_sequence(bytes: &[u8]) -> Result<String, String> {
        if bytes.len() > 3 {
            return Err("sequence encodes at most 3 bytes at once".to_string());
        }

        if bytes.len() == 0 {
            return Err("sequence encodes at least 1 bytes".to_string());
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

        Ok("".to_string())
    }

    pub fn convert_number_to_base64_char(number: u8) -> Result<char, String> {
        if number > 63 {
            return Err("number exceeds range 0 - 63".to_string());
        }

        let mut base64_table : Vec<char> = vec![];

        let mut uppercase = ('A'..'Z').into_iter().collect::<Vec<char>>();
        base64_table.append(&mut uppercase);

        let mut lowercase = ('a'..'z').into_iter().collect::<Vec<char>>();
        base64_table.append(&mut lowercase);

        let mut numbers = ('0'..'9').into_iter().collect::<Vec<char>>();
        base64_table.append(&mut numbers);

        base64_table.push('+');
        base64_table.push('/');

        let boxed_get : Option<char> = base64_table.get(number as usize).copied();
        if boxed_get.is_none() {
            return Err("unknown error".to_string())
        }

        let char: char = boxed_get.unwrap();
        Ok(char)
    }
}