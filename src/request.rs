use std::io;
use std::io::{BufRead, Cursor};
use crate::header::Header;
use regex::Regex;
use crate::constant::{CONSTANTS, HTTP_HEADERS};

pub struct Request {
    pub(crate) method: String,
    pub(crate) request_uri: String,
    pub(crate) http_version: String,
    pub(crate) headers: Vec<Header>,
}

impl Request {
    pub(crate) const METHOD_AND_REQUEST_URI_AND_HTTP_VERSION_REGEX: &'static str = "(?P<method>\\w+)\\s(?P<request_uri>[._/A-Za-z0-9]+)\\s(?P<http_version>[/.A-Za-z0-9]+)";

    pub(crate) fn get_header(&self, name: String) -> Option<&Header> {
        let header =  self.headers.iter().find(|x| x.header_name == name);
        header
    }

    pub(crate) fn generate_request(request: Request) -> String {
        let status = [request.method, request.request_uri, request.http_version, CONSTANTS.NEW_LINE_SEPARATOR.to_string()].join(CONSTANTS.WHITESPACE);

        let mut headers = CONSTANTS.EMPTY_STRING.to_string();
        for header in request.headers {
            let mut header_string = CONSTANTS.EMPTY_STRING.to_string();
            header_string.push_str(&header.header_name);
            header_string.push_str(CONSTANTS.HEADER_NAME_VALUE_SEPARATOR);
            header_string.push_str(&header.header_value);
            header_string.push_str(CONSTANTS.NEW_LINE_SEPARATOR);
            headers.push_str(&header_string);
        }

        let request = format!(
            "{}{}{}",
            status,
            headers,
            CONSTANTS.NEW_LINE_SEPARATOR
        );

        println!("_____REQUEST______\n{}", request);

        request
    }

    pub(crate) fn parse_request(request_vec_u8: &[u8]) ->  Request {
        let mut cursor = io::Cursor::new(request_vec_u8);

        let mut request = Request {
            method: "".to_string(),
            request_uri: "".to_string(),
            http_version: "".to_string(),
            headers: vec![]
        };

        let mut content_length: usize = 0;
        let mut iteration_number : usize = 0;
        Request::cursor_read(&mut cursor, iteration_number, &mut request, content_length);

        request
    }

    pub(crate)  fn parse_method_and_request_uri_and_http_version_string(http_version_status_code_reason_phrase: &str) -> (String, String, String) {
        let re = Regex::new(Request::METHOD_AND_REQUEST_URI_AND_HTTP_VERSION_REGEX).unwrap();
        let caps = re.captures(&http_version_status_code_reason_phrase).unwrap();

        let method= String::from(&caps["method"]);
        let request_uri = String::from(&caps["request_uri"]);
        let http_version = String::from(&caps["http_version"]);

        return (method, request_uri, http_version)
    }

    pub(crate)  fn parse_http_request_header_string(header_string: &str) -> Header {
        let header_parts: Vec<&str> = header_string.split(CONSTANTS.HEADER_NAME_VALUE_SEPARATOR).collect();

        Header {
            header_name: header_parts[0].to_string(),
            header_value: header_parts[1].to_string()
        }
    }

    pub(crate) fn cursor_read(cursor: &mut Cursor<&[u8]>, mut iteration_number: usize, request: &mut Request, mut content_length: usize) {
        let mut buf = vec![];
        let bytes_offset = cursor.read_until(b'\n', &mut buf).unwrap();
        let b : &[u8] = &buf;
        let string = String::from_utf8(Vec::from(b)).unwrap();
        let is_first_iteration = iteration_number == 0;
        let no_more_new_line_chars_found = bytes_offset == 0;
        let new_line_char_found = bytes_offset != 0;
        let current_string_is_empty = string.trim().len() == 0;
        println!("{}\n Length: {} Iteration:  {}, is_first_iteration: {}", string.trim(), string.trim().len(), iteration_number, is_first_iteration);


        if is_first_iteration {
            println!("parse method, request_uri, http_version");
            let (method, request_uri, http_version) = Request::parse_method_and_request_uri_and_http_version_string(&string);
            println!("parse method: {}, request_uri: {}, http_version: {}", method, request_uri, http_version);

            request.method = method;
            request.request_uri = request_uri;
            request.http_version = http_version;
        }

        if no_more_new_line_chars_found && current_string_is_empty {
            println!("!!!end of headers...parse message body here");
            return;
        }

        if new_line_char_found && !current_string_is_empty  && !is_first_iteration {
            let header: Header = Request::parse_http_request_header_string(&string);
            if header.header_name == HTTP_HEADERS.CONTENT_LENGTH {
                content_length = header.header_value.parse().unwrap();
                println!("content_length: {}", content_length);
            }

            request.headers.push(header);
            iteration_number += 1;
            Request::cursor_read(cursor, iteration_number, request, content_length);
        }
    }

}

impl std::fmt::Display for Request {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "Request method {} and request uri {} and http_version {}", self.method, self.request_uri, self.http_version)
    }
}