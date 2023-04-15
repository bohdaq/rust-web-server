use std::io;
use std::io::{BufRead, Read};
use crate::ext::string_ext::StringExt;
use crate::json::property::{JSONProperty, JSONValue};
use crate::symbol::SYMBOL;

#[cfg(test)]
mod tests;

pub trait ToJSON {
    fn list_properties() -> Vec<JSONProperty>;
    fn get_property(&self, property_name: String) -> JSONValue;
    fn to_json_string(&self) -> String;
}

pub trait FromJSON {
    fn parse_json_to_properties(&self, json_string: String) -> Result<Vec<(JSONProperty, JSONValue)>, String>;
    fn set_properties(&mut self, properties: Vec<(JSONProperty, JSONValue)>) -> Result<(), String>;
    fn parse(&mut self, json_string: String) -> Result<(), String>;
}

pub struct JSON;

impl JSON {
    pub fn parse_as_properties(json_string: String) -> Result<Vec<(JSONProperty, JSONValue)>, String> {
        let mut properties = vec![];

        let data = json_string.as_bytes();
        let mut cursor = io::Cursor::new(data);
        let mut bytes_read : i128 = 0;
        let total_bytes : i128 = data.len() as i128;

        // read obj start '{'
        let mut _is_root_opening_curly_brace = true;
        let mut buf = vec![];
        let mut boxed_read = cursor.read_until(b'{', &mut buf);
        if boxed_read.is_err() {
            let error = boxed_read.err().unwrap().to_string();
            let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
            return Err(message);
        }
        bytes_read = bytes_read + boxed_read.unwrap() as i128;

        let mut b : &[u8] = &buf;

        let mut boxed_line = String::from_utf8(Vec::from(b));
        if boxed_line.is_err() {
            let error = boxed_line.err().unwrap().to_string();
            let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
            return Err(message);
        }
        let mut _line = boxed_line.unwrap();


        let mut is_there_a_key_value = true;
        while is_there_a_key_value {
            // read until key starts '"', save to buffer
            // it will work for first and consecutive key value pair
            let mut key_value_pair : String = "".to_string();


            buf = vec![];
            boxed_read = cursor.read_until(b'\"', &mut buf);
            if boxed_read.is_err() {
                let error = boxed_read.err().unwrap().to_string();
                let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                return Err(message);
            }
            bytes_read = bytes_read + boxed_read.unwrap() as i128;
            b  = &buf;

            boxed_line = String::from_utf8(Vec::from(b));
            if boxed_line.is_err() {
                let error = boxed_line.err().unwrap().to_string();
                let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                return Err(message);
            }

            _line = boxed_line.unwrap();
            let buffer_filtered_control_chars = StringExt::filter_ascii_control_characters(_line.as_str());
            if buffer_filtered_control_chars != "\"" {
                let message = format!("provided json is not valid");
                return Err(message);
            }

            key_value_pair = [key_value_pair, _line].join(SYMBOL.empty_string);



            // read until key ends '"', append to buffer
            buf = vec![];
            boxed_read = cursor.read_until(b'\"', &mut buf);
            if boxed_read.is_err() {
                let error = boxed_read.err().unwrap().to_string();
                let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                return Err(message);
            }
            bytes_read = bytes_read + boxed_read.unwrap() as i128;
            b = buf.as_slice();

            boxed_line = String::from_utf8(Vec::from(b));
            if boxed_line.is_err() {
                let error = boxed_line.err().unwrap().to_string();
                let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                return Err(message);
            }
            _line = boxed_line.unwrap();
            key_value_pair = [key_value_pair, _line].join(SYMBOL.empty_string);


            // read until delimiter ':', append to buffer
            let mut not_delimiter = true;
            while not_delimiter {
                let bytes_to_read = 1;
                let mut char_buffer = vec![bytes_to_read];

                let boxed_read = cursor.read_exact(&mut char_buffer);
                if boxed_read.is_err() {
                    let error = boxed_read.err().unwrap().to_string();
                    let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                    return Err(message);
                }
                boxed_read.unwrap();
                bytes_read = bytes_read + bytes_to_read as i128;
                let boxed_char = String::from_utf8(char_buffer);
                if boxed_char.is_err() {
                    let error = boxed_char.err().unwrap().to_string();
                    let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                    return Err(message);
                }

                let boxed_last_char = boxed_char.unwrap().chars().last();
                if boxed_last_char.is_none() {
                    let error = "last char is none (after ':')";
                    let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                    return Err(message);
                }
                let char = boxed_last_char.unwrap();

                if char != ' ' && char != '\n' && char != '\r' && !char.is_ascii_control() {
                    if char == ':' {
                        not_delimiter = false;
                        key_value_pair = [key_value_pair, char.to_string()].join(SYMBOL.empty_string);
                    } else {
                        let message = format!("while seeking for property delimiter ':', found unexpected character: {}", char);
                        return Err(message);
                    }
                }
            }



            // read in a while loop until char is not ascii control char and not whitespace, append to buffer
            let mut comma_delimiter_read_already = false;
            let mut is_whitespace_or_new_line_or_carriage_return = true;

            while is_whitespace_or_new_line_or_carriage_return {
                let bytes_to_read = 1;
                let mut char_buffer = vec![bytes_to_read];
                comma_delimiter_read_already = false;

                let boxed_read = cursor.read_exact(&mut char_buffer);
                if boxed_read.is_err() {
                    let error = boxed_read.err().unwrap().to_string();
                    let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                    return Err(message);
                }
                boxed_read.unwrap();
                bytes_read = bytes_read + bytes_to_read as i128;
                let boxed_char = String::from_utf8(char_buffer);
                if boxed_char.is_err() {
                    let error = boxed_char.err().unwrap().to_string();
                    let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                    return Err(message);
                }

                let boxed_last_char = boxed_char.unwrap().chars().last();
                if boxed_last_char.is_none() {
                    let error = "last char is none (after ':')";
                    let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                    return Err(message);
                }
                let char = boxed_last_char.unwrap();

                if char != ' ' && char != '\n' && char != '\r' && !char.is_ascii_control() {
                    // we passed opening curly brace at the beginning, at this point only nested objects can have '{'
                    _is_root_opening_curly_brace = false;


                    let is_string = char == '\"';
                    if is_string {
                        key_value_pair = [key_value_pair, char.to_string()].join(SYMBOL.empty_string);

                        // read till non escaped '"'
                        let mut not_end_of_string_property_value = true;
                        while not_end_of_string_property_value {

                            char_buffer = vec![bytes_to_read];
                            let boxed_read = cursor.read_exact(&mut char_buffer);
                            if boxed_read.is_err() {
                                let error = boxed_read.err().unwrap().to_string();
                                let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                                return Err(message);
                            }
                            boxed_read.unwrap();
                            bytes_read = bytes_read + bytes_to_read as i128;
                            let boxed_parse = String::from_utf8(char_buffer);
                            if boxed_parse.is_err() {
                                let error = boxed_parse.err().unwrap().to_string();
                                let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                                return Err(message);
                            }
                            let _char = boxed_parse.unwrap();
                            let last_char_in_buffer = key_value_pair.chars().last().unwrap().to_string();
                            not_end_of_string_property_value = _char != "\"" && last_char_in_buffer != "\\";
                            key_value_pair = [key_value_pair, _char].join(SYMBOL.empty_string);
                        }


                        // read till comma
                        buf = vec![];
                        let boxed_read = cursor.read_until(b',', &mut buf);
                        if boxed_read.is_err() {
                            let error = boxed_read.err().unwrap().to_string();
                            let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                            return Err(message);
                        }
                        bytes_read = bytes_read + boxed_read.unwrap() as i128;
                        if bytes_read == total_bytes {
                            is_there_a_key_value = false;
                        };

                        let boxed_parse = String::from_utf8(buf);
                        if boxed_parse.is_err() {
                            let message = boxed_parse.err().unwrap().to_string();
                            return Err(message);
                        }
                        let buffer_before_comma = boxed_parse.unwrap();
                        let buffer_filtered_control_chars = StringExt::filter_ascii_control_characters(buffer_before_comma.as_str());

                        if buffer_filtered_control_chars.chars().count() != 0 && buffer_filtered_control_chars != "}" && buffer_filtered_control_chars != "," {
                            let message = format!("there are not expected characters after number (expected comma): {}", buffer_before_comma);
                            return Err(message);
                        } else {
                            comma_delimiter_read_already = true;
                        }
                    }

                    let is_null = char == 'n';
                    if is_null {
                        // read 'ull'
                        key_value_pair = [key_value_pair, char.to_string()].join(SYMBOL.empty_string);
                        let byte = 0;
                        let mut char_buffer = vec![byte, byte, byte];
                        let length = char_buffer.len();
                        let boxed_read = cursor.read_exact(&mut char_buffer);
                        if boxed_read.is_err() {
                            let error = boxed_read.err().unwrap().to_string();
                            let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                            return Err(message);
                        }
                        boxed_read.unwrap();
                        bytes_read = bytes_read + length as i128;
                        let boxed_parse = String::from_utf8(char_buffer);
                        if boxed_parse.is_err() {
                            let error = boxed_parse.err().unwrap().to_string();
                            let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                            return Err(message);
                        }
                        let remaining_bool = boxed_parse.unwrap();
                        if remaining_bool != "ull" {
                            let error = format!("Unable to parse null: {}", key_value_pair);
                            let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                            return Err(message);
                        }
                        key_value_pair = [key_value_pair, remaining_bool].join(SYMBOL.empty_string);

                        // read till comma
                        buf = vec![];
                        let boxed_read = cursor.read_until(b',', &mut buf);
                        if boxed_read.is_err() {
                            let error = boxed_read.err().unwrap().to_string();
                            let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                            return Err(message);
                        }
                        bytes_read = bytes_read + boxed_read.unwrap() as i128;
                        if bytes_read == total_bytes {
                            is_there_a_key_value = false;
                        };

                        let boxed_parse = String::from_utf8(buf);
                        if boxed_parse.is_err() {
                            let message = boxed_parse.err().unwrap().to_string();
                            return Err(message)
                        }
                        let buffer_before_comma = boxed_parse.unwrap();
                        let buffer_filtered_control_chars = StringExt::filter_ascii_control_characters(buffer_before_comma.as_str());

                        if buffer_filtered_control_chars.chars().count() != 0 && buffer_filtered_control_chars != "}" && buffer_filtered_control_chars != "," {
                            let message = format!("before comma there are some unexpected characters: {}", buffer_before_comma);
                            return Err(message);
                        } else {
                            comma_delimiter_read_already = true;
                        }
                    }

                    let is_boolean_true = char == 't';
                    if is_boolean_true {
                        // read 'rue'
                        key_value_pair = [key_value_pair, char.to_string()].join(SYMBOL.empty_string);
                        let byte = 0;
                        let mut char_buffer = vec![byte, byte, byte];
                        let length = char_buffer.len();
                        let boxed_read = cursor.read_exact(&mut char_buffer);
                        if boxed_read.is_err() {
                            let error = boxed_read.err().unwrap().to_string();
                            let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                            return Err(message);
                        }
                        boxed_read.unwrap();
                        bytes_read = bytes_read + length as i128;
                        let boxed_parse = String::from_utf8(char_buffer);
                        if boxed_parse.is_err() {
                            let error = boxed_parse.err().unwrap().to_string();
                            let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                            return Err(message);
                        }
                        let remaining_bool = boxed_parse.unwrap();
                        if remaining_bool != "rue" {
                            let error = format!("Unable to parse boolean: {}", key_value_pair);
                            let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                            return Err(message);
                        }
                        key_value_pair = [key_value_pair, remaining_bool].join(SYMBOL.empty_string);

                        // read till comma
                        buf = vec![];
                        let boxed_read = cursor.read_until(b',', &mut buf);
                        if boxed_read.is_err() {
                            let error = boxed_read.err().unwrap().to_string();
                            let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                            return Err(message);
                        }
                        bytes_read = bytes_read + boxed_read.unwrap() as i128;
                        if bytes_read == total_bytes {
                            is_there_a_key_value = false;
                        };

                        let boxed_parse = String::from_utf8(buf);
                        if boxed_parse.is_err() {
                            let message = boxed_parse.err().unwrap().to_string();
                            return Err(message);
                        }
                        let buffer_before_comma = boxed_parse.unwrap();
                        let buffer_filtered_control_chars = StringExt::filter_ascii_control_characters(buffer_before_comma.as_str());

                        if buffer_filtered_control_chars.chars().count() != 0 && buffer_filtered_control_chars != "}" && buffer_filtered_control_chars != "," {
                            let message = format!("before comma there are some unexpected characters: {}", buffer_before_comma);
                            return Err(message);
                        } else {
                            comma_delimiter_read_already = true;
                        }
                    }

                    let is_boolean_false = char == 'f';
                    if is_boolean_false {
                        // read 'alse'
                        key_value_pair = [key_value_pair, char.to_string()].join(SYMBOL.empty_string);
                        let byte = 0;
                        let mut char_buffer = vec![byte, byte, byte, byte];
                        let length = char_buffer.len();
                        let boxed_read = cursor.read_exact(&mut char_buffer);
                        if boxed_read.is_err() {
                            let error = boxed_read.err().unwrap().to_string();
                            let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                            return Err(message);
                        }
                        boxed_read.unwrap();
                        bytes_read = bytes_read + length as i128;
                        let boxed_parse = String::from_utf8(char_buffer);

                        if boxed_parse.is_err() {
                            let error = boxed_parse.err().unwrap().to_string();
                            let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                            return Err(message);
                        }
                        let remaining_bool = boxed_parse.unwrap();
                        if remaining_bool != "alse" {
                            let message = format!("Unable to parse boolean: {}", key_value_pair);
                            return Err(message)
                        }
                        key_value_pair = [key_value_pair, remaining_bool].join(SYMBOL.empty_string);

                        // read till comma
                        buf = vec![];
                        let boxed_read = cursor.read_until(b',', &mut buf);
                        if boxed_read.is_err() {
                            let error = boxed_read.err().unwrap().to_string();
                            let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                            return Err(message);
                        }
                        bytes_read = bytes_read + boxed_read.unwrap() as i128;
                        if bytes_read == total_bytes {
                            is_there_a_key_value = false;
                        };

                        let boxed_parse = String::from_utf8(buf);
                        if boxed_parse.is_err() {
                            let message = boxed_parse.err().unwrap().to_string();
                            return Err(message);
                        }
                        let buffer_before_comma = boxed_parse.unwrap();
                        let buffer_filtered_control_chars = StringExt::filter_ascii_control_characters(buffer_before_comma.as_str());

                        if buffer_filtered_control_chars.chars().count() != 0 && buffer_filtered_control_chars != "}" && buffer_filtered_control_chars != "," {
                            let message = format!("before comma there are some unexpected characters: {}", buffer_before_comma);
                            return Err(message);
                        } else {
                            comma_delimiter_read_already = true;
                        }
                    }

                    let is_array = char == '[';
                    if is_array {
                        // read the array (including nested objects and arrays)
                        key_value_pair = [key_value_pair, char.to_string()].join(SYMBOL.empty_string);
                        let mut number_of_open_square_brackets = 1;
                        let mut number_of_closed_square_brackets = 0;

                        let mut read_char = true;
                        while read_char {

                            let byte = 0;
                            let mut char_buffer = vec![byte];
                            let length = char_buffer.len();
                            let boxed_read = cursor.read_exact(&mut char_buffer);
                            if boxed_read.is_err() {
                                let error = boxed_read.err().unwrap().to_string();
                                let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                                return Err(message);
                            }
                            boxed_read.unwrap();
                            bytes_read = bytes_read + length as i128;
                            let boxed_parse = String::from_utf8(char_buffer);
                            if boxed_parse.is_err() {
                                let error = boxed_parse.err().unwrap().to_string();
                                let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                                return Err(message);
                            }
                            let char = boxed_parse.unwrap().chars().last().unwrap();

                            let is_open_square_bracket = char == '[';
                            if is_open_square_bracket {
                                number_of_open_square_brackets = number_of_open_square_brackets + 1;
                            }


                            let is_close_square_bracket = char == ']';
                            if is_close_square_bracket {
                                number_of_closed_square_brackets = number_of_closed_square_brackets + 1;
                            }

                            key_value_pair = [key_value_pair, char.to_string()].join(SYMBOL.empty_string);

                            if number_of_open_square_brackets == number_of_closed_square_brackets {
                                read_char = false;
                            }
                        }

                        // read till comma
                        buf = vec![];
                        let boxed_read = cursor.read_until(b',', &mut buf);
                        if boxed_read.is_err() {
                            let error = boxed_read.err().unwrap().to_string();
                            let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                            return Err(message);
                        }
                        bytes_read = bytes_read + boxed_read.unwrap() as i128;
                        if bytes_read == total_bytes {
                            is_there_a_key_value = false;
                        };

                        let boxed_parse = String::from_utf8(buf);
                        if boxed_parse.is_err() {
                            let message = boxed_parse.err().unwrap().to_string();
                            return Err(message);
                        }
                        let buffer_before_comma = boxed_parse.unwrap();
                        let buffer_filtered_control_chars = StringExt::filter_ascii_control_characters(buffer_before_comma.as_str());

                        if buffer_filtered_control_chars.chars().count() != 0 && buffer_filtered_control_chars != "}" && buffer_filtered_control_chars != "," {
                            let message = format!("before comma there are some unexpected characters: {}", buffer_before_comma);
                            return Err(message);
                        } else {
                            comma_delimiter_read_already = true;
                        }
                    }


                    let is_nested_object = char == '{' && !_is_root_opening_curly_brace;
                    if is_nested_object {
                        // read the object (including nested objects and arrays)
                        key_value_pair = [key_value_pair, char.to_string()].join(SYMBOL.empty_string);
                        let mut number_of_open_curly_braces = 1;
                        let mut number_of_closed_curly_braces = 0;

                        let mut read_char = true;
                        while read_char {

                            let byte = 0;
                            let mut char_buffer = vec![byte];
                            let length = char_buffer.len();
                            let boxed_read = cursor.read_exact(&mut char_buffer);
                            if boxed_read.is_err() {
                                let error = boxed_read.err().unwrap().to_string();
                                let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                                return Err(message);
                            }
                            boxed_read.unwrap();
                            bytes_read = bytes_read + length as i128;
                            let boxed_parse = String::from_utf8(char_buffer);
                            if boxed_parse.is_err() {
                                let error = boxed_parse.err().unwrap().to_string();
                                let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                                return Err(message);
                            }
                            let boxed_last_char = boxed_parse.unwrap().chars().last();
                            if boxed_last_char.is_none() {
                                let error = "last char is empty";
                                let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                                return Err(message);
                            }
                            let char = boxed_last_char.unwrap();

                            let is_open_curly_brace = char == '{';
                            if is_open_curly_brace {
                                number_of_open_curly_braces = number_of_open_curly_braces + 1;
                            }


                            let is_close_curly_brace = char == '}';
                            if is_close_curly_brace {
                                number_of_closed_curly_braces = number_of_closed_curly_braces + 1;
                            }

                            key_value_pair = [key_value_pair, char.to_string()].join(SYMBOL.empty_string);

                            if number_of_open_curly_braces == number_of_closed_curly_braces {
                                read_char = false;
                            }
                        }
                        
                        // read till comma
                        buf = vec![];
                        let boxed_read = cursor.read_until(b',', &mut buf);
                        if boxed_read.is_err() {
                            let error = boxed_read.err().unwrap().to_string();
                            let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                            return Err(message);
                        }
                        bytes_read = bytes_read + boxed_read.unwrap() as i128;
                        if bytes_read == total_bytes {
                            is_there_a_key_value = false;
                        };

                        let boxed_parse = String::from_utf8(buf);
                        if boxed_parse.is_err() {
                            let message = boxed_parse.err().unwrap().to_string();
                            return Err(message);
                        }
                        let buffer_before_comma = boxed_parse.unwrap();
                        let buffer_filtered_control_chars = StringExt::filter_ascii_control_characters(buffer_before_comma.as_str());

                        if buffer_filtered_control_chars.chars().count() != 0 && buffer_filtered_control_chars != "}" && buffer_filtered_control_chars != "," {
                            let message = format!("before comma there are some unexpected characters: {}", buffer_before_comma);
                            return Err(message);
                        } else {
                            comma_delimiter_read_already = true;
                        }
                    }


                    let is_number = char.is_numeric();
                    if is_number {
                        // read until char is not number and decimal point, minus, exponent

                        key_value_pair = [key_value_pair, char.to_string()].join(SYMBOL.empty_string);

                        let mut _is_point_symbol_already_used = false;
                        let mut _is_exponent_symbol_already_used = false;
                        let mut _is_minus_symbol_already_used = false;

                        let mut read_char = true;
                        while read_char {

                            let byte = 0;
                            let mut char_buffer = vec![byte];
                            let length = char_buffer.len();
                            let boxed_read = cursor.read_exact(&mut char_buffer);
                            if boxed_read.is_err() {
                                let message = boxed_read.err().unwrap().to_string();
                                return Err(message);
                            }
                            bytes_read = bytes_read + length as i128;
                            let boxed_parse = String::from_utf8(char_buffer);
                            if boxed_parse.is_err() {
                                let error = boxed_parse.err().unwrap().to_string();
                                let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                                return Err(message);
                            }
                            let char = boxed_parse.unwrap().chars().last().unwrap();

                            let is_numeric = char.is_numeric();
                            let is_comma_symbol = char == ',';

                            let is_point_symbol = char == '.';
                            if is_point_symbol && _is_point_symbol_already_used {
                                _is_point_symbol_already_used = true;
                                let message = format!("unable to parse number: {}", key_value_pair);
                                return Err(message)
                            }

                            let is_exponent_symbol = char == 'e';
                            if is_exponent_symbol && _is_exponent_symbol_already_used {
                                _is_exponent_symbol_already_used = true;
                                let message = format!("unable to parse number: {}", key_value_pair);
                                return Err(message)
                            }

                            let is_minus_symbol = char == '-';
                            if is_minus_symbol && _is_minus_symbol_already_used {
                                _is_minus_symbol_already_used = true;
                                let message = format!("unable to parse number: {}", key_value_pair);
                                return Err(message)
                            }

                            let char_is_part_of_number = is_numeric || is_point_symbol || is_exponent_symbol || is_minus_symbol;
                            let is_delimiter = char == '\r' || char == '\n' || char == ' ';

                            if !is_delimiter {
                                if char_is_part_of_number {
                                    key_value_pair = [key_value_pair, char.to_string()].join(SYMBOL.empty_string);
                                } else {
                                    read_char = false;
                                    if char != '}' { // case where property is at the end of json object
                                        if is_comma_symbol {
                                            comma_delimiter_read_already = true;
                                        } else {
                                            let message = format!("there are not expected characters after number (expected comma): {} after {}", char,  key_value_pair);
                                            return Err(message);
                                        }
                                    }
                                }
                            }


                        }
                    }

                    let is_unknown_type =
                        !is_string &&
                            !is_null &&
                            !is_boolean_true &&
                            !is_boolean_false &&
                            !is_array &&
                            !is_number &&
                            !is_nested_object;
                    if is_unknown_type {
                        let message  = "provided json is not valid";
                        return Err(message.to_string());
                    }

                    is_whitespace_or_new_line_or_carriage_return = false;
                }


            }

            // attempt to read till comma, indicates presence of another key-value pair
            if !comma_delimiter_read_already {
                buf = vec![];
                boxed_read = cursor.read_until(b',', &mut buf);
                if boxed_read.is_err() {
                    let error = boxed_read.err().unwrap().to_string();
                    let message = format!("error at byte {} of {} bytes, message: {} ", bytes_read, total_bytes, error);
                    return Err(message);
                }
                bytes_read = bytes_read + boxed_read.unwrap() as i128;
                if bytes_read == total_bytes {
                    is_there_a_key_value = false;
                };
            }


            let boxed_parse = JSONProperty::parse(&key_value_pair);
            if boxed_parse.is_err() {
                let message = boxed_parse.err().unwrap().to_string();
                return Err(message);
            }
            let (property, value) = boxed_parse.unwrap();


            properties.push((property, value));

        }
        Ok(properties)
    }

    pub fn to_json_string(key_value_list: Vec<(JSONProperty, JSONValue)>) -> String {
        let mut json_list = vec![];
        json_list.push(SYMBOL.opening_curly_bracket.to_string());


        let mut properties_list = vec![];

        for (property, value) in key_value_list {

            if &property.property_type == "String" {
                if value.string.is_some() {
                    let raw_value = value.string.unwrap();
                    let formatted_property = format!("  \"{}\": \"{}\"", &property.property_name, raw_value);
                    properties_list.push(formatted_property.to_string());
                }
            }

            if &property.property_type == "bool" {
                if value.bool.is_some() {
                    let raw_value = value.bool.unwrap();
                    let formatted_property = format!("  \"{}\": {}", &property.property_name, raw_value);
                    properties_list.push(formatted_property.to_string());
                }
            }

            if &property.property_type == "i128" {
                if value.i128.is_some() {
                    let raw_value = value.i128.unwrap();
                    let formatted_property = format!("  \"{}\": {}", &property.property_name, raw_value);
                    properties_list.push(formatted_property.to_string());
                }
            }

            if &property.property_type == "f64" {
                if value.f64.is_some() {
                    let raw_value = value.f64.unwrap();
                    let mut _parsed_float = "0.0".to_string();
                    if raw_value != 0.0 {
                        _parsed_float = raw_value.to_string();
                    }
                    let formatted_property = format!("  \"{}\": {}", &property.property_name, _parsed_float);
                    properties_list.push(formatted_property.to_string());
                }
            }

            if &property.property_type == "object" {
                if value.object.is_some() {
                    let raw_value = value.object.unwrap();
                    let formatted_property = format!("  \"{}\": {}", &property.property_name, raw_value);
                    properties_list.push(formatted_property.to_string());
                }
            }

            if &property.property_type == "array" {
                if value.array.is_some() {
                    let raw_value = value.array.unwrap();
                    let formatted_property = format!("  \"{}\": {}", &property.property_name, raw_value);
                    properties_list.push(formatted_property.to_string());
                }
            }
        }


        let comma_new_line_carriage_return = format!("{}{}", SYMBOL.comma, SYMBOL.new_line_carriage_return);
        let properties = properties_list.join(&comma_new_line_carriage_return);

        json_list.push(properties);
        json_list.push(SYMBOL.closing_curly_bracket.to_string());
        let json= json_list.join(SYMBOL.new_line_carriage_return);
        json
    }
}
