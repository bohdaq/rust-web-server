use std::io::prelude::*;
use std::net::TcpStream;
use std::{env, fs, io};
use std::borrow::Borrow;
use std::char::MAX;
use std::fs::{File, metadata};
use std::io::{BufReader, Cursor, SeekFrom};
use std::os::macos::raw::stat;
use regex::Regex;

use crate::request::Request;
use crate::response::Response;
use crate::app::App;
use crate::{CONSTANTS, Server};
use crate::constant::{HTTP_HEADERS, HTTP_VERSIONS, REQUEST_METHODS, RESPONSE_STATUS_CODE_REASON_PHRASES};
use crate::header::Header;
use crate::mime_type::MimeType;


pub struct Range {
    pub(crate) start: u64,
    pub(crate) end: u64,
}

pub struct ContentRange {
    pub(crate) unit: String,
    pub(crate) range: Range,
    pub(crate) size: String,
    pub(crate) body: Vec<u8>,
    pub(crate) content_type: String,
}


impl Range {

    pub(crate) const CONTENT_RANGE_REGEX: &'static str = "bytes\\s(?P<start>\\d{1,})-(?P<end>\\d{1,})/(?P<size>\\d{1,})";


    pub(crate) fn parse_range(filelength: u64, range_str: &str) -> Range {
        const START_INDEX: usize = 0;
        const END_INDEX: usize = 1;

        let mut range = Range { start: 0, end: filelength };
        let parts: Vec<&str> = range_str.split(CONSTANTS.HYPHEN).collect();

        let mut start_range_not_provided = true;
        for (i, part) in parts.iter().enumerate() {

            let num = part.trim();
            let length = num.len();

            if i == START_INDEX && length != 0 {
                start_range_not_provided = false;
            }
            if i == START_INDEX && length != 0 {
                range.start = num.parse().unwrap();
            }
            if i == END_INDEX && length != 0 {
                range.end = num.parse().unwrap();
            }
            if i == END_INDEX && length != 0 && start_range_not_provided {
                let num_usize : u64 = num.parse().unwrap();
                range.start = filelength - num_usize;
                range.end = filelength;
            }

        }
        range
    }

    pub(crate) fn parse_content_range(filepath: &str, filelength: u64, raw_range_value: &str) -> Vec<ContentRange> {
        const INDEX_AFTER_UNIT_DECLARATION : usize = 1;
        let mut content_range_list: Vec<ContentRange> = vec![];

        let split_raw_range_value: Vec<&str> = raw_range_value.split(CONSTANTS.EQUALS).collect();
        let raw_bytes = split_raw_range_value.get(INDEX_AFTER_UNIT_DECLARATION).unwrap();

        let bytes: Vec<&str> = raw_bytes.split(CONSTANTS.COMMA).collect();
        for byte in bytes {
            let range = Range::parse_range(filelength, byte);
            let mut buff_length = (range.end - range.start) + 1;

            let mut file = File::open(filepath).unwrap();
            let mut reader = BufReader::new(file);

            reader.seek(SeekFrom::Start(range.start));
            let mut buffer = Vec::new();
            reader.take(buff_length).read_to_end(&mut buffer).expect("Unable to read");

            let content_type = MimeType::detect_mime_type(filepath);

            let content_range = ContentRange {
                unit: CONSTANTS.BYTES.to_string(),
                range,
                size: filelength.to_string(),
                body: buffer,
                content_type,
            };

            println!("unit: {} range: {} - {} size: {} body len: {} mime type: {}" , content_range.unit, content_range.range.start, content_range.range.end, content_range.size, content_range.body.len(), content_range.content_type);
            content_range_list.push(content_range);
        }
        content_range_list
    }

    pub(crate) fn get_content_range_list(request_uri: &str, range: &Header) -> Vec<ContentRange> {
        let mut content_range_list : Vec<ContentRange> = vec![];
        let static_filepath = Server::get_static_filepath(request_uri);

        let md = metadata(&static_filepath).unwrap();
        if md.is_file() {
            content_range_list = Range::parse_content_range(&static_filepath, md.len(), &range.header_value);
        }

        content_range_list
    }

    pub(crate) fn parse_multipart_body(cursor: &mut Cursor<&[u8]>, mut content_range_list: Vec<ContentRange>) -> Vec<ContentRange> {

        let mut buffer = Range::parse_line_as_bytes(cursor);
        let new_line_char_found = buffer.len() != 0;
        let mut string = Range::convert_bytes_array_to_string(buffer);

        println!("string: {}", string);

        if !new_line_char_found {
            return content_range_list
        };

        let mut content_range: ContentRange = ContentRange {
            unit: CONSTANTS.BYTES.to_string(),
            range: Range { start: 0, end: 0 },
            size: "".to_string(),
            body: vec![],
            content_type: "".to_string()
        };

        let content_range_is_not_parsed = content_range.body.len() == 0;
        if string.starts_with(CONSTANTS.SEPARATOR) && content_range_is_not_parsed {
            //read next line - Content-Type
            buffer = Range::parse_line_as_bytes(cursor);
            string = Range::convert_bytes_array_to_string(buffer);
        }

        let content_type_is_not_parsed = content_range.content_type.len() == 0;
        if string.starts_with(HTTP_HEADERS.CONTENT_TYPE) && content_type_is_not_parsed {
            let content_type = Response::parse_http_response_header_string(string.as_str());
            content_range.content_type = content_type.header_value.trim().to_string();

            //read next line - Content-Range
            buffer = Range::parse_line_as_bytes(cursor);
            string = Range::convert_bytes_array_to_string(buffer);
        }

        let content_range_is_not_parsed = content_range.size.len() == 0;
        if string.starts_with(HTTP_HEADERS.CONTENT_RANGE) && content_range_is_not_parsed {
            let content_range_header = Response::parse_http_response_header_string(string.as_str());

            let (size, start, end) = Range::parse_content_range_header_value(content_range_header.header_value);

            content_range.size = size;
            content_range.range.start = start;
            content_range.range.end = end;

            // read next line - empty line
            buffer = Range::parse_line_as_bytes(cursor);
            string = Range::convert_bytes_array_to_string(buffer);

            // read next line - separator between content ranges
            buffer = Range::parse_line_as_bytes(cursor);
            string = Range::convert_bytes_array_to_string(buffer);
        }

        let content_range_is_parsed = content_range.size.len() != 0;
        let content_type_is_parsed = content_range.content_type.len() != 0;
        if content_range_is_parsed && content_type_is_parsed {
            let mut body : Vec<u8> = vec![];
            body = [body, string.as_bytes().to_vec()].concat();

            let mut buf = Vec::from(string.as_bytes());
            while !buf.starts_with(CONSTANTS.SEPARATOR.as_bytes()) {
                buf = vec![];
                cursor.read_until(b'\n', &mut buf).unwrap();

                if !buf.starts_with(CONSTANTS.SEPARATOR.as_bytes()) {
                    body = [body, buf.to_vec()].concat();
                }
            }

            let mut debug_body : &[u8]  = &body;
            println!("content range body is {} length is {}", String::from_utf8(debug_body.to_vec()).unwrap(), debug_body.len());

            content_range.body = body;

            content_range_list.push(content_range);
        }

        println!("content_range_list length: {}", content_range_list.len());
        content_range_list = Range::parse_multipart_body(cursor, content_range_list);

        content_range_list
    }

    pub(crate)  fn parse_content_range_header_value(header_value: String) -> (String, u64, u64) {
        let re = Regex::new(Range::CONTENT_RANGE_REGEX).unwrap();
        let caps = re.captures(&header_value).unwrap();

        let start= &caps["start"];
        let end = &caps["end"];
        let size = &caps["size"];

        let size = size.to_string();
        let start = start.parse().unwrap();
        let end = end.parse().unwrap();

        (size, start, end)
    }

    pub(crate) fn parse_line_as_bytes(mut cursor: &mut Cursor<&[u8]>) -> Vec<u8> {
        let mut buffer = vec![];
        cursor.read_until(b'\n', &mut buffer).unwrap();
        buffer
    }

    pub(crate) fn convert_bytes_array_to_string(buffer: Vec<u8>) -> String {
        let mut b : &[u8] = &buffer;
        String::from_utf8(Vec::from(b)).unwrap()
    }
}


