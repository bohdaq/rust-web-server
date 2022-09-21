#[cfg(test)]
mod tests;

use std::io;
use std::io::{BufRead, Cursor};
use crate::header::Header;
use regex::{Regex};
use crate::constant::{CONSTANTS};
use crate::Server;

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Request {
    pub(crate) method: String,
    pub(crate) request_uri: String,
    pub(crate) http_version: String,
    pub(crate) headers: Vec<Header>,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Method {
    pub get: &'static str,
    pub head: &'static str,
    pub post: &'static str,
    pub put: &'static str,
    pub delete: &'static str,
    pub connect: &'static str,
    pub options: &'static str,
    pub trace: &'static str,
}

pub const METHOD: Method = Method {
    get: "GET",
    head: "HEAD",
    post: "POST",
    put: "PUT",
    delete: "DELETE",
    connect: "CONNECT",
    options: "OPTIONS",
    trace: "TRACE",
};

impl Request {
    pub(crate) const METHOD_AND_REQUEST_URI_AND_HTTP_VERSION_REGEX: &'static str = "(?P<method>(GET|HEAD|POST|PUT|DELETE|CONNECT|OPTIONS|TRACE))\\s(?P<request_uri>[^\\s]+)\\s(?P<http_version>[/.A-Za-z0-9]+)";

    pub(crate) fn get_header(&self, name: String) -> Option<&Header> {
        let header =  self.headers.iter().find(|x| x.name == name);
        header
    }

    pub(crate) fn generate_request(request: Request) -> String {
        let status = [
            request.method,
            request.request_uri,
            request.http_version,
            CONSTANTS.new_line_separator.to_string()
        ].join(CONSTANTS.whitespace);

        let mut headers = CONSTANTS.empty_string.to_string();
        for header in request.headers {
            let mut header_string = CONSTANTS.empty_string.to_string();
            header_string.push_str(&header.name);
            header_string.push_str(Header::NAME_VALUE_SEPARATOR);
            header_string.push_str(&header.value);
            header_string.push_str(CONSTANTS.new_line_separator);
            headers.push_str(&header_string);
        }

        let request = format!(
            "{}{}{}",
            status,
            headers,
            CONSTANTS.new_line_separator
        );


        request
    }

    pub(crate) fn parse_request(request_vec_u8: &[u8]) ->  Result<Request, String> {
        let mut cursor = io::Cursor::new(request_vec_u8);

        let mut request = Request {
            method: "".to_string(),
            request_uri: "".to_string(),
            http_version: "".to_string(),
            headers: vec![]
        };

        let content_length: usize = 0;
        let iteration_number : usize = 0;
        return match Request::cursor_read(&mut cursor, iteration_number, &mut request, content_length) {
            Ok(_) => {
                Ok(request)
            }
            Err(error_message) => {
                Err(error_message)
            }
        }

    }

    pub(crate)  fn parse_method_and_request_uri_and_http_version_string(http_version_status_code_reason_phrase: &str) -> Result<(String, String, String), String> {
        let re = Regex::new(Request::METHOD_AND_REQUEST_URI_AND_HTTP_VERSION_REGEX).unwrap();
        let caps = re.captures(&http_version_status_code_reason_phrase);

        return match caps {
            None => {
                let message = format!("Unable to parse method, request uri and http version: {}", http_version_status_code_reason_phrase);
                return Err(message)
            }
            Some(captures) => {
                let method = String::from(&captures["method"]);
                let request_uri = String::from(&captures["request_uri"]);
                let http_version = String::from(&captures["http_version"]);

                Ok((method, request_uri, http_version))
            }
        }



    }

    pub(crate)  fn parse_http_request_header_string(header_string: &str) -> Header {
        let header_parts: Vec<&str> = header_string.split(Header::NAME_VALUE_SEPARATOR).collect();
        let header_name = Server::truncate_new_line_carriage_return(header_parts[0]);
        let header_value = Server::truncate_new_line_carriage_return(header_parts[1]);

        Header {
            name: header_name,
            value: header_value,
        }
    }

    pub(crate) fn cursor_read(cursor: &mut Cursor<&[u8]>, mut iteration_number: usize, request: &mut Request, mut content_length: usize) -> Result<bool, String> {
        let mut buf = vec![];
        let bytes_offset = cursor.read_until(b'\n', &mut buf).unwrap();
        let b : &[u8] = &buf;
        let string = String::from_utf8(Vec::from(b)).unwrap();

        let is_first_iteration = iteration_number == 0;
        let new_line_char_found = bytes_offset != 0;
        let current_string_is_empty = string.trim().len() == 0;

        if is_first_iteration {
            match Request::parse_method_and_request_uri_and_http_version_string(&string) {
                Ok((method, request_uri, http_version)) => {
                    request.method = method;
                    request.request_uri = request_uri;
                    request.http_version = http_version;
                }
                Err(error_message) => {
                    return Err(error_message)
                }
            }
        }

        if current_string_is_empty {
            return Ok(true);
        }

        if new_line_char_found && !current_string_is_empty {
            let mut header = Header { name: "".to_string(), value: "".to_string() };
            if !is_first_iteration {
                header = Request::parse_http_request_header_string(&string);
                if header.name == Header::CONTENT_LENGTH {
                    content_length = header.value.parse().unwrap();
                }
            }

            request.headers.push(header);
            iteration_number += 1;
            let boxed_read = Request::cursor_read(cursor, iteration_number, request, content_length);
            if boxed_read.is_err() {
                let reason = boxed_read.err().unwrap().to_string();
                eprintln!("unable to read request: {}", reason);
            }
        }

        Ok(true)
    }

}

impl std::fmt::Display for Request {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "Request method {} and request uri {} and http_version {}", self.method, self.request_uri, self.http_version)
    }
}