use std::io;
use std::io::Read;
use crate::symbol::SYMBOL;

#[cfg(test)]
mod tests;

pub mod boolean;
pub mod string;
pub mod null;
pub mod float;
pub mod integer;
pub mod object;

pub struct RawUnprocessedJSONArray;
impl RawUnprocessedJSONArray {
    pub fn split_into_vector_of_strings(_json_string: String) -> Result<Vec<String>, String> {
        let mut list : Vec<String> = vec![];

        // cursor
        let mut is_end_of_json_string = false;
        let mut cursor = io::Cursor::new(_json_string.to_string());
        let mut bytes_read : i128 = 0;
        let total_bytes : i128 = _json_string.len() as i128;


        // read the start of the array

        let mut read_until_start_of_array = true;
        while read_until_start_of_array {

            if is_end_of_json_string {
                let message = format!("not proper end of the json array: {}", _json_string.to_string());
                return Err(message);
            }

            let byte = 0;
            let mut char_buffer = vec![byte];
            let length = char_buffer.len();
            let boxed_read = cursor.read_exact(&mut char_buffer);
            if boxed_read.is_err() {
                let message = boxed_read.err().unwrap().to_string();
                return Err(message);
            }
            boxed_read.unwrap();
            bytes_read = bytes_read + length as i128;
            is_end_of_json_string = total_bytes == bytes_read;
            if is_end_of_json_string {
                let message = format!("not proper start of the json array: {}", _json_string.to_string());
                return Err(message);
            }
            let char = String::from_utf8(char_buffer).unwrap().chars().last().unwrap();

            if !char.is_whitespace() && char != '['{
                let message = format!("input string does not start with opening square bracket: {} in {}", char, _json_string);
                return Err(message);
            }

            if char == '[' {
                read_until_start_of_array = false;
            }
        }



        let mut is_end_of_array = false;
        let mut token;
        while !is_end_of_array {

            let byte = 0;
            let mut char_buffer = vec![byte];
            let length = char_buffer.len();
            let boxed_read = cursor.read_exact(&mut char_buffer);
            if boxed_read.is_err() {
                let message = boxed_read.err().unwrap().to_string();
                return Err(message);
            }
            boxed_read.unwrap();
            bytes_read = bytes_read + length as i128;
            let mut char = String::from_utf8(char_buffer).unwrap().chars().last().unwrap();

            if char == ']' {
                is_end_of_array = true;
            }

            if char != ' ' && char != ']' {
                let is_string = char == '\"';
                if is_string {
                    token = ["".to_string(), char.to_string()].join(SYMBOL.empty_string);

                    // read till non escaped '"'
                    let mut not_end_of_string_property_value = true;
                    while not_end_of_string_property_value {
                        let byte = 0;
                        char_buffer = vec![byte];
                        let boxed_read = cursor.read_exact(&mut char_buffer);
                        if boxed_read.is_err() {
                            let message = boxed_read.err().unwrap().to_string();
                            return Err(message);
                        }
                        boxed_read.unwrap();
                        let length = char_buffer.len();
                        bytes_read = bytes_read + length as i128;
                        let _char = String::from_utf8(char_buffer).unwrap();
                        let last_char_in_buffer = token.chars().last().unwrap().to_string();
                        not_end_of_string_property_value = _char != "\"" && last_char_in_buffer != "\\";
                        token = [token, _char.to_string()].join(SYMBOL.empty_string);
                    }
                    list.push(token.to_string());

                    // if char is whitespace read until non whitespace and check it is comma, if not return error
                    let is_whitespace = char == ' ';
                    if is_whitespace {
                        let mut read_till_end_of_whitespace = true;
                        while read_till_end_of_whitespace {
                            let byte = 0;
                            let mut char_buffer = vec![byte];
                            let length = char_buffer.len();
                            let boxed_read = cursor.read_exact(&mut char_buffer);
                            if boxed_read.is_err() {
                                let message = boxed_read.err().unwrap().to_string();
                                return Err(message);
                            }
                            boxed_read.unwrap();
                            bytes_read = bytes_read + length as i128;
                            char = String::from_utf8(char_buffer).unwrap().chars().last().unwrap();

                            if char == ',' {
                                read_till_end_of_whitespace = false
                            } else {
                                if char == ']' {
                                    read_till_end_of_whitespace = false;
                                    is_end_of_array = true;
                                } else {
                                    let message = format!("Missing comma between array items or closing square bracket at the end of array: {}", _json_string);
                                    return Err(message);
                                }
                            }
                        }
                    }
                }

                let is_null = char == 'n';
                if is_null {
                    // read 'ull'
                    token = ["".to_string(), char.to_string()].join(SYMBOL.empty_string);
                    let byte = 0;
                    let mut char_buffer = vec![byte, byte, byte];
                    let length = char_buffer.len();
                    let boxed_read = cursor.read_exact(&mut char_buffer);
                    if boxed_read.is_err() {
                        let message = boxed_read.err().unwrap().to_string();
                        return Err(message);
                    }
                    boxed_read.unwrap();
                    bytes_read = bytes_read + length as i128;
                    let remaining_bool = String::from_utf8(char_buffer).unwrap();
                    if remaining_bool != "ull" {
                        let message = format!("Unable to parse null: {} in {}", remaining_bool, _json_string);
                        return Err(message)
                    }
                    token = [token.to_string(), remaining_bool.to_string()].join(SYMBOL.empty_string);
                    list.push(token.to_string());
                }

                let is_boolean_true = char == 't';
                if is_boolean_true {
                    // read 'rue'
                    token = ["".to_string(), char.to_string()].join(SYMBOL.empty_string);
                    let byte = 0;
                    let mut char_buffer = vec![byte, byte, byte];
                    let length = char_buffer.len();
                    let boxed_read = cursor.read_exact(&mut char_buffer);
                    if boxed_read.is_err() {
                        let message = boxed_read.err().unwrap().to_string();
                        return Err(message);
                    }
                    boxed_read.unwrap();
                    bytes_read = bytes_read + length as i128;
                    let remaining_bool = String::from_utf8(char_buffer).unwrap();
                    if remaining_bool != "rue" {
                        let message = format!("Unable to parse true: {} in {}", remaining_bool, _json_string);
                        return Err(message)
                    }
                    token = [token.to_string(), remaining_bool.to_string()].join(SYMBOL.empty_string);
                    list.push(token.to_string());
                }

                let is_boolean_false = char == 'f';
                if is_boolean_false {
                    // read 'alse'
                    token = ["".to_string(), char.to_string()].join(SYMBOL.empty_string);
                    let byte = 0;
                    let mut char_buffer = vec![byte, byte, byte, byte];
                    let length = char_buffer.len();
                    let boxed_read = cursor.read_exact(&mut char_buffer);
                    if boxed_read.is_err() {
                        let message = boxed_read.err().unwrap().to_string();
                        return Err(message);
                    }
                    boxed_read.unwrap();
                    bytes_read = bytes_read + length as i128;
                    let remaining_bool = String::from_utf8(char_buffer).unwrap();
                    if remaining_bool != "alse" {
                        let message = format!("Unable to parse false: {} in {}", remaining_bool, _json_string);
                        return Err(message)
                    }
                    token = [token.to_string(), remaining_bool.to_string()].join(SYMBOL.empty_string);
                    list.push(token.to_string());
                }

                let is_array = char == '[';
                if is_array {
                    // read the array (including nested objects and arrays)
                    token = ["".to_string(), char.to_string()].join(SYMBOL.empty_string);
                    let mut number_of_open_square_brackets = 1;
                    let mut number_of_closed_square_brackets = 0;

                    let mut read_nested_array = true;
                    while read_nested_array {

                        let byte = 0;
                        let mut char_buffer = vec![byte];
                        let length = char_buffer.len();
                        let boxed_read = cursor.read_exact(&mut char_buffer);
                        if boxed_read.is_err() {
                            let message = boxed_read.err().unwrap().to_string();
                            return Err(message);
                        }
                        boxed_read.unwrap();
                        bytes_read = bytes_read + length as i128;
                        let char = String::from_utf8(char_buffer).unwrap().chars().last().unwrap();

                        let is_open_square_bracket = char == '[';
                        if is_open_square_bracket {
                            number_of_open_square_brackets = number_of_open_square_brackets + 1;
                        }


                        let is_close_square_bracket = char == ']';
                        if is_close_square_bracket {
                            number_of_closed_square_brackets = number_of_closed_square_brackets + 1;
                        }

                        token = [token.to_string(), char.to_string()].join(SYMBOL.empty_string);

                        if number_of_open_square_brackets == number_of_closed_square_brackets {
                            list.push(token.to_string());
                            read_nested_array = false;
                        }
                    }
                }


                let is_nested_object = char == '{';
                if is_nested_object {
                    // read the object (including nested objects and arrays)
                    token = ["".to_string(), char.to_string()].join(SYMBOL.empty_string);
                    let mut number_of_open_curly_braces = 1;
                    let mut number_of_closed_curly_braces = 0;

                    let mut read_nested_object = true;
                    while read_nested_object {

                        let byte = 0;
                        let mut char_buffer = vec![byte];
                        let length = char_buffer.len();
                        let boxed_read = cursor.read_exact(&mut char_buffer);
                        if boxed_read.is_err() {
                            let message = boxed_read.err().unwrap().to_string();
                            return Err(message);
                        }
                        boxed_read.unwrap();
                        bytes_read = bytes_read + length as i128;
                        let char = String::from_utf8(char_buffer).unwrap().chars().last().unwrap();

                        let is_open_curly_brace = char == '{';
                        if is_open_curly_brace {
                            number_of_open_curly_braces = number_of_open_curly_braces + 1;
                        }


                        let is_close_curly_brace = char == '}';
                        if is_close_curly_brace {
                            number_of_closed_curly_braces = number_of_closed_curly_braces + 1;
                        }

                        token = [token.to_string(), char.to_string()].join(SYMBOL.empty_string);

                        if number_of_open_curly_braces == number_of_closed_curly_braces {
                            list.push(token.to_string());
                            read_nested_object = false;
                        }
                    }
                }

                let mut is_comma_separator = char == ',';
                let is_numeric = char.is_numeric();
                let is_minus = char == '-';

                let is_number =
                    !is_string &&
                        !is_null &&
                        !is_boolean_true &&
                        !is_boolean_false &&
                        !is_array &&
                        !is_nested_object &&
                        !is_comma_separator &&
                        (is_numeric || is_minus);
                if is_number {
                    token = "".to_string();
                    // read until char is not number and decimal point, minus, exponent
                    if char != ',' {
                        token = ["".to_string(), char.to_string()].join(SYMBOL.empty_string);
                    }

                    let mut _is_point_symbol_already_used = false;
                    let mut _is_exponent_symbol_already_used = false;
                    let mut _is_minus_symbol_already_used = false;
                    if char == '-' {
                        _is_minus_symbol_already_used = true;
                    }

                    let mut read_number = true;
                    while read_number {

                        let byte = 0;
                        let mut char_buffer = vec![byte];
                        let length = char_buffer.len();
                        let boxed_read = cursor.read_exact(&mut char_buffer);
                        if boxed_read.is_err() {
                            let message = boxed_read.err().unwrap().to_string();
                            return Err(message);
                        }
                        boxed_read.unwrap();
                        bytes_read = bytes_read + length as i128;
                        char = String::from_utf8(char_buffer).unwrap().chars().last().unwrap();

                        let is_numeric = char.is_numeric();

                        let is_point_symbol = char == '.';
                        if is_point_symbol && _is_point_symbol_already_used {
                            _is_point_symbol_already_used = true;
                            let message = format!("unable to parse number: {} in {}", token, _json_string);
                            return Err(message)
                        }
                        if is_point_symbol {
                            _is_point_symbol_already_used = true;
                        }

                        let is_exponent_symbol = char == 'e';
                        if is_exponent_symbol && _is_exponent_symbol_already_used {
                            _is_exponent_symbol_already_used = true;
                            let message = format!("unable to parse number: {} in {}", token, _json_string);
                            return Err(message)
                        }
                        if is_exponent_symbol {
                            _is_exponent_symbol_already_used = true;
                        }

                        let is_minus_symbol = char == '-';
                        if is_minus_symbol && _is_minus_symbol_already_used {
                            _is_minus_symbol_already_used = true;
                            let message = format!("unable to parse number: {} in {}", token, _json_string);
                            return Err(message)
                        }

                        let char_is_part_of_number = is_numeric || is_point_symbol || is_exponent_symbol || is_minus_symbol;
                        // if char is whitespace read until non whitespace and check it is comma, if not return error
                        let is_whitespace = char == ' ';
                        if is_whitespace {
                            let mut read_till_end_of_whitespace = true;
                            while read_till_end_of_whitespace {
                                let byte = 0;
                                let mut char_buffer = vec![byte];
                                let length = char_buffer.len();
                                let boxed_read = cursor.read_exact(&mut char_buffer);
                                if boxed_read.is_err() {
                                    let message = boxed_read.err().unwrap().to_string();
                                    return Err(message);
                                }
                                boxed_read.unwrap();
                                bytes_read = bytes_read + length as i128;
                                char = String::from_utf8(char_buffer).unwrap().chars().last().unwrap();

                                if char == ',' {
                                    read_till_end_of_whitespace = false
                                } else {
                                    if char == ']' {
                                        read_till_end_of_whitespace = false;
                                        is_end_of_array = true;
                                    } else {
                                        let message = format!("Missing comma between array items or closing square bracket at the end of array: {}", _json_string);
                                        return Err(message);
                                    }
                                }
                            }
                        }

                        if char_is_part_of_number {
                            token = [token, char.to_string()].join(SYMBOL.empty_string);
                        } else {
                            read_number = false;
                            // if char is not array element separator or end of the array
                            if char != ',' && char != ']' {
                                let message = format!("unable to parse number: {} in {}", char, _json_string);
                                return Err(message)
                            }

                        }
                    }
                    list.push(token.to_string());
                }

                is_comma_separator = char == ',';
                let is_ascii_control = char.is_ascii_control();
                let is_carriage_return = char == '\r';
                let is_newline = char == '\n';
                let is_not_supported_type =
                    !is_string &&
                        !is_null &&
                        !is_boolean_true &&
                        !is_boolean_false &&
                        !is_array &&
                        !is_nested_object &&
                        !is_comma_separator &&
                        !is_carriage_return &&
                        !is_newline &&
                        !is_ascii_control &&
                        !is_numeric;
                if is_not_supported_type {
                    let message = format!("unknown type: {} in {}", char, _json_string);
                    return Err(message);
                }

            }


            if !is_end_of_array {
                is_end_of_array = char == ']';
            }

        }


        is_end_of_json_string = total_bytes == bytes_read;
        let mut read_after_end_of_array = !is_end_of_json_string;
        while read_after_end_of_array {

            let byte = 0;
            let mut char_buffer = vec![byte];
            let length = char_buffer.len();
            let boxed_read = cursor.read_exact(&mut char_buffer);
            if boxed_read.is_err() {
                let message = boxed_read.err().unwrap().to_string();
                return Err(message);
            }
            boxed_read.unwrap();
            bytes_read = bytes_read + length as i128;
            let char = String::from_utf8(char_buffer).unwrap().chars().last().unwrap();

            if !char.is_whitespace(){
                let message = format!("after array there are some characters: {} in {}", char, _json_string);
                return Err(message);
            }

            if bytes_read == total_bytes {
                read_after_end_of_array = false;
            }
        }

        Ok(list)
    }
}











