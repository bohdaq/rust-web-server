use std::io;
use std::io::{BufRead, Cursor, Read};
use crate::header::Header;
use regex::Regex;
use crate::app::App;
use crate::constant::{CONSTANTS, HTTP_HEADERS};
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
                let content_type = [HTTP_HEADERS.CONTENT_TYPE, CONSTANTS.HEADER_NAME_VALUE_SEPARATOR, CONSTANTS.WHITESPACE, &content_range.content_type.to_string()].join("");
                body_str.push_str(content_type.as_str());
                body_str.push_str(CONSTANTS.NEW_LINE_SEPARATOR);
                let content_range_header = [HTTP_HEADERS.CONTENT_RANGE, CONSTANTS.HEADER_NAME_VALUE_SEPARATOR, CONSTANTS.WHITESPACE, CONSTANTS.BYTES, CONSTANTS.WHITESPACE, &content_range.range.start.to_string(), CONSTANTS.HYPHEN, &content_range.range.end.to_string(), CONSTANTS.SLASH, &content_range.size].join("");
                body_str.push_str(content_range_header.as_str());
                body_str.push_str(CONSTANTS.NEW_LINE_SEPARATOR);
                body_str.push_str(CONSTANTS.NEW_LINE_SEPARATOR);

                let inner_body = [body_str.as_bytes(), &content_range.body].concat();
                body = [body, inner_body].concat();
            }
            let mut trailing_separator = CONSTANTS.EMPTY_STRING.to_string();
            trailing_separator.push_str(CONSTANTS.SEPARATOR);
            body = [&body, trailing_separator.as_bytes()].concat();
        }

        body
    }

    pub(crate) fn generate_response(mut response: Response) -> Vec<u8> {
        let status = [response.http_version, response.status_code, response.reason_phrase].join(CONSTANTS.WHITESPACE);
        let mut headers = vec![
            App::get_x_content_type_options_header(),
            App::get_accept_ranges_header(),
        ];

        if response.content_range_list.len() == 1 {
            let content_range_index = 0;
            let content_range = response.content_range_list.get(content_range_index).unwrap();
            headers.push(Header {
                header_name: HTTP_HEADERS.CONTENT_TYPE.to_string(),
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
                header_name: HTTP_HEADERS.CONTENT_RANGE.to_string(),
                header_value: content_range_header_value.to_string()
            });

            headers.push(Header {
                header_name: HTTP_HEADERS.CONTENT_LENGTH.to_string(),
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
                header_name: HTTP_HEADERS.CONTENT_TYPE.to_string(),
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

        let mut response  = [response_without_body.into_bytes(), body].concat();


        response
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
        Response::cursor_read(&mut cursor, iteration_number, &mut response, content_length);

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

    pub(crate) fn parse_multipart_body(cursor: &mut Cursor<&[u8]>, mut content_range_list: Vec<ContentRange>) -> Vec<ContentRange> {

        let mut buf = vec![];
        let bytes_offset = cursor.read_until(b'\n', &mut buf).unwrap();
        let mut b : &[u8] = &buf;
        let mut string = String::from_utf8(Vec::from(b)).unwrap();

        let new_line_char_found = bytes_offset != 0;
        let current_string_is_empty = string.trim().len() == 0;

        // println!("string: {} new_line_char_found: {} current_string_is_empty: {}", string, new_line_char_found, current_string_is_empty);
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
            buf = vec![];
            cursor.read_until(b'\n', &mut buf).unwrap();
            b = &buf;
            string = String::from_utf8(Vec::from(b)).unwrap();


            println!("string: {}", string);
        }

        let content_type_is_not_parsed = content_range.content_type.len() == 0;
        if string.starts_with(HTTP_HEADERS.CONTENT_TYPE) && content_type_is_not_parsed {
            let content_type = Response::parse_http_response_header_string(string.as_str());
            content_range.content_type = content_type.header_value.trim().to_string();

            buf = vec![];
            cursor.read_until(b'\n', &mut buf).unwrap();
            b = &buf;
            string = String::from_utf8(Vec::from(b)).unwrap();


            println!("content type is {}", &content_range.content_type);
            println!("string: {}", string);
        }

        let content_range_is_not_parsed = content_range.size.len() == 0;
        if string.starts_with(HTTP_HEADERS.CONTENT_RANGE) && content_range_is_not_parsed {
            let content_range_header = Response::parse_http_response_header_string(string.as_str());
            //parse header value ...
            let split_token = [CONSTANTS.BYTES, CONSTANTS.WHITESPACE].join("");
            let first_split: Vec<&str> = content_range_header.header_value.split(&split_token).collect();

            let value_index = 1;
            let first_split_string = first_split.get(value_index).unwrap().trim();
            //println!(": {}", &first_split_string);

            let split_token = CONSTANTS.SLASH;
            let second_split: Vec<&str> = first_split_string.split(split_token).collect();

            let second_split_first_value = second_split.get(0).unwrap().trim();
            let second_split_second_value = second_split.get(1).unwrap().trim();
            content_range.size = second_split_second_value.to_string();
            //println!(": {} : {}", &second_split_first_value, &second_split_second_value);

            let split_token = CONSTANTS.HYPHEN;
            let third_split : Vec<&str> = second_split_first_value.split(split_token).collect();
            let third_split_first_value =  third_split.get(0).unwrap().trim();
            let third_split_second_value =  third_split.get(1).unwrap().trim();
            content_range.range.start = third_split_first_value.parse().unwrap();
            content_range.range.end = third_split_second_value.parse().unwrap();
            //println!(": {} : {}", &third_split_first_value, &third_split_second_value);



            buf = vec![];
            cursor.read_until(b'\n', &mut buf).unwrap();
            b = &buf;
            string = String::from_utf8(Vec::from(b)).unwrap();

            println!("content range start is {} and end {}, size is {}", content_range.range.start, content_range.range.end, content_range.size);
            println!("string: {}", string);
        }

        let current_string_is_empty = string.trim().len() == 0;
        if current_string_is_empty {
            buf = vec![];
            cursor.read_until(b'\n', &mut buf).unwrap();
            b = &buf;
            string = String::from_utf8(Vec::from(b)).unwrap();

            println!("empty string, next line");
            println!("string: {}", string);
        }

        let current_string_is_empty = string.trim().len() == 0;
        let content_range_is_parsed = content_range.size.len() != 0;
        let content_type_is_parsed = content_range.content_type.len() != 0;
        if !current_string_is_empty && content_range_is_parsed && content_type_is_parsed {
            let mut body : Vec<u8> = vec![];
            // println!("before while {} body.len: {} string len: {}", string, body.len(), string.len());
            body = [body, string.as_bytes().to_vec()].concat();

            while !string.starts_with(CONSTANTS.SEPARATOR) {
                buf = vec![];
                cursor.read_until(b'\n', &mut buf).unwrap();
                b = &buf;
                string = String::from_utf8(Vec::from(b)).unwrap();
                // println!("in while {}", string);

                if !string.starts_with(CONSTANTS.SEPARATOR) {
                    body = [body, string.as_bytes().to_vec()].concat();
                }
            }
            //remove new line '\n' char from content range body
            //body.pop();


            let mut debug_body : &[u8]  = &body;
            println!("content range body is {} length is {}", String::from_utf8(debug_body.to_vec()).unwrap(), debug_body.len());
            content_range.body = body;
        }

        // println!("!!! {} {} {} {} {} {}", content_range.unit, content_range.content_type, content_range.size, content_range.range.start, content_range.range.end, content_range.body.len());

        content_range_list = Response::parse_multipart_body(cursor, content_range_list);

        content_range_list
    }



    pub(crate) fn cursor_read(cursor: &mut Cursor<&[u8]>, mut iteration_number: usize, response: &mut Response, mut content_length: usize) {
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
            let content_type = response.get_header(HTTP_HEADERS.CONTENT_TYPE.to_string()).unwrap();
            let is_multipart = Response::is_multipart_byteranges_content_type(&content_type);

            if is_multipart {
                let mut content_range_list : Vec<ContentRange> = vec![];

                let mut buf = vec![];
                cursor.read_until(b'\n', &mut buf).unwrap();
                content_range_list = Response::parse_multipart_body(cursor, content_range_list);


                response.content_range_list = content_range_list;
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
                if header.header_name == HTTP_HEADERS.CONTENT_LENGTH {
                    content_length = header.header_value.parse().unwrap();
                }
            }

            response.headers.push(header);
            iteration_number += 1;
            Response::cursor_read(cursor, iteration_number, response, content_length);
        }
    }

    pub(crate) fn is_multipart_byteranges_content_type(content_type: &Header) -> bool {
        let multipart_byteranges = [CONSTANTS.MULTIPART, CONSTANTS.SLASH, CONSTANTS.BYTERANGES].join("");
        let is_multipart_byteranges = content_type.header_value.starts_with(&multipart_byteranges);
        is_multipart_byteranges
    }
}