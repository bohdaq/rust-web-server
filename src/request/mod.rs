#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::io;
use std::io::{BufRead, Cursor, Read};
use crate::header::Header;
use crate::ext::string_ext::StringExt;
use crate::http::HTTP;
use crate::symbol::SYMBOL;
use url_build_parse::parse_url;

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Request {
    pub method: String,
    pub request_uri: String,
    pub http_version: String,
    pub headers: Vec<Header>,
    pub body: Vec<u8>,
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
    pub patch: &'static str,
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
    patch: "PATCH",
};

impl Request {
    pub const _ERROR_UNABLE_TO_PARSE_METHOD_AND_REQUEST_URI_AND_HTTP_VERSION: &'static str = "Unable to parse method, request uri and http version";

    pub fn get_header(&self, name: String) -> Option<&Header> {
        let header =  self.headers.iter().find(|x| x.name.to_lowercase() == name.to_lowercase());
        header
    }

    pub fn get_uri_query(&self) -> Result<Option<HashMap<String, String>>, String> {
        // it will return an error if unable to parse url
        // it will return None if there are no query params
        // scheme and host required for the parse_url function
        let url_array = ["http://", "localhost/", &self.request_uri];
        let url = url_array.join(SYMBOL.empty_string);

        let boxed_url_components = parse_url(&url);
        if boxed_url_components.is_err() {
            let message = boxed_url_components.err().unwrap().to_string();
            return Err(message)
        }
        Ok(boxed_url_components.unwrap().query)
    }

    pub fn get_uri_path(&self) -> Result<String, String> {
        // scheme and host required for the parse_url function
        let url_array = ["http://", "localhost", &self.request_uri];
        let url = url_array.join(SYMBOL.empty_string);

        let boxed_url_components = parse_url(&url);
        if boxed_url_components.is_err() {
            let message = boxed_url_components.err().unwrap().to_string();
            return Err(message)
        }
        Ok(boxed_url_components.unwrap().path)
    }

    pub fn method_list() -> Vec<String> {
        let method_get = METHOD.get.to_string();
        let method_head = METHOD.head.to_string();
        let method_post = METHOD.post.to_string();
        let method_put = METHOD.put.to_string();
        let method_delete = METHOD.delete.to_string();
        let method_connect = METHOD.connect.to_string();
        let method_options = METHOD.options.to_string();
        let method_trace = METHOD.trace.to_string();
        let method_patch = METHOD.patch.to_string();

        let method_list = vec![
            method_get,
            method_head,
            method_post,
            method_put,
            method_delete,
            method_connect,
            method_options,
            method_trace,
            method_patch,
        ];

        method_list
    }

    pub fn _generate_request(request: Request) -> String {
        let status = [
            request.method,
            request.request_uri,
            request.http_version,
            SYMBOL.new_line_carriage_return.to_string()
        ].join(SYMBOL.whitespace);

        let mut headers = SYMBOL.empty_string.to_string();
        for header in request.headers {
            let mut header_string = SYMBOL.empty_string.to_string();
            header_string.push_str(&header.name);
            header_string.push_str(Header::NAME_VALUE_SEPARATOR);
            header_string.push_str(&header.value);
            header_string.push_str(SYMBOL.new_line_carriage_return);
            headers.push_str(&header_string);
        }

        let request = format!(
            "{}{}{}",
            status,
            headers,
            SYMBOL.new_line_carriage_return
        );


        request
    }

    pub fn parse_request(request_vec_u8: &[u8]) ->  Result<Request, String> {
        let mut cursor = io::Cursor::new(request_vec_u8);

        let mut request = Request {
            method: "".to_string(),
            request_uri: "".to_string(),
            http_version: "".to_string(),
            headers: vec![],
            body: vec![],
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


    pub fn parse_method_and_request_uri_and_http_version_string(http_version_status_code_reason_phrase: &str) -> Result<(String, String, String), String> {
        let lowercase_unparsed_method_and_request_uri_and_http_version = http_version_status_code_reason_phrase.trim();

        let boxed_split_without_method = lowercase_unparsed_method_and_request_uri_and_http_version.split_once(SYMBOL.whitespace);
        if boxed_split_without_method.is_none() {
            return Err(Request::_ERROR_UNABLE_TO_PARSE_METHOD_AND_REQUEST_URI_AND_HTTP_VERSION.to_string())
        }

        let (method, without_method) = boxed_split_without_method.unwrap();
        let supported_methods = Request::method_list();
        if !supported_methods.contains(&method.to_uppercase().to_string()) {
            return Err(Request::_ERROR_UNABLE_TO_PARSE_METHOD_AND_REQUEST_URI_AND_HTTP_VERSION.to_string())
        }

        let boxed_without_method = without_method.split_once(SYMBOL.whitespace);
        if boxed_without_method.is_none() {
            return Err(Request::_ERROR_UNABLE_TO_PARSE_METHOD_AND_REQUEST_URI_AND_HTTP_VERSION.to_string())
        }

        let (request_uri, http_version) = boxed_without_method.unwrap();


        let supported_http_versions = HTTP::version_list();
        if !supported_http_versions.contains(&http_version.to_uppercase().to_string()) {
            return Err(Request::_ERROR_UNABLE_TO_PARSE_METHOD_AND_REQUEST_URI_AND_HTTP_VERSION.to_string())
        }

        Ok((method.to_string(), request_uri.to_string(), http_version.to_string()))

    }

    pub fn parse_http_request_header_string(header_string: &str) -> Header {
        let header_parts: Vec<&str> = header_string.split(Header::NAME_VALUE_SEPARATOR).collect();
        let header_name = StringExt::truncate_new_line_carriage_return(header_parts[0]);
        let mut header_value= "".to_string();
        if header_parts.get(1).is_some() {
            header_value = StringExt::truncate_new_line_carriage_return(header_parts[1]);
        }

        Header {
            name: header_name,
            value: header_value,
        }
    }

    pub fn cursor_read(cursor: &mut Cursor<&[u8]>, mut iteration_number: usize, request: &mut Request, mut content_length: usize) -> Result<bool, String> {
        let mut buf = vec![];
        let bytes_offset = cursor.read_until(b'\n', &mut buf).unwrap();
        let b : &[u8] = &buf;
        let boxed_request = String::from_utf8(Vec::from(b));
        if boxed_request.is_err() {
            let error_message = boxed_request.err().unwrap().to_string();
            return Err(error_message);
        }
        let string = boxed_request.unwrap();

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
                if header.name == Header::_CONTENT_LENGTH {
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

        // remaining part is request body

        let mut buf = vec![];
        let _ = cursor.read_to_end(&mut buf).unwrap();
        let b : &[u8] = &buf;

        request.body.append(&mut Vec::from(b));// = Vec::from(b);

        Ok(true)
    }

}

impl std::fmt::Display for Request {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "Request method {} and request uri {} and http_version {}", self.method, self.request_uri, self.http_version)
    }
}