use std::io::prelude::*;
use std::fs::{File, metadata};
use std::io::{BufReader, Cursor, SeekFrom};
use regex::Regex;

use crate::response::Response;
use crate::{CONSTANTS, Server};
use crate::constant::{HTTPError, RESPONSE_STATUS_CODE_REASON_PHRASES};
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
    pub(crate) const ERROR_NO_EMPTY_LINE_BETWEEN_CONTENT_RANGE_HEADER_AND_BODY: &'static str = "no empty line between content range headers and body";
    pub(crate) const ERROR_UNABLE_TO_PARSE_CONTENT_RANGE: &'static str = "unable to parse content-range";

    pub(crate) const ERROR_START_IS_AFTER_END_CONTENT_RANGE: &'static str = "start is after end in content range";
    pub(crate) const ERROR_START_IS_BIGGER_THAN_FILESIZE_CONTENT_RANGE: &'static str = "start is bigger than filesize in content range";
    pub(crate) const ERROR_END_IS_BIGGER_THAN_FILESIZE_CONTENT_RANGE: &'static str = "end is bigger than filesize in content range";
    pub(crate) const ERROR_MALFORMED_RANGE_HEADER_WRONG_UNIT: &'static str = "range header malformed, most likely you have an error in unit statement";

    pub(crate) const ERROR_UNABLE_TO_PARSE_RANGE_START: &'static str = "unable to parse range start";
    pub(crate) const ERROR_UNABLE_TO_PARSE_RANGE_END: &'static str = "unable to parse range end";


    pub(crate) fn parse_range_in_content_range(filelength: u64, range_str: &str) -> Result<Range, HTTPError> {
        const START_INDEX: usize = 0;
        const END_INDEX: usize = 1;

        let mut range = Range { start: 0, end: filelength };
        let parts: Vec<&str> = range_str.split(CONSTANTS.hyphen).collect();

        let mut start_range_not_provided = true;
        for (i, part) in parts.iter().enumerate() {

            let num = part.trim();
            let length = num.len();

            if i == START_INDEX && length != 0 {
                start_range_not_provided = false;
            }
            if i == START_INDEX && length != 0 {
                let boxed_start  = num.parse();
                if boxed_start.is_ok() {
                    range.start = boxed_start.unwrap()
                } else {
                    let message = Range::ERROR_UNABLE_TO_PARSE_RANGE_START.to_string();
                    let error = HTTPError {
                        status_code_reason_phrase: RESPONSE_STATUS_CODE_REASON_PHRASES.n416_range_not_satisfiable,
                        message: message.to_string()
                    };
                    return Err(error)
                }
            }
            if i == END_INDEX && length != 0 {
                let boxed_end  = num.parse();
                if boxed_end.is_ok() {
                    range.end = boxed_end.unwrap()
                } else {
                    let message = Range::ERROR_UNABLE_TO_PARSE_RANGE_END.to_string();
                    let error = HTTPError {
                        status_code_reason_phrase: RESPONSE_STATUS_CODE_REASON_PHRASES.n416_range_not_satisfiable,
                        message: message.to_string()
                    };
                    return Err(error)
                }
            }
            if i == END_INDEX && length != 0 && start_range_not_provided {
                let num_usize : u64 = num.parse().unwrap();
                range.start = filelength - num_usize;
                range.end = filelength;
            }

            if range.end > filelength {
                let message = Range::ERROR_END_IS_BIGGER_THAN_FILESIZE_CONTENT_RANGE.to_string();
                let error = HTTPError {
                    status_code_reason_phrase: RESPONSE_STATUS_CODE_REASON_PHRASES.n416_range_not_satisfiable,
                    message: message,
                };
                return Err(error);
            }

            if range.start > filelength {
                let message = Range::ERROR_START_IS_BIGGER_THAN_FILESIZE_CONTENT_RANGE.to_string();
                let error = HTTPError {
                    status_code_reason_phrase: RESPONSE_STATUS_CODE_REASON_PHRASES.n416_range_not_satisfiable,
                    message: message,
                };
                return Err(error);
            }

            if range.start > range.end {
                let message = Range::ERROR_START_IS_AFTER_END_CONTENT_RANGE.to_string();
                let error = HTTPError {
                    status_code_reason_phrase: RESPONSE_STATUS_CODE_REASON_PHRASES.n416_range_not_satisfiable,
                    message: message,
                };
                return Err(error);
            }



        }
        Ok(range)
    }

    pub(crate) fn parse_content_range(filepath: &str, filelength: u64, raw_range_value: &str) -> Result<Vec<ContentRange>, HTTPError> {
        const INDEX_AFTER_UNIT_DECLARATION : usize = 1;
        let mut content_range_list: Vec<ContentRange> = vec![];

        let prefix = [CONSTANTS.bytes, CONSTANTS.equals].join("");
        if !raw_range_value.starts_with(prefix.as_str()) {
            let message = Range::ERROR_MALFORMED_RANGE_HEADER_WRONG_UNIT.to_string();
            let error = HTTPError {
                status_code_reason_phrase: RESPONSE_STATUS_CODE_REASON_PHRASES.n416_range_not_satisfiable,
                message: message,
            };
            return Err(error);
        }

        let split_raw_range_value: Vec<&str> = raw_range_value.split(CONSTANTS.equals).collect();
        let raw_bytes = split_raw_range_value.get(INDEX_AFTER_UNIT_DECLARATION).unwrap();

        let bytes: Vec<&str> = raw_bytes.split(CONSTANTS.comma).collect();
        for byte in bytes {
            let boxed_range = Range::parse_range_in_content_range(filelength, byte);
            if boxed_range.is_ok() {
                let range = boxed_range.unwrap();
                let mut buff_length = (range.end - range.start) + 1;

                let mut file = File::open(filepath).unwrap();
                let mut reader = BufReader::new(file);

                let boxed_seek = reader.seek(SeekFrom::Start(range.start));
                if boxed_seek.is_ok() {
                    let mut buffer = Vec::new();
                    reader.take(buff_length).read_to_end(&mut buffer).expect("Unable to read");

                    let content_type = MimeType::detect_mime_type(filepath);

                    let content_range = ContentRange {
                        unit: CONSTANTS.bytes.to_string(),
                        range,
                        size: filelength.to_string(),
                        body: buffer,
                        content_type,
                    };

                    content_range_list.push(content_range);
                } else {
                    let error : HTTPError = HTTPError {
                        status_code_reason_phrase:  RESPONSE_STATUS_CODE_REASON_PHRASES.n416_range_not_satisfiable,
                        message: boxed_seek.err().unwrap().to_string()
                    };
                    return Err(error)
                }

            } else {
                let error : HTTPError = boxed_range.err().unwrap();
                return Err(error);
            }
        }
        Ok(content_range_list)
    }

    pub(crate) fn get_content_range_list(request_uri: &str, range: &Header) -> Result<Vec<ContentRange>, HTTPError> {
        let mut content_range_list : Vec<ContentRange> = vec![];
        let static_filepath = Server::get_static_filepath(request_uri);

        let md = metadata(&static_filepath).unwrap();
        if md.is_file() {
            let boxed_content_range_list = Range::parse_content_range(&static_filepath, md.len(), &range.header_value);
            if boxed_content_range_list.is_ok() {
                content_range_list = boxed_content_range_list.unwrap();
            } else {
                let error = boxed_content_range_list.err().unwrap();
                return Err(error)
            }
        }

        Ok(content_range_list)
    }

    pub(crate) fn parse_multipart_body(cursor: &mut Cursor<&[u8]>, mut content_range_list: Vec<ContentRange>) -> Result<Vec<ContentRange>, String> {

        let mut buffer = Range::parse_line_as_bytes(cursor);
        let new_line_char_found = buffer.len() != 0;
        let mut string = Range::convert_bytes_array_to_string(buffer);

        if !new_line_char_found {
            return Ok(content_range_list)
        };

        let mut content_range: ContentRange = ContentRange {
            unit: CONSTANTS.bytes.to_string(),
            range: Range { start: 0, end: 0 },
            size: "".to_string(),
            body: vec![],
            content_type: "".to_string()
        };

        let content_range_is_not_parsed = content_range.body.len() == 0;
        if string.starts_with(CONSTANTS.separator) && content_range_is_not_parsed {
            //read next line - Content-Type
            buffer = Range::parse_line_as_bytes(cursor);
            string = Range::convert_bytes_array_to_string(buffer);
        }

        let content_type_is_not_parsed = content_range.content_type.len() == 0;
        if string.starts_with(Header::CONTENT_TYPE) && content_type_is_not_parsed {
            let content_type = Response::parse_http_response_header_string(string.as_str());
            content_range.content_type = content_type.header_value.trim().to_string();

            //read next line - Content-Range
            buffer = Range::parse_line_as_bytes(cursor);
            string = Range::convert_bytes_array_to_string(buffer);
        }

        let content_range_is_not_parsed = content_range.size.len() == 0;
        if string.starts_with(Header::CONTENT_RANGE) && content_range_is_not_parsed {
            let content_range_header = Response::parse_http_response_header_string(string.as_str());

            let boxed_result = Range::parse_content_range_header_value(content_range_header.header_value);
            if boxed_result.is_ok() {
                let (size, start, end) = boxed_result.unwrap();

                content_range.size = size;
                content_range.range.start = start;
                content_range.range.end = end;
            } else {
                return Err(boxed_result.err().unwrap())
            }



            // read next line - empty line
            buffer = Range::parse_line_as_bytes(cursor);
            string = Range::convert_bytes_array_to_string(buffer);

            if string.trim().len() > 0 {
                return Err(Range::ERROR_NO_EMPTY_LINE_BETWEEN_CONTENT_RANGE_HEADER_AND_BODY.to_string());
            }

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
            while !buf.starts_with(CONSTANTS.separator.as_bytes()) {
                buf = vec![];
                cursor.read_until(b'\n', &mut buf).unwrap();

                if !buf.starts_with(CONSTANTS.separator.as_bytes()) {
                    body = [body, buf.to_vec()].concat();
                }
            }

            let mut mutable_body : Vec<u8>  = body;
            mutable_body.pop(); // remove /r
            mutable_body.pop(); // remove /n


            content_range.body = mutable_body;

            content_range_list.push(content_range);
        }

        let boxed_result = Range::parse_multipart_body(cursor, content_range_list);
        return if boxed_result.is_ok() {
            Ok(boxed_result.unwrap())
        } else {
            let error = boxed_result.err().unwrap();
            Err(error)
        }

    }

    pub(crate)  fn parse_content_range_header_value(header_value: String) -> Result<(String, u64, u64), String> {
        let re = Regex::new(Range::CONTENT_RANGE_REGEX).unwrap();
        let boxed_caps = re.captures(&header_value);
        if boxed_caps.is_none() {
            return Err(Range::ERROR_UNABLE_TO_PARSE_CONTENT_RANGE.to_string())
        }

        let caps = boxed_caps.unwrap();

        let start= &caps["start"];
        let end = &caps["end"];
        let size = &caps["size"];

        let size = size.to_string();
        let start = start.parse().unwrap();
        let end = end.parse().unwrap();

        if start > end {
            return Err(Range::ERROR_START_IS_AFTER_END_CONTENT_RANGE.to_string())
        }

        let size_num: u64 = size.parse().unwrap();
        if start > size_num {
            return Err(Range::ERROR_START_IS_BIGGER_THAN_FILESIZE_CONTENT_RANGE.to_string());
        }
        if end > size_num {
            return  Err(Range::ERROR_END_IS_BIGGER_THAN_FILESIZE_CONTENT_RANGE.to_string());
        }

        Ok((size, start, end))
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


