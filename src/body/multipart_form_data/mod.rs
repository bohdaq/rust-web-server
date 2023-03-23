#[cfg(test)]
mod tests;

use std::io;
use std::io::{BufRead, Cursor};
use crate::ext::string_ext::StringExt;
use crate::header::Header;
use crate::symbol::SYMBOL;

pub struct FormMultipartData;

pub struct Part {
    pub headers: Vec<Header>,
    pub body: Vec<u8>,
}

impl Part {
    pub fn get_header(&self, name: String) -> Option<&Header> {
        let header =  self.headers.iter().find(|x| x.name.to_lowercase() == name.to_lowercase());
        header
    }
}

impl FormMultipartData {
    pub fn parse(data: &[u8], boundary: String) -> Result<Vec<Part>, String> {

        let cursor = io::Cursor::new(data);
        let bytes_read : i128 = 0;
        let total_bytes : i128 = data.len() as i128;

        let part_list : Vec<Part> = vec![];

        let boxed_part_list = FormMultipartData::
            parse_form_part_recursively(
                cursor,
                boundary,
                bytes_read,
                total_bytes,
                part_list
            );

        if boxed_part_list.is_err() {
            let message  = boxed_part_list.err().unwrap();
            return Err(message)
        }

        Ok(boxed_part_list.unwrap())
    }

    fn parse_form_part_recursively(
                mut cursor: Cursor<&[u8]>,
                boundary: String,
                mut bytes_read: i128,
                total_bytes: i128,
                mut part_list: Vec<Part>) -> Result<Vec<Part>, String> {
        let mut buf = vec![];
        let mut part = Part { headers: vec![], body: vec![] };

        // first boundary starts parsable payload
        if bytes_read == 0 {
            let boxed_read = cursor.read_until(b'\n', &mut buf);
            if boxed_read.is_err() {
                let message = boxed_read.err().unwrap().to_string();
                return Err(message);
            }
            let bytes_offset = boxed_read.unwrap();
            let b : &[u8] = &buf;
            bytes_read = bytes_read + bytes_offset as i128;

            let boxed_line = String::from_utf8(Vec::from(b));
            if boxed_line.is_err() {
                let error_message = boxed_line.err().unwrap().to_string();
                return Err(error_message);
            }
            let string = boxed_line.unwrap();
            let string = StringExt::filter_ascii_control_characters(&string);
            let string = StringExt::truncate_new_line_carriage_return(&string);

            let _current_string_is_boundary =
                string.replace(SYMBOL.hyphen, SYMBOL.empty_string)
                    .ends_with(&boundary.replace(SYMBOL.hyphen, SYMBOL.empty_string));

            if !_current_string_is_boundary {
                let message = format!("Body in multipart/form-data request needs to start with a boundary, actual string: '{}'", string);
                return Err(message.to_string())
            }
        }

        // headers part. by spec it shall have at least Content-Disposition header or more, following
        // by empty line. Headers shall be valid utf-8 encoded strings
        let mut current_string_is_empty = false;
        while !current_string_is_empty {
            buf = vec![];
            let boxed_read = cursor.read_until(b'\n', &mut buf);
            if boxed_read.is_err() {
                let message = boxed_read.err().unwrap().to_string();
                return Err(message);
            }
            let bytes_offset = boxed_read.unwrap();
            let b : &[u8] = &buf;
            bytes_read = bytes_read + bytes_offset as i128;

            let boxed_line = String::from_utf8(Vec::from(b));
            if boxed_line.is_err() {
                let error_message = boxed_line.err().unwrap().to_string();
                return Err(error_message);
            }
            let string = boxed_line.unwrap();

            let string = StringExt::filter_ascii_control_characters(&string);
            current_string_is_empty = string.trim().len() == 0;

            let _current_string_is_boundary =
                string.replace(SYMBOL.hyphen, SYMBOL.empty_string)
                    .ends_with(&boundary.replace(SYMBOL.hyphen, SYMBOL.empty_string));

            if _current_string_is_boundary {
                let message = "There is at least one missing body part in the multipart/form-data request";
                return Err(message.to_string())
            }

            if bytes_read == total_bytes as i128 {
                return Ok(part_list)
            }


            // multipart/form-data part does not have any header specified
            if current_string_is_empty && part.headers.len() == 0 {
                let message = "One of the body parts does not have any header specified. At least Content-Disposition is required";
                return Err(message.to_string());
            }

            if !current_string_is_empty {
                let boxed_header = Header::parse_header(&string);
                if boxed_header.is_err() {
                    let message = boxed_header.err().unwrap();
                    return Err(message)
                }

                let header = boxed_header.unwrap();
                part.headers.push(header);
            }
        }


        // multipart/form-data body part. it just arbitrary bytes. ends by delimiter.
        let mut _boundary_position = 0;
        let mut current_string_is_boundary = false;
        while !current_string_is_boundary {
            buf = vec![];

            let boxed_read = cursor.read_until(b'\n', &mut buf);
            if boxed_read.is_err() {
                let message = boxed_read.err().unwrap().to_string();
                return Err(message);
            }

            let bytes_offset = boxed_read.unwrap();

            if bytes_offset == 0 { break };

            let b : &[u8] = &buf;

            bytes_read = bytes_read + bytes_offset as i128;

            let escaped_dash_boundary = boundary.replace(SYMBOL.hyphen, SYMBOL.empty_string);

            current_string_is_boundary = false;
            if b.len() >= escaped_dash_boundary.len() {
                let boxed_sequence = FormMultipartData::find_subsequence(b, escaped_dash_boundary.as_bytes());
                if boxed_sequence.is_some() {
                    current_string_is_boundary = true;
                    _boundary_position = boxed_sequence.unwrap();
                }
            }

            if !current_string_is_boundary {
                part.body.append(&mut buf.clone());
            }

        }

        if !current_string_is_boundary && bytes_read == total_bytes as i128 {
            let message = "No end boundary present in the multipart/form-data request body";
            return Err(message.to_string());
        }

        // body for specific part may end with a new line or carriage return and a new line
        // in both cases new line carriage return delimiter is not part of the body
        let body_length = part.body.len();
        if body_length > 2 { // check if body itself is present
            let is_new_line_carriage_return_ending =
                *part.body.get(body_length-2).unwrap() == b'\r'
                    && *part.body.get(body_length-1).unwrap() == b'\n';

            let is_new_line_ending =
                *part.body.get(body_length-2).unwrap() != b'\r'
                    && *part.body.get(body_length-1).unwrap() == b'\n';

            if is_new_line_carriage_return_ending {
                part.body.remove(body_length - 1); // removing \n
                part.body.remove(body_length - 2); // removing \r
            }

            if is_new_line_ending {
                part.body.remove(body_length - 1); // removing \n
            }
        }



        part_list.push(part);


        if bytes_read == total_bytes as i128 {
            return Ok(part_list)
        }

        FormMultipartData::parse_form_part_recursively(cursor, boundary, bytes_read, total_bytes, part_list)
    }

    pub fn extract_boundary(content_type: &str) -> Result<String, String> {
        let boxed_split = content_type.split_once("boundary=");
        if boxed_split.is_none() {
            let message = "No boundary found in Content-Type header";
            return Err(message.to_string())
        }


        let (_, boundary) = boxed_split.unwrap();
        Ok(boundary.to_string())
    }

    fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|window| window == needle)
    }

    pub fn generate_part(part: Part) -> Result<Vec<u8>, String> {
        if part.headers.len() == 0 {
            let message = "One of the body parts does not have any header specified. At least Content-Disposition is required";
            return Err(message.to_string())
        }

        let mut formatted_header_list : String = "".to_string();
        for header in part.headers.into_iter() {
            let formatted = format!("{}{}", header.as_string(), SYMBOL.new_line_carriage_return.to_string());
            formatted_header_list = [formatted_header_list, formatted].join(SYMBOL.empty_string);
        }

        let header_body_delimiter = SYMBOL.new_line_carriage_return.to_string();

        let body = part.body;

        let part = [
            formatted_header_list.as_bytes().to_vec(),
            header_body_delimiter.as_bytes().to_vec(),
            body
        ].join(SYMBOL.empty_string.as_bytes());

        Ok(part)
    }

    pub fn generate(part_list: Vec<Part>, boundary: &str) -> Result<Vec<u8>, String> {
        if part_list.len() == 0 {
            let message = "List of the multipart/form-data request body parts is empty";
            return Err(message.to_string());
        }

        let mut bytes = vec![];
        bytes.push(boundary.as_bytes().to_vec());

        for part in part_list.into_iter() {
            let boxed_part_as_bytes = FormMultipartData::generate_part(part);
            if boxed_part_as_bytes.is_err() {
                let message = boxed_part_as_bytes.err().unwrap();
                return Err(message);
            }
            let part_as_bytes = boxed_part_as_bytes.unwrap();
            bytes.push(part_as_bytes);
            bytes.push(boundary.as_bytes().to_vec());
        }

        let result = bytes.join(SYMBOL.new_line_carriage_return.as_bytes());

        Ok(result)
    }
}