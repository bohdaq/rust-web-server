#[cfg(test)]
mod tests;

use std::io;
use std::io::{BufRead, Cursor};
use file_ext::FileExt;
use crate::ext::string_ext::StringExt;
use crate::header::Header;

pub struct FormMultipartData;

pub struct Part {
    pub headers: Vec<Header>,
    pub body: Vec<u8>,
}

impl FormMultipartData {
    pub fn parse(data: &[u8], boundary: String) -> Result<Vec<Part>, String> {

        let cursor = io::Cursor::new(data);
        let bytes_read = 0;
        let total_bytes = data.len();


        let mut part_list : Vec<Part> = vec![];
        part_list = FormMultipartData::
            parse_form_part_recursively(
                cursor,
                boundary,
                bytes_read,
                total_bytes,
                part_list
            ).unwrap();


        Ok(part_list)
    }

    //TODO: wip
    fn parse_form_part_recursively(
        mut cursor: Cursor<&[u8]>,
        boundary: String,
        mut bytes_read: i32,
        total_bytes: usize,
        mut part_list: Vec<Part>) -> Result<Vec<Part>, String> {
        let mut buf = vec![];
        let mut part = Part { headers: vec![], body: vec![] };

        // first boundary starts parsable payload
        if bytes_read == 0 {
            let bytes_offset = cursor.read_until(b'\n', &mut buf).unwrap();
            let b : &[u8] = &buf;
            bytes_read = bytes_read + bytes_offset as i32;
            FileExt::write_file("out.log", "bytes_read".to_string().as_bytes()).unwrap();
            FileExt::write_file("out.log", bytes_read.to_string().as_bytes()).unwrap();

            let boxed_line = String::from_utf8(Vec::from(b));
            if boxed_line.is_err() {
                let error_message = boxed_line.err().unwrap().to_string();
                return Err(error_message);
            }
            let string = boxed_line.unwrap();
            let string = StringExt::filter_ascii_control_characters(&string);
            let is_start_of_payload = string.starts_with(&boundary);


            FileExt::write_file("out.log", is_start_of_payload.to_string().as_bytes()).unwrap();
            buf = vec![];
        }

        // headers part. by spec it shall have at least Content-Disposition header or more, following
        // by empty line. Headers shall be valid utf-8 encoded strings
        // TODO:
        let mut current_string_is_empty = false;
        while !current_string_is_empty {
            buf = vec![];
            let bytes_offset = cursor.read_until(b'\n', &mut buf).unwrap();
            let b : &[u8] = &buf;
            bytes_read = bytes_read + bytes_offset as i32;
            // FileExt::write_file("out.log", "bytes_read".to_string().as_bytes()).unwrap();
            // FileExt::write_file("out.log", bytes_read.to_string().as_bytes()).unwrap();

            let boxed_line = String::from_utf8(Vec::from(b));
            if boxed_line.is_err() {
                let error_message = boxed_line.err().unwrap().to_string();
                return Err(error_message);
            }
            let string = boxed_line.unwrap();
            FileExt::write_file("out.log", "string: ".to_string().as_bytes()).unwrap();
            FileExt::write_file("out.log", string.to_string().as_bytes()).unwrap();

            let string = StringExt::filter_ascii_control_characters(&string);
            let string = StringExt::truncate_new_line_carriage_return(&string);

            current_string_is_empty = string.trim().len() == 0;

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


        // body part. it just arbitrary bytes. ends by delimiter.
        // TODO:
        let mut body: Vec<u8> = vec![];
        let mut current_string_is_boundary = false;
        while !current_string_is_boundary {
            buf = vec![];
            let bytes_offset = cursor.read_until(b'\n', &mut buf).unwrap();

            if bytes_offset == 0 { break };

            let b : &[u8] = &buf;

            bytes_read = bytes_read + bytes_offset as i32;


            let boxed_line = String::from_utf8(Vec::from(b));
            if boxed_line.is_ok() {
                let string = boxed_line.unwrap();
                let string = StringExt::filter_ascii_control_characters(&string);
                current_string_is_boundary = string.starts_with(&boundary);

                if current_string_is_boundary {
                    let buffer_ref : &[u8] = &body;
                    let boxed_line = String::from_utf8(Vec::from(buffer_ref));
                    if boxed_line.is_err() {
                        let error_message = boxed_line.err().unwrap().to_string();
                        return Err(error_message);
                    }
                    let string = boxed_line.unwrap();
                    FileExt::write_file("out.log", &body).unwrap();
                    part.body.append(&mut body);
                    FileExt::write_file("out.log", "string: ".to_string().as_bytes()).unwrap();
                    FileExt::write_file("out.log", string.to_string().as_bytes()).unwrap();
                } else {
                    body.append(&mut buf.clone());
                }
            }

        }

        part_list.push(part);


        if bytes_read == total_bytes as i32 {
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
}