use std::io;
use std::io::{BufRead, Cursor};
use crate::header::Header;
use regex::Regex;
use crate::constant::{CONSTANTS, HTTP_HEADERS};
use crate::Server;

pub struct Response {
    pub(crate) http_version: String,
    pub(crate) status_code: String,
    pub(crate) reason_phrase: String,
    pub(crate) headers: Vec<Header>,
    pub(crate) message_body: Vec<u8>
}

impl Response {
    pub(crate) const HTTP_VERSION_AND_STATUS_CODE_AND_REASON_PHRASE_REGEX: &'static str = "(?P<http_version>\\w+/\\w+.\\w)\\s(?P<status_code>\\w+)\\s(?P<reason_phrase>.+)";

    pub(crate) fn get_header(&self, name: String) -> Option<&Header> {
        let header =  self.headers.iter().find(|x| x.header_name == name);
        header
    }

    pub(crate) fn generate_response(response: Response) -> Vec<u8> {
        let status = [response.http_version, response.status_code, response.reason_phrase].join(CONSTANTS.WHITESPACE);

        let mut headers = CONSTANTS.NEW_LINE_SEPARATOR.to_string();
        for header in response.headers {
            let mut header_string = CONSTANTS.EMPTY_STRING.to_string();
            header_string.push_str(&header.header_name);
            header_string.push_str(CONSTANTS.HEADER_NAME_VALUE_SEPARATOR);
            header_string.push_str(&header.header_value);
            header_string.push_str(CONSTANTS.NEW_LINE_SEPARATOR);
            headers.push_str(&header_string);
        }

        let mut content_length_header_string = CONSTANTS.EMPTY_STRING.to_string();
        content_length_header_string.push_str("Content-Length");
        content_length_header_string.push_str(CONSTANTS.HEADER_NAME_VALUE_SEPARATOR);
        content_length_header_string.push_str(response.message_body.len().to_string().as_str());
        content_length_header_string.push_str(CONSTANTS.NEW_LINE_SEPARATOR);
        headers.push_str(&content_length_header_string);

        let response_without_body = format!(
            "{}{}{}",
            status,
            headers,
            CONSTANTS.NEW_LINE_SEPARATOR,
        );

        println!("_____RESPONSE w/o body______\n{}", &response_without_body);

        let mut response  = [response_without_body.into_bytes(), response.message_body].concat();


        response
    }

    pub(crate) fn parse_response(response_vec_u8: &[u8]) -> Response {
        let mut cursor = io::Cursor::new(response_vec_u8);

        let mut response = Response {
            http_version: "".to_string(),
            status_code: "".to_string(),
            reason_phrase: "".to_string(),
            headers: vec![],
            message_body: vec![],
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
        let reason_phrase = String::from(&caps["reason_phrase"]);

        return (http_version, status_code, reason_phrase)
    }

    pub(crate)  fn parse_http_response_header_string(header_string: &str) -> Header {
        let mut header_parts: Vec<&str> = header_string.split(CONSTANTS.HEADER_NAME_VALUE_SEPARATOR).collect();
        let mut raw_header_name = header_parts[0].to_string();
        let mut header_name = Server::truncate_new_line_carriage_return(raw_header_name);
        let mut raw_header_value = header_parts[1].to_string();
        let mut header_value = Server::truncate_new_line_carriage_return(raw_header_value);


        Header {
            header_name: header_name.to_string(),
            header_value: header_value.to_string()
        }
    }



    pub(crate) fn cursor_read(cursor: &mut Cursor<&[u8]>, mut iteration_number: usize, response: &mut Response, mut content_length: usize) {
        let mut buf = vec![];
        let bytes_offset = cursor.read_until(b'\n', &mut buf).unwrap();
        let b : &[u8] = &buf;
        let string = String::from_utf8(Vec::from(b)).unwrap();

        let is_first_iteration = iteration_number == 0;
        let no_more_new_line_chars_found = bytes_offset == 0;
        let new_line_char_found = bytes_offset != 0;
        let current_string_is_empty = string.trim().len() == 0;

        println!("is_first_iteration: {}", is_first_iteration);
        println!("no_more_new_line_chars_found: {}", no_more_new_line_chars_found);
        println!("new_line_char_found: {}", new_line_char_found);
        println!("current_string_is_empty: {}", current_string_is_empty);

        if is_first_iteration {
            let (http_version, status_code, reason_phrase) = Response::parse_http_version_status_code_reason_phrase_string(&string);
            println!("http_version: {} status_code: {} reason_phrase: {}", http_version, status_code, reason_phrase);

            response.http_version = http_version;
            response.status_code = status_code;
            response.reason_phrase = reason_phrase;
        }

        if no_more_new_line_chars_found && current_string_is_empty {
            println!("!!!end of headers...parse message body here");
            return;
        }

        if new_line_char_found && !current_string_is_empty {
            let mut header = Header { header_name: "".to_string(), header_value: "".to_string() };
            if !is_first_iteration {
                header = Response::parse_http_response_header_string(&string);
                if header.header_name == HTTP_HEADERS.CONTENT_LENGTH {
                    println!("!!!! \n{}", &header.header_name);
                    println!("!!!! \n{}", &header.header_value);
                    println!("!!!! \n{}", &string);
                    content_length = header.header_value.parse().unwrap();
                    println!("content_length: {}", content_length);
                }
            }

            println!("{}: {}", header.header_name, header.header_value);
            response.headers.push(header);
            iteration_number += 1;
            Response::cursor_read(cursor, iteration_number, response, content_length);
        }
    }
}