#[cfg(test)]
mod tests;

use std::io;
use std::io::{BufRead, Cursor, Read};
use crate::header::Header;
use regex::Regex;
use crate::range::{ContentRange, Range};
use crate::request::{METHOD, Request};
use crate::server::Server;
use crate::symbol::SYMBOL;

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Error {
    pub status_code_reason_phrase: &'static StatusCodeReasonPhrase,
    pub message: String,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Response {
    pub(crate) http_version: String,
    pub(crate) status_code: String,
    pub(crate) reason_phrase: String,
    pub(crate) headers: Vec<Header>,
    pub(crate) content_range_list: Vec<ContentRange>
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct StatusCodeReasonPhrase {
    pub(crate) status_code: &'static str,
    pub(crate) reason_phrase: &'static str,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ResponseStatusCodeReasonPhrase {
    pub(crate) n200_ok: &'static StatusCodeReasonPhrase,
    pub(crate) n204_no_content: &'static StatusCodeReasonPhrase,
    pub(crate) n206_partial_content: &'static StatusCodeReasonPhrase,
    pub(crate) n400_bad_request: &'static StatusCodeReasonPhrase,
    pub(crate) n404_not_found: &'static StatusCodeReasonPhrase,
    pub(crate) n416_range_not_satisfiable: &'static StatusCodeReasonPhrase,
}

pub const STATUS_CODE_REASON_PHRASE: ResponseStatusCodeReasonPhrase = ResponseStatusCodeReasonPhrase {
    n200_ok: &StatusCodeReasonPhrase {
        status_code: "200",
        reason_phrase: "OK"
    },

    n204_no_content: &StatusCodeReasonPhrase {
        status_code: "204",
        reason_phrase: "No Content"
    },

    n206_partial_content: &StatusCodeReasonPhrase {
        status_code: "206",
        reason_phrase: "Partial Content"
    },

    n400_bad_request: &StatusCodeReasonPhrase {
        status_code: "400",
        reason_phrase: "Bad Request"
    },

    n404_not_found: &StatusCodeReasonPhrase {
        status_code: "404",
        reason_phrase: "Not Found"
    },

    n416_range_not_satisfiable: &StatusCodeReasonPhrase {
        status_code: "416",
        reason_phrase: "Range Not Satisfiable"
    },

};

impl Response {

    pub const _HTTP_VERSION_AND_STATUS_CODE_AND_REASON_PHRASE_REGEX: &'static str = "(?P<http_version>\\w+/\\w+.\\w)\\s(?P<status_code>\\w+)\\s(?P<reason_phrase>.+)";

    pub(crate) fn _get_header(&self, name: String) -> Option<&Header> {
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
                let mut body_str = SYMBOL.empty_string.to_string();
                if i != 0 {
                    body_str.push_str(SYMBOL.new_line_carriage_return);
                }
                body_str.push_str(SYMBOL.hyphen);
                body_str.push_str(SYMBOL.hyphen);
                body_str.push_str(Range::STRING_SEPARATOR);
                body_str.push_str(SYMBOL.new_line_carriage_return);
                let content_type = [Header::CONTENT_TYPE, Header::NAME_VALUE_SEPARATOR, SYMBOL.whitespace, &content_range.content_type.to_string()].join("");
                body_str.push_str(content_type.as_str());
                body_str.push_str(SYMBOL.new_line_carriage_return);
                let content_range_header = [Header::CONTENT_RANGE, Header::NAME_VALUE_SEPARATOR, SYMBOL.whitespace, Range::BYTES, SYMBOL.whitespace, &content_range.range.start.to_string(), SYMBOL.hyphen, &content_range.range.end.to_string(), SYMBOL.slash, &content_range.size].join("");
                body_str.push_str(content_range_header.as_str());
                body_str.push_str(SYMBOL.new_line_carriage_return);
                body_str.push_str(SYMBOL.new_line_carriage_return);

                let inner_body = [body_str.as_bytes(), &content_range.body].concat();
                body = [body, inner_body].concat();
            }
            let mut trailing_separator = SYMBOL.empty_string.to_string();
            trailing_separator.push_str(SYMBOL.new_line_carriage_return);
            trailing_separator.push_str(SYMBOL.hyphen);
            trailing_separator.push_str(SYMBOL.hyphen);
            trailing_separator.push_str(Range::STRING_SEPARATOR);
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
                Range::BYTES,
                SYMBOL.whitespace,
                &content_range.range.start.to_string(),
                SYMBOL.hyphen,
                &content_range.range.end.to_string(),
                SYMBOL.slash,
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
                Range::MULTIPART,
                SYMBOL.slash,
                Range::BYTERANGES,
                SYMBOL.semicolon,
                SYMBOL.whitespace,
                Range::BOUNDARY,
                SYMBOL.equals,
                Range::STRING_SEPARATOR
            ].join("");
            headers.push(Header {
                name: Header::CONTENT_TYPE.to_string(),
                value: content_range_header_value,
            });
        }

        let body = Response::generate_body(response.content_range_list);

        let mut headers_str = SYMBOL.new_line_carriage_return.to_string();
        for header in headers {
            let mut header_string = SYMBOL.empty_string.to_string();
            header_string.push_str(&header.name);
            header_string.push_str(Header::NAME_VALUE_SEPARATOR);
            header_string.push_str(&header.value);
            header_string.push_str(SYMBOL.new_line_carriage_return);
            headers_str.push_str(&header_string);
        }
        let status = [response.http_version, response.status_code, response.reason_phrase].join(SYMBOL.whitespace);
        let response_without_body = format!(
            "{}{}{}",
            status,
            headers_str,
            SYMBOL.new_line_carriage_return,
        );

        let is_head = request.method == METHOD.head;
        let is_options = request.method == METHOD.options;

        return if is_head || is_options {
            response_without_body.into_bytes()
        } else {
            [response_without_body.into_bytes(), body].concat()
        }

    }

    pub(crate) fn _parse_response(response_vec_u8: &[u8]) -> Response {
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

        Response::_parse_raw_response_via_cursor(&mut cursor, iteration_number, &mut response, content_length);

        return response;
    }

    pub(crate)  fn _parse_http_version_status_code_reason_phrase_string(http_version_status_code_reason_phrase: &str) -> (String, String, String) {
        let re = Regex::new(Response::_HTTP_VERSION_AND_STATUS_CODE_AND_REASON_PHRASE_REGEX).unwrap();
        let caps = re.captures(&http_version_status_code_reason_phrase).unwrap();

        let http_version= String::from(&caps["http_version"]);
        let status_code = String::from(&caps["status_code"]);
        let mut reason_phrase = String::from(&caps["reason_phrase"]);
        reason_phrase = Server::truncate_new_line_carriage_return(&reason_phrase);

        return (http_version, status_code, reason_phrase)
    }

    pub(crate)  fn _parse_http_response_header_string(header_string: &str) -> Header {
        let header_parts: Vec<&str> = header_string.split(Header::NAME_VALUE_SEPARATOR).collect();
        let header_name = header_parts[0].to_string();
        let raw_header_value = header_parts[1].to_string();
        let header_value = Server::truncate_new_line_carriage_return(&raw_header_value);


        Header {
            name: header_name.to_string(),
            value: header_value.to_string()
        }
    }

    pub(crate) fn _parse_raw_response_via_cursor(
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
            let (http_version, status_code, reason_phrase) = Response::_parse_http_version_status_code_reason_phrase_string(&string);

            response.http_version = http_version;
            response.status_code = status_code;
            response.reason_phrase = reason_phrase;
        }

        if current_string_is_empty {
            let content_type = response._get_header(Header::CONTENT_TYPE.to_string()).unwrap();
            let is_multipart = Response::_is_multipart_byteranges_content_type(&content_type);

            if is_multipart {
                let content_range_list : Vec<ContentRange> = vec![];

                let mut buf = vec![];
                cursor.read_until(b'\n', &mut buf).unwrap();
                let boxed_value = Range::_parse_multipart_body(cursor, content_range_list);
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
                        unit: Range::BYTES.to_string(),
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
                header = Response::_parse_http_response_header_string(&string);
                if header.name == Header::CONTENT_LENGTH {
                    content_length = header.value.parse().unwrap();
                }
            }

            response.headers.push(header);
            iteration_number += 1;
            Response::_parse_raw_response_via_cursor(cursor, iteration_number, response, content_length);
        }
    }

    pub(crate) fn _is_multipart_byteranges_content_type(content_type: &Header) -> bool {
        let multipart_byteranges =
            [
                Range::MULTIPART,
                SYMBOL.slash,
                Range::BYTERANGES
            ].join("");
        let is_multipart_byteranges = content_type.value.starts_with(&multipart_byteranges);
        is_multipart_byteranges
    }

    pub(crate) fn get_x_content_type_options_header() -> Header {
        Header {
            name: Header::X_CONTENT_TYPE_OPTIONS.to_string(),
            value: Header::X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF.to_string(),
        }
    }

    pub(crate) fn get_accept_ranges_header() -> Header {
        Header {
            name: Header::ACCEPT_RANGES.to_string(),
            value: Range::BYTES.to_string(),
        }
    }
}