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

        // let mut iteration_number : usize = 0;
        // let mut bytes_offset : usize = 0;
        // let end_of_headers = false;


        // bytes_offset = cursor.read_until(b'\n', &mut buf).unwrap();
        // let b : &[u8] = &buf;
        // let string = String::from_utf8(Vec::from(b)).unwrap();
        // println!("{}", string);
        // println!("offset in bytes: {}", bytes_offset);
        // buf.clear();
        //
        // bytes_offset = cursor.read_until(b'\n', &mut buf).unwrap();
        // let b : &[u8] = &buf;
        // let string = String::from_utf8(Vec::from(b)).unwrap();
        // println!("{}", string);
        // println!("offset in bytes: {}", bytes_offset);
        // buf.clear();

        Request::cursor_read(&mut cursor);


        let len : usize = request_vec_u8.len();
        let iteration_end_position : usize = len - 4;
        let mut last_new_line_position: usize = 0;
        let mut content_length : usize = 0;

        let mut request = Request {
            method: "".to_string(),
            request_uri: "".to_string(),
            http_version: "".to_string(),
            headers: vec![]
        };

        for i in 0..iteration_end_position {
            let first_byte = request_vec_u8[i];
            let second_byte = request_vec_u8[i+1];
            let third_byte = request_vec_u8[i+2];
            let fourth_byte = request_vec_u8[i+3];

            let char_as_u8_4 = [first_byte, second_byte, third_byte, fourth_byte];
            let char_as_u32 = Request::as_u32_be(&char_as_u8_4);
            let char = char::from_u32(char_as_u32).unwrap();

            if char == '\n' {
                let string_as_bytes_u8 = &request_vec_u8[last_new_line_position..i];
                let string = String::from_utf8(string_as_bytes_u8.to_vec()).unwrap();

                println!("String:\n{}", string);
                println!("Last new line position:\n{}", last_new_line_position);
                println!("Current new line position:\n{}", i);

                if last_new_line_position == 0 {
                    let (method, request_uri, http_version) = Request::parse_method_and_request_uri_and_http_version_string(&string);
                    println!("method: {} request_uri: {} http_version: {}", method, request_uri, http_version);

                    request.method = method;
                    request.request_uri = request_uri;
                    request.http_version = http_version;
                }

                if last_new_line_position != 0 {
                    if string.len() <= 1 {
                        println!("detected end of headers part");
                        break;
                    } else {
                        let header: Header = Request::parse_http_request_header_string(&string);

                        if header.header_name == HTTP_HEADERS.CONTENT_LENGTH {
                            content_length = header.header_value.parse().unwrap();
                            println!("content_length: {}", content_length);
                        }

                        println!("{}", header);

                        request.headers.push(header);
                    }

                }

                last_new_line_position = i + 1; // start from new line, next char after '/n'

            }

        }
        let request = Request {
            method: "".to_string(),
            request_uri: "".to_string(),
            http_version: "".to_string(),
            headers: vec![]
        };
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

    pub(crate) fn as_u32_be(array: &[u8; 4]) -> u32 {
        ((array[0] as u32) << 24 )  |
            ((array[1] as u32) << 16)   |
            ((array[2] as u32) << 8)    |
            ((array[3] as u32) << 0)
    }

    pub(crate) fn cursor_read(cursor: &mut Cursor<&[u8]>) {
        let mut buf = vec![];
        let bytes_offset = cursor.read_until(b'\n', &mut buf).unwrap();
        let b : &[u8] = &buf;
        let string = String::from_utf8(Vec::from(b)).unwrap();
        println!("{}, \n Length: {}", string.trim(), string.trim().len());
        buf.clear();

        if bytes_offset == 0 {
            println!("!!!end of \\n");
            return;
        } else {
            Request::cursor_read(cursor);
        }
    }

}

impl std::fmt::Display for Request {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "Request method {} and request uri {} and http_version {}", self.method, self.request_uri, self.http_version)
    }
}