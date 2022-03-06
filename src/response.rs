use std::io;
use std::io::{BufRead, Cursor, Read};
use crate::header::Header;
use regex::Regex;
use crate::app::App;
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
    pub(crate) const HTTP_VERSION_AND_STATUS_CODE_AND_REASON_PHRASE_REGEX: &'static str = "(?P<http_version>\\w+/\\w+.\\w)\\s(?P<status_code>\\w+)\\s(?P<reason_phrase>.+)";

    pub(crate) fn get_header(&self, name: String) -> Option<&Header> {
        let header =  self.headers.iter().find(|x| x.header_name == name);
        header
    }

    pub(crate) fn generate_body(content_range_list: Vec<ContentRange>) -> Vec<u8> {
        let mut body = vec![];
        let ONE = 1;

        if content_range_list.len() == ONE {
            let index = 0;
            let content_range = content_range_list.get(index).unwrap();
            body = content_range.body.to_vec();
        }

        if content_range_list.len() > ONE {
            for (i, content_range) in content_range_list.iter().enumerate() {
                let mut body_str = CONSTANTS.EMPTY_STRING.to_string();
                if i != 0 {
                    body_str.push_str(CONSTANTS.NEW_LINE_SEPARATOR);
                }
                body_str.push_str(CONSTANTS.SEPARATOR);
                body_str.push_str(CONSTANTS.NEW_LINE_SEPARATOR);
                let content_type = [Header::CONTENT_TYPE, CONSTANTS.HEADER_NAME_VALUE_SEPARATOR, CONSTANTS.WHITESPACE, &content_range.content_type.to_string()].join("");
                body_str.push_str(content_type.as_str());
                body_str.push_str(CONSTANTS.NEW_LINE_SEPARATOR);
                let content_range_header = [Header::CONTENT_RANGE, CONSTANTS.HEADER_NAME_VALUE_SEPARATOR, CONSTANTS.WHITESPACE, CONSTANTS.BYTES, CONSTANTS.WHITESPACE, &content_range.range.start.to_string(), CONSTANTS.HYPHEN, &content_range.range.end.to_string(), CONSTANTS.SLASH, &content_range.size].join("");
                body_str.push_str(content_range_header.as_str());
                body_str.push_str(CONSTANTS.NEW_LINE_SEPARATOR);
                body_str.push_str(CONSTANTS.NEW_LINE_SEPARATOR);

                let inner_body = [body_str.as_bytes(), &content_range.body].concat();
                body = [body, inner_body].concat();
            }
            let mut trailing_separator = CONSTANTS.EMPTY_STRING.to_string();
            trailing_separator.push_str(CONSTANTS.NEW_LINE_SEPARATOR);
            trailing_separator.push_str(CONSTANTS.SEPARATOR);
            body = [&body, trailing_separator.as_bytes()].concat();
        }

        body
    }

    pub(crate) fn generate_response(mut response: Response, request: Request) -> Vec<u8> {
        let status = [response.http_version, response.status_code, response.reason_phrase].join(CONSTANTS.WHITESPACE);
        let mut headers = vec![
            App::get_x_content_type_options_header(),
            App::get_accept_ranges_header(),
        ];

        if response.content_range_list.len() == 1 {
            let content_range_index = 0;
            let content_range = response.content_range_list.get(content_range_index).unwrap();
            headers.push(Header {
                header_name: Header::CONTENT_TYPE.to_string(),
                header_value: content_range.content_type.to_string()
            });

            let content_range_header_value = [
                CONSTANTS.BYTES,
                CONSTANTS.WHITESPACE,
                &content_range.range.start.to_string(),
                CONSTANTS.HYPHEN,
                &content_range.range.end.to_string(),
                CONSTANTS.SLASH,
                &content_range.size
            ].join("");
            headers.push(Header {
                header_name: Header::CONTENT_RANGE.to_string(),
                header_value: content_range_header_value.to_string()
            });

            headers.push(Header {
                header_name: Header::CONTENT_LENGTH.to_string(),
                header_value: content_range.body.len().to_string()
            });
        }

        if response.content_range_list.len() > 1 {
            let content_range_header_value = [
                CONSTANTS.MULTIPART,
                CONSTANTS.SLASH,
                CONSTANTS.BYTERANGES,
                CONSTANTS.SEMICOLON,
                CONSTANTS.WHITESPACE,
                CONSTANTS.BOUNDARY,
                CONSTANTS.EQUALS,
                CONSTANTS.STRING_SEPARATOR
            ].join("");
            headers.push(Header {
                header_name: Header::CONTENT_TYPE.to_string(),
                header_value: content_range_header_value,
            });
        }

        let mut body = Response::generate_body(response.content_range_list);

        let mut headers_str = CONSTANTS.NEW_LINE_SEPARATOR.to_string();
        for header in headers {
            let mut header_string = CONSTANTS.EMPTY_STRING.to_string();
            header_string.push_str(&header.header_name);
            header_string.push_str(CONSTANTS.HEADER_NAME_VALUE_SEPARATOR);
            header_string.push_str(&header.header_value);
            header_string.push_str(CONSTANTS.NEW_LINE_SEPARATOR);
            headers_str.push_str(&header_string);
        }

        let response_without_body = format!(
            "{}{}{}",
            status,
            headers_str,
            CONSTANTS.NEW_LINE_SEPARATOR,
        );

        println!("_____RESPONSE w/o body______\n{}", &response_without_body);

        let mut response_as_vector : Vec<u8> = vec![];

        let is_head = request.method == REQUEST_METHODS.HEAD;
        let is_options = request.method == REQUEST_METHODS.OPTIONS;
        if is_head || is_options {
            response_as_vector = response_without_body.into_bytes();
        } else {
            response_as_vector = [response_without_body.into_bytes(), body].concat();
        }

        response_as_vector
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

        let mut content_length: usize = 0;
        let mut iteration_number : usize = 0;

        Response::parse_raw_response_via_cursor(&mut cursor, iteration_number, &mut response, content_length);

        return response;
    }

    pub(crate)  fn parse_http_version_status_code_reason_phrase_string(http_version_status_code_reason_phrase: &str) -> (String, String, String) {
        let re = Regex::new(Response::HTTP_VERSION_AND_STATUS_CODE_AND_REASON_PHRASE_REGEX).unwrap();
        let caps = re.captures(&http_version_status_code_reason_phrase).unwrap();

        let http_version= String::from(&caps["http_version"]);
        let status_code = String::from(&caps["status_code"]);
        let mut reason_phrase = String::from(&caps["reason_phrase"]);
        reason_phrase = Server::truncate_new_line_carriage_return(&reason_phrase);

        return (http_version, status_code, reason_phrase)
    }

    pub(crate)  fn parse_http_response_header_string(header_string: &str) -> Header {
        let mut header_parts: Vec<&str> = header_string.split(CONSTANTS.HEADER_NAME_VALUE_SEPARATOR).collect();
        let mut raw_header_name = header_parts[0].to_string();
        let mut header_name = Server::truncate_new_line_carriage_return(&raw_header_name);
        let mut raw_header_value = header_parts[1].to_string();
        let mut header_value = Server::truncate_new_line_carriage_return(&raw_header_value);


        Header {
            header_name: header_name.to_string(),
            header_value: header_value.to_string()
        }
    }

    pub(crate) fn parse_raw_response_via_cursor(cursor: &mut Cursor<&[u8]>, mut iteration_number: usize, response: &mut Response, mut content_length: usize) {
        let mut buf = vec![];
        let bytes_offset = cursor.read_until(b'\n', &mut buf).unwrap();
        let mut b : &[u8] = &buf;
        let string = String::from_utf8(Vec::from(b)).unwrap();

        let is_first_iteration = iteration_number == 0;
        let new_line_char_found = bytes_offset != 0;
        let current_string_is_empty = string.trim().len() == 0;

        if is_first_iteration {
            let (http_version, status_code, reason_phrase) = Response::parse_http_version_status_code_reason_phrase_string(&string);
            println!("{} {} {}", http_version, status_code, reason_phrase);

            response.http_version = http_version;
            response.status_code = status_code;
            response.reason_phrase = reason_phrase;
        }

        if current_string_is_empty {
            println!("end of headers... parse message length: {}", content_length);
            let content_type = response.get_header(Header::CONTENT_TYPE.to_string()).unwrap();
            let is_multipart = Response::is_multipart_byteranges_content_type(&content_type);

            if is_multipart {
                let mut content_range_list : Vec<ContentRange> = vec![];

                let mut buf = vec![];
                cursor.read_until(b'\n', &mut buf).unwrap();
                let boxed_value = Range::parse_multipart_body(cursor, content_range_list);
                let mut range_list = vec![];
                if boxed_value.is_ok() {
                    range_list = boxed_value.unwrap();
                }
                response.content_range_list = range_list;
            } else {
                buf = vec![];
                cursor.read_to_end(&mut buf);
                b = &buf;

                let content_range = ContentRange {
                    unit: CONSTANTS.BYTES.to_string(),
                    range: Range {
                        start: 0,
                        end: b.len() as u64
                    },
                    size: b.len().to_string(),
                    body: Vec::from(b),
                    content_type: content_type.header_value.to_string()
                };
                response.content_range_list = vec![content_range];
            }

            return;
        }

        if new_line_char_found && !current_string_is_empty {
            let mut header = Header { header_name: "".to_string(), header_value: "".to_string() };
            if !is_first_iteration {
                header = Response::parse_http_response_header_string(&string);
                println!("{}: {}", &header.header_name, &header.header_value);
                if header.header_name == Header::CONTENT_LENGTH {
                    content_length = header.header_value.parse().unwrap();
                }
            }

            response.headers.push(header);
            iteration_number += 1;
            Response::parse_raw_response_via_cursor(cursor, iteration_number, response, content_length);
        }
    }

    pub(crate) fn is_multipart_byteranges_content_type(content_type: &Header) -> bool {
        let multipart_byteranges = [CONSTANTS.MULTIPART, CONSTANTS.SLASH, CONSTANTS.BYTERANGES].join("");
        let is_multipart_byteranges = content_type.header_value.starts_with(&multipart_byteranges);
        is_multipart_byteranges
    }
}