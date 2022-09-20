#[cfg(test)]
mod tests;

use std::io;
use std::io::{BufRead, Cursor, Read};
use crate::header::Header;
use regex::Regex;
use crate::constant::{CONSTANTS, REQUEST_METHODS};
use crate::range::{ContentRange, Range};
use crate::{Request, Server};

pub struct Response {
    pub(crate) http_version: String,
    pub(crate) status_code: String,
    pub(crate) reason_phrase: String,
    pub(crate) headers: Vec<Header>,
    pub(crate) content_range_list: Vec<ContentRange>
}

impl Response {

    pub(crate) fn get_header(&self, name: String) -> Option<&Header> {
        let header =  self.headers.iter().find(|x| x.name == name);
        header
    }

    pub(crate) fn generate_body(content_range_list: Vec<ContentRange>) -> Vec<u8> {
        let mut body = vec![];
        let one = 1;

        if content_range_list.len() == one {
            let index = 0;
            let content_range = content_range_list.get(index).unwrap();
            body = content_range.body.to_vec();
        }

        if content_range_list.len() > one {
            for (i, content_range) in content_range_list.iter().enumerate() {
                let mut body_str = CONSTANTS.empty_string.to_string();
                if i != 0 {
                    body_str.push_str(CONSTANTS.new_line_separator);
                }
                body_str.push_str(CONSTANTS.separator);
                body_str.push_str(CONSTANTS.new_line_separator);
                let content_type = [Header::CONTENT_TYPE, CONSTANTS.header_name_value_separator, CONSTANTS.whitespace, &content_range.content_type.to_string()].join("");
                body_str.push_str(content_type.as_str());
                body_str.push_str(CONSTANTS.new_line_separator);
                let content_range_header = [Header::CONTENT_RANGE, CONSTANTS.header_name_value_separator, CONSTANTS.whitespace, CONSTANTS.bytes, CONSTANTS.whitespace, &content_range.range.start.to_string(), CONSTANTS.hyphen, &content_range.range.end.to_string(), CONSTANTS.slash, &content_range.size].join("");
                body_str.push_str(content_range_header.as_str());
                body_str.push_str(CONSTANTS.new_line_separator);
                body_str.push_str(CONSTANTS.new_line_separator);

                let inner_body = [body_str.as_bytes(), &content_range.body].concat();
                body = [body, inner_body].concat();
            }
            let mut trailing_separator = CONSTANTS.empty_string.to_string();
            trailing_separator.push_str(CONSTANTS.new_line_separator);
            trailing_separator.push_str(CONSTANTS.separator);
            body = [&body, trailing_separator.as_bytes()].concat();
        }

        body
    }

    pub(crate) fn generate_response(mut response: Response, request: Request) -> Vec<u8> {
        let mut headers = vec![
            Response::get_x_content_type_options_header(),
            Response::get_accept_ranges_header(),
        ];

        headers.append(&mut response.headers);

        if response.content_range_list.len() == 1 {
            let content_range_index = 0;
            let content_range = response.content_range_list.get(content_range_index).unwrap();
            headers.push(Header {
                name: Header::CONTENT_TYPE.to_string(),
                value: content_range.content_type.to_string()
            });

            let content_range_header_value = [
                CONSTANTS.bytes,
                CONSTANTS.whitespace,
                &content_range.range.start.to_string(),
                CONSTANTS.hyphen,
                &content_range.range.end.to_string(),
                CONSTANTS.slash,
                &content_range.size
            ].join("");
            headers.push(Header {
                name: Header::CONTENT_RANGE.to_string(),
                value: content_range_header_value.to_string()
            });

            headers.push(Header {
                name: Header::CONTENT_LENGTH.to_string(),
                value: content_range.body.len().to_string()
            });
        }

        if response.content_range_list.len() > 1 {
            let content_range_header_value = [
                CONSTANTS.multipart,
                CONSTANTS.slash,
                CONSTANTS.byteranges,
                CONSTANTS.semicolon,
                CONSTANTS.whitespace,
                CONSTANTS.boundary,
                CONSTANTS.equals,
                CONSTANTS.string_separator
            ].join("");
            headers.push(Header {
                name: Header::CONTENT_TYPE.to_string(),
                value: content_range_header_value,
            });
        }

        let body = Response::generate_body(response.content_range_list);

        let mut headers_str = CONSTANTS.new_line_separator.to_string();
        for header in headers {
            let mut header_string = CONSTANTS.empty_string.to_string();
            header_string.push_str(&header.name);
            header_string.push_str(CONSTANTS.header_name_value_separator);
            header_string.push_str(&header.value);
            header_string.push_str(CONSTANTS.new_line_separator);
            headers_str.push_str(&header_string);
        }
        let status = [response.http_version, response.status_code, response.reason_phrase].join(CONSTANTS.whitespace);
        let response_without_body = format!(
            "{}{}{}",
            status,
            headers_str,
            CONSTANTS.new_line_separator,
        );

        let is_head = request.method == REQUEST_METHODS.head;
        let is_options = request.method == REQUEST_METHODS.options;

        return if is_head || is_options {
            response_without_body.into_bytes()
        } else {
            [response_without_body.into_bytes(), body].concat()
        }

    }

    pub(crate) fn parse_response(response_vec_u8: &[u8]) -> Response {
        let mut cursor = io::Cursor::new(response_vec_u8);

        let mut response = Response {
            http_version: "".to_string(),
            status_code: "".to_string(),
            reason_phrase: "".to_string(),
            headers: vec![],
            content_range_list: vec![],
        };

        let content_length: usize = 0;
        let iteration_number : usize = 0;

        Response::parse_raw_response_via_cursor(&mut cursor, iteration_number, &mut response, content_length);

        return response;
    }

    pub(crate)  fn parse_http_version_status_code_reason_phrase_string(http_version_status_code_reason_phrase: &str) -> (String, String, String) {
        let re = Regex::new(CONSTANTS.http_version_and_status_code_and_reason_phrase_regex).unwrap();
        let caps = re.captures(&http_version_status_code_reason_phrase).unwrap();

        let http_version= String::from(&caps["http_version"]);
        let status_code = String::from(&caps["status_code"]);
        let mut reason_phrase = String::from(&caps["reason_phrase"]);
        reason_phrase = Server::truncate_new_line_carriage_return(&reason_phrase);

        return (http_version, status_code, reason_phrase)
    }

    pub(crate)  fn parse_http_response_header_string(header_string: &str) -> Header {
        let header_parts: Vec<&str> = header_string.split(CONSTANTS.header_name_value_separator).collect();
        let raw_header_name = header_parts[0].to_string();
        let header_name = Server::truncate_new_line_carriage_return(&raw_header_name);
        let raw_header_value = header_parts[1].to_string();
        let header_value = Server::truncate_new_line_carriage_return(&raw_header_value);


        Header {
            name: header_name.to_string(),
            value: header_value.to_string()
        }
    }

    pub(crate) fn parse_raw_response_via_cursor(
        cursor: &mut Cursor<&[u8]>,
        mut iteration_number: usize,
        response: &mut Response,
        mut content_length: usize) {

        let mut buffer = vec![];
        let bytes_offset = cursor.read_until(b'\n', &mut buffer).unwrap();
        let mut buffer_as_u8_array: &[u8] = &buffer;
        let string = String::from_utf8(Vec::from(buffer_as_u8_array)).unwrap();

        let is_first_iteration = iteration_number == 0;
        let new_line_char_found = bytes_offset != 0;
        let current_string_is_empty = string.trim().len() == 0;

        if is_first_iteration {
            let (http_version, status_code, reason_phrase) = Response::parse_http_version_status_code_reason_phrase_string(&string);

            response.http_version = http_version;
            response.status_code = status_code;
            response.reason_phrase = reason_phrase;
        }

        if current_string_is_empty {
            let content_type = response.get_header(Header::CONTENT_TYPE.to_string()).unwrap();
            let is_multipart = Response::is_multipart_byteranges_content_type(&content_type);

            if is_multipart {
                let content_range_list : Vec<ContentRange> = vec![];

                let mut buf = vec![];
                cursor.read_until(b'\n', &mut buf).unwrap();
                let boxed_value = Range::parse_multipart_body(cursor, content_range_list);
                let mut range_list = vec![];
                if boxed_value.is_ok() {
                    range_list = boxed_value.unwrap();
                }
                response.content_range_list = range_list;
            } else {
                buffer = vec![];
                let boxed_read = cursor.read_to_end(&mut buffer);
                if boxed_read.is_ok() {
                    buffer_as_u8_array = &buffer;

                    let content_range = ContentRange {
                        unit: CONSTANTS.bytes.to_string(),
                        range: Range {
                            start: 0,
                            end: buffer_as_u8_array.len() as u64
                        },
                        size: buffer_as_u8_array.len().to_string(),
                        body: Vec::from(buffer_as_u8_array),
                        content_type: content_type.value.to_string()
                    };
                    response.content_range_list = vec![content_range];
                } else {
                    let reason = boxed_read.err().unwrap();
                    eprintln!("error reading file: {}", reason.to_string())
                }

            }

            return;
        }

        if new_line_char_found && !current_string_is_empty {
            let mut header = Header { name: "".to_string(), value: "".to_string() };
            if !is_first_iteration {
                header = Response::parse_http_response_header_string(&string);
                if header.name == Header::CONTENT_LENGTH {
                    content_length = header.value.parse().unwrap();
                }
            }

            response.headers.push(header);
            iteration_number += 1;
            Response::parse_raw_response_via_cursor(cursor, iteration_number, response, content_length);
        }
    }

    pub(crate) fn is_multipart_byteranges_content_type(content_type: &Header) -> bool {
        let multipart_byteranges = [CONSTANTS.multipart, CONSTANTS.slash, CONSTANTS.byteranges].join("");
        let is_multipart_byteranges = content_type.value.starts_with(&multipart_byteranges);
        is_multipart_byteranges
    }

    pub(crate) fn get_x_content_type_options_header() -> Header {
        Header {
            name: Header::X_CONTENT_TYPE_OPTIONS.to_string(),
            value: CONSTANTS.nosniff.to_string(),
        }
    }

    pub(crate) fn get_accept_ranges_header() -> Header {
        Header {
            name: Header::ACCEPT_RANGES.to_string(),
            value: CONSTANTS.bytes.to_string(),
        }
    }
}