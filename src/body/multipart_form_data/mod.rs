#[cfg(test)]
mod tests;

use std::io;
use std::io::{BufRead, Cursor};
use file_ext::FileExt;
use crate::header::Header;
use crate::symbol::SYMBOL;

pub struct FormMultipartData;

pub struct Part {
    pub headers: Vec<Header>,
    pub body: Vec<u8>,
}

impl FormMultipartData {
    pub fn parse(data: &[u8], boundary: String) -> Result<Vec<Part>, String> {
        let parts = vec![];

        let cursor = io::Cursor::new(data);
        let bytes_read = 0;
        let total_bytes = data.len();

        FormMultipartData::
        read_line(
            cursor,
            boundary,
            bytes_read,
            total_bytes
        ).unwrap();


        Ok(parts)
    }

    //TODO: wip
    fn read_line(mut cursor: Cursor<&[u8]>, boundary: String, mut bytes_read: i32, total_bytes: usize) -> Result<(), String> {
        let mut buf = vec![];
        if bytes_read == 0 {
            let bytes_offset = cursor.read_until(b'\n', &mut buf).unwrap();
            let b : &[u8] = &buf;
            bytes_read = bytes_read + bytes_offset as i32;
            let boxed_line = String::from_utf8(Vec::from(b));
            if boxed_line.is_err() {
                let error_message = boxed_line.err().unwrap().to_string();
                return Err(error_message);
            }
            let string = boxed_line.unwrap();
            let string = string.replace(|x : char | x.is_ascii_control(), SYMBOL.empty_string).trim().to_string();
            let is_start_of_payload = string.starts_with(&boundary);

            FileExt::write_file("out.log", is_start_of_payload.to_string().as_bytes()).unwrap();
            buf = vec![];
        }


        let bytes_offset = cursor.read_until(b'\n', &mut buf).unwrap();
        FileExt::write_file("out.log", bytes_read.to_string().as_bytes()).unwrap();
        let b : &[u8] = &buf;
        bytes_read = bytes_read + bytes_offset as i32;

        FileExt::write_file("out.log", b).unwrap();
        if bytes_read == total_bytes as i32 {
            return Ok(())
        }

        FormMultipartData::read_line(cursor, boundary, bytes_read, total_bytes)
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