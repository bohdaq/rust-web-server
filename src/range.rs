use std::io::prelude::*;
use std::net::TcpStream;
use std::{env, fs, io};
use std::borrow::Borrow;
use std::char::MAX;
use std::fs::{File, metadata};
use std::io::{BufReader, SeekFrom};

use crate::request::Request;
use crate::response::Response;
use crate::app::App;
use crate::{CONSTANTS, Server};
use crate::constant::{HTTP_VERSIONS, REQUEST_METHODS, RESPONSE_STATUS_CODE_REASON_PHRASES};
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

    pub(crate) const MAX_BUFFER_LENGTH: u64 = 100000000; // 100 mb is max buffer size


    pub(crate) fn parse_range(filelength: u64, range_str: &str) -> Range {
        const START_INDEX: usize = 0;
        const END_INDEX: usize = 1;

        let mut range = Range { start: 0, end: filelength };
        let parts: Vec<&str> = range_str.split(CONSTANTS.HYPHEN).collect();
        for (i, part) in parts.iter().enumerate() {
            let num = part.trim();
            let length = num.len();
            if i == START_INDEX && length != 0 {
                range.start = num.parse().unwrap();
            }
            if i == END_INDEX && length != 0 {
                range.end = num.parse().unwrap();
            }
            if i == END_INDEX && length != 0 && range.start == 0 {
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

        println!("raw_range_value: {}", raw_range_value);
        let split_raw_range_value: Vec<&str> = raw_range_value.split(CONSTANTS.EQUALS).collect();
        let raw_bytes = split_raw_range_value.get(INDEX_AFTER_UNIT_DECLARATION).unwrap();
        println!("split_raw_range_value: {}", raw_bytes);

        let bytes: Vec<&str> = raw_bytes.split(CONSTANTS.COMMA).collect();
        for byte in bytes {
            let range = Range::parse_range(filelength, byte);
            let mut buff_length = range.end - range.start;
            if buff_length > MAX_BUFFER_LENGTH {
                buff_length = MAX_BUFFER_LENGTH;
            }

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
}


