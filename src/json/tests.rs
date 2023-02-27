use std::io;
use std::io::{BufRead, Read};
use crate::json::{ToJSON, JSONProperty, JSONValue};
use crate::symbol::SYMBOL;

#[test]
fn parse() {
    struct SomeObject {
        prop_a: String,
        prop_b: bool
    }

    impl SomeObject {
        fn from_json_string(&self, json_string: String) -> Result<SomeObject, String> {
            let result = SomeObject { prop_a: "".to_string(), prop_b: false };

            let data = json_string.as_bytes();
            let mut cursor = io::Cursor::new(data);

            // read obj start '{'
            let mut buf = vec![];
            let boxed_read = cursor.read_until(b'{', &mut buf);
            if boxed_read.is_err() {
                let message = boxed_read.err().unwrap().to_string();
                return Err(message);
            }
            let b : &[u8] = &buf;

            let boxed_line = String::from_utf8(Vec::from(b));
            if boxed_line.is_err() {
                let error_message = boxed_line.err().unwrap().to_string();
                return Err(error_message);
            }
            let line = boxed_line.unwrap();

            let mut key_value_pair : String = "".to_string();

            // read until key starts '"', save to buffer
            let mut buf = vec![];
            let boxed_read = cursor.read_until(b'\"', &mut buf);
            if boxed_read.is_err() {
                let message = boxed_read.err().unwrap().to_string();
                return Err(message);
            }
            let b : &[u8] = &buf;

            let boxed_line = String::from_utf8(Vec::from(b));
            if boxed_line.is_err() {
                let error_message = boxed_line.err().unwrap().to_string();
                return Err(error_message);
            }

            let line = boxed_line.unwrap();
            key_value_pair = [key_value_pair, line].join(SYMBOL.empty_string);



            // read until key ends '"', append to buffer
            let mut buf = vec![];
            let boxed_read = cursor.read_until(b'\"', &mut buf);
            if boxed_read.is_err() {
                let message = boxed_read.err().unwrap().to_string();
                return Err(message);
            }
            let b : &[u8] = &buf;

            let boxed_line = String::from_utf8(Vec::from(b));
            if boxed_line.is_err() {
                let error_message = boxed_line.err().unwrap().to_string();
                return Err(error_message);
            }
            let line = boxed_line.unwrap();
            key_value_pair = [key_value_pair, line].join(SYMBOL.empty_string);


            // read until delimiter ':', append to buffer
            let mut buf = vec![];
            let boxed_read = cursor.read_until(b':', &mut buf);
            if boxed_read.is_err() {
                let message = boxed_read.err().unwrap().to_string();
                return Err(message);
            }
            let b : &[u8] = &buf;

            let boxed_line = String::from_utf8(Vec::from(b));
            if boxed_line.is_err() {
                let error_message = boxed_line.err().unwrap().to_string();
                return Err(error_message);
            }
            let line = boxed_line.unwrap();
            key_value_pair = [key_value_pair, line].join(SYMBOL.empty_string);

            // read in a while loop until char is not ascii control char and not whitespace, append to buffer
            let mut is_whitespace = true;
            let mut is_string = false;
            let mut is_number = false;
            let mut is_null = false;
            let mut is_boolean = false;

            while is_whitespace {
                let bytes_to_read = 1;
                let mut char_buffer = vec![0u8, bytes_to_read];

                cursor.read_exact(&mut char_buffer).unwrap();
                let char = String::from_utf8(char_buffer).unwrap();

                if char != " " {
                    if char == "\"" {
                        // read till non escaped '"'
                        is_string = true;
                    }

                    if char == "n" {
                        // read 'ull'
                        is_null = true;
                    }

                    if char == "t" {
                        // read 'rue'
                        is_boolean = true;
                    }

                    if char == "f" {
                        // read 'alse'
                        is_boolean = true;
                    }

                    if !is_string && !is_null && !is_boolean  {
                        // read till not number and decimal point, minus, exponent
                        is_number = true;
                    }

                    break;
                }


            }

            let mut buf = vec![];
            let boxed_read = cursor.read_until(b'"', &mut buf);
            if boxed_read.is_err() {
                let message = boxed_read.err().unwrap().to_string();
                return Err(message);
            }
            let b : &[u8] = &buf;

            let boxed_line = String::from_utf8(Vec::from(b));
            if boxed_line.is_err() {
                let error_message = boxed_line.err().unwrap().to_string();
                return Err(error_message);
            }
            let line = boxed_line.unwrap();
            key_value_pair = [key_value_pair, line].join(SYMBOL.empty_string);

            // read until value ends '"', append to buffer
            let mut buf = vec![];
            let boxed_read = cursor.read_until(b'"', &mut buf);
            if boxed_read.is_err() {
                let message = boxed_read.err().unwrap().to_string();
                return Err(message);
            }
            let b : &[u8] = &buf;

            let boxed_line = String::from_utf8(Vec::from(b));
            if boxed_line.is_err() {
                let error_message = boxed_line.err().unwrap().to_string();
                return Err(error_message);
            }
            let line = boxed_line.unwrap();
            key_value_pair = [key_value_pair, line].join(SYMBOL.empty_string);

            println!("{}", key_value_pair);


            // escape \r\n
            // read obj start '{'
            // read until key starts '"', save to buffer
            // read until key ends '"', append to buffer
            // read until delimiter ':', append to buffer
            // read in a while loop until char is not ascii control char and not whitespace, append to buffer
               // if char is '"' - mark as string
               // if char is 't' or 'f' mark as bool
               // if char is 'n' mark as null
            // read until value ends, append to buffer
               // if string it means char is '"' and last in buffer not the '\'
               // if bool it means t(true) f(false)
               // if null it means null
            // read until key-value pair delimiter ',' or end of the obj '}', parse key-value pair, in case of delimiter continue

            Ok(result)
        }
    }

    impl ToJSON for SomeObject {
        fn list_properties() -> Vec<JSONProperty> {
            let mut list = vec![];

            let property = JSONProperty { property_name: "prop_a".to_string(), property_type: "String".to_string() };
            list.push(property);

            let property = JSONProperty { property_name: "prop_b".to_string(), property_type: "bool".to_string() };
            list.push(property);

            list
        }

        fn get_property(&self, property_name: String) -> JSONValue {
            let mut value = JSONValue {
                f64: None,
                i128: None,
                String: None,
                bool: None,
                null: None,
            };

            if property_name == "prop_a".to_string() {
                let string : String = self.prop_a.to_owned();
                value.String = Some(string);
            }

            if property_name == "prop_b".to_string() {
                let boolean : bool = self.prop_b;
                value.bool = Some(boolean);
            }

            value
        }

        fn to_json_string(&self) -> String {
            let mut json_list = vec![];
            json_list.push(SYMBOL.opening_curly_bracket.to_string());


            let mut properties_list = vec![];

            let properties = SomeObject::list_properties();
            for property in properties {
                let value = self.get_property(property.property_name.to_string());

                if &property.property_type == "String" {
                    let raw_value = value.String.unwrap();
                    let formatted_property = format!("  \"{}\": \"{}\"", &property.property_name, raw_value);
                    properties_list.push(formatted_property.to_string());
                }

                if &property.property_type == "bool" {
                    let raw_value = value.bool.unwrap();
                    let formatted_property = format!("  \"{}\": {}", &property.property_name, raw_value);
                    properties_list.push(formatted_property.to_string());
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

    let obj = SomeObject { prop_a: "123abc".to_string(), prop_b: true };

    let json_string = obj.to_json_string();
    let expected_json_string = "{\r\n  \"prop_a\": \"123abc\",\r\n  \"prop_b\": true\r\n}";

    assert_eq!(expected_json_string, json_string);

    let parsed_json_object : SomeObject = obj.from_json_string(json_string.to_string()).unwrap();
}

