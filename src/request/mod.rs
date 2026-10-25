//! HTTP requests: [`Request`] and its parser.
//!
//! [`Request::parse`] turns a request head (`GET /path HTTP/1.1`, header lines,
//! blank line) into a `Request`; `generate` writes one back to bytes (used by the
//! proxy and tests). The request target is normalized during parsing: origin-form
//! is kept, absolute-form reduced to its path, `*` kept, anything else rejected.
//! Look headers up with `request.header(name)` (case-insensitive, no
//! allocation). `get_query`, `get_path`, `get_domain` and `get_port` read the URL
//! parts; the domain and port come from the `Host` header, not the socket.
//! Examples: `src/request/example/`.

#[cfg(test)]
mod tests;
#[cfg(test)]
mod example;
#[cfg(test)]
mod reference_parser;

use std::collections::HashMap;
use std::io;
use std::io::Cursor;
use crate::header::Header;
use crate::http::HTTP;
use crate::symbol::SYMBOL;
use crate::url::URL;

/// A parsed HTTP request.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Request {
    /// HTTP method: `"GET"`, `"POST"`, etc. Compare with [`METHOD`] constants.
    pub method: String,
    /// Request URI including path and query string, e.g. `"/search?q=rust"`.
    pub request_uri: String,
    /// HTTP version string, e.g. `"HTTP/1.1"`. Compare with [`crate::http::VERSION`] constants.
    pub http_version: String,
    /// Request headers.
    pub headers: Vec<Header>,
    /// Raw request body bytes.
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

    /// The first header named `name` (case-insensitive). Prefer
    /// [`Request::header`], which needs no `String`.
    pub fn get_header(&self, name: String) -> Option<&Header> {
        self.header(&name)
    }

    /// The first header named `name`, compared case-insensitively without
    /// allocating (this used to lowercase every header name on every lookup).
    pub fn header(&self, name: &str) -> Option<&Header> {
        self.headers.iter().find(|h| h.name.eq_ignore_ascii_case(name))
    }

    pub fn get_domain(&self) -> Result<Option<String>, String> {
        let boxed_host = &self.header(Header::_HOST);
        if boxed_host.is_none() {
            return Ok(None);
        }

        let host_header = boxed_host.unwrap();
        let host = host_header.value.to_string();
        if host.is_empty() {
            return Ok(None); // `Host:` with no value names no domain
        }

        let boxed_host_port = host.split_once(&SYMBOL.colon.to_string());
        if boxed_host_port.is_none() {
            return Ok(Some(host))
        }

        let (domain, _port_str) = boxed_host_port.unwrap();

        Ok(Some(domain.to_string()))
    }

    pub fn get_port(&self) -> Result<Option<i128>, String> {
        let boxed_host = &self.header(Header::_HOST);
        if boxed_host.is_none() {
            return Ok(None);
        }

        let host_header = boxed_host.unwrap();
        let host = host_header.value.to_string();

        let boxed_host_port = host.split_once(&SYMBOL.colon.to_string());
        if boxed_host_port.is_none() {
            return Ok(None)
        }

        let (_domain, port_str) = boxed_host_port.unwrap();

        let boxed_port_i128 = port_str.parse::<i128>();
        if boxed_port_i128.is_err() {
            let message = boxed_port_i128.err().unwrap().to_string();
            return Err(message);
        }

        let port = boxed_port_i128.unwrap();

        Ok(Some(port))
    }

    pub fn get_query(&self) -> Result<Option<HashMap<String, String>>, String> {
        self.get_uri_query()
    }

    pub fn get_uri_query(&self) -> Result<Option<HashMap<String, String>>, String> {
        // it will return an error if unable to parse url
        // it will return None if there are no query params
        // scheme and host required for the parse_url function
        let url_array = ["http://", "localhost/", &self.request_uri];
        let url = url_array.join(SYMBOL.empty_string);

        let boxed_url_components = URL::parse(&url);
        if boxed_url_components.is_err() {
            let message = boxed_url_components.err().unwrap().to_string();
            return Err(message)
        }
        Ok(boxed_url_components.unwrap().query)
    }

    pub fn get_path(&self) -> Result<String, String> {
        self.get_uri_path()
    }

    pub fn get_uri_path(&self) -> Result<String, String> {
        // scheme and host required for the parse_url function
        let url = self.request_uri.to_string();

        let boxed_url_components = crate::url::URL::parse_request_target(&url);
        if boxed_url_components.is_err() {
            let message = boxed_url_components.err().unwrap().to_string();
            return Err(message)
        }
        Ok(boxed_url_components.unwrap().path)
    }

    /// `true` if `method` (any case) is one of [`Request::method_list`],
    /// without building that list — it's checked on every request.
    pub fn is_supported_method(method: &str) -> bool {
        [METHOD.get, METHOD.head, METHOD.post, METHOD.put, METHOD.delete, METHOD.connect, METHOD.options, METHOD.trace, METHOD.patch]
            .iter()
            .any(|m| m.eq_ignore_ascii_case(method))
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

    // instance method way to generate a request
    pub fn generate(&self) -> Vec<u8> {
        let clone = self.clone();
        let mut request = Request::_generate_request(clone).as_bytes().to_vec();

        let mut body = self.body.clone();
        request.append(&mut body);

        request
    }

    // same as _generate_request, not renamed original for backward compatability
    pub fn generate_request(request: Request) -> String {
        Request::_generate_request(request)
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

    pub fn parse(request_vec_u8: &[u8]) ->  Result<Request, String> {
        Request::parse_request(request_vec_u8)
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
        if !Request::is_supported_method(method) {
            return Err(Request::_ERROR_UNABLE_TO_PARSE_METHOD_AND_REQUEST_URI_AND_HTTP_VERSION.to_string())
        }

        let boxed_without_method = without_method.split_once(SYMBOL.whitespace);
        if boxed_without_method.is_none() {
            return Err(Request::_ERROR_UNABLE_TO_PARSE_METHOD_AND_REQUEST_URI_AND_HTTP_VERSION.to_string())
        }

        let (request_uri, http_version) = boxed_without_method.unwrap();
        let request_uri = Request::normalize_request_target(request_uri)
            .ok_or_else(|| Request::_ERROR_UNABLE_TO_PARSE_METHOD_AND_REQUEST_URI_AND_HTTP_VERSION.to_string())?;


        if !HTTP::is_supported_version(http_version) {
            return Err(Request::_ERROR_UNABLE_TO_PARSE_METHOD_AND_REQUEST_URI_AND_HTTP_VERSION.to_string())
        }

        Ok((method.to_string(), request_uri, http_version.to_string()))

    }

    /// RFC 9112 §3.2 request targets: origin-form (`/path?query`) is kept,
    /// absolute-form (`http://host/path?query`) is reduced to its
    /// origin-form path, asterisk-form (`*`) is kept. Anything else —
    /// authority-form (`host:443`, CONNECT is not served) or junk like `:x` —
    /// is `None`, so the request is rejected before reaching a controller.
    pub fn normalize_request_target(target: &str) -> Option<String> {
        if target.starts_with('/') || target == "*" {
            return Some(target.to_string());
        }
        let lower = target.to_ascii_lowercase();
        let after_scheme = if lower.starts_with("http://") {
            &target["http://".len()..]
        } else if lower.starts_with("https://") {
            &target["https://".len()..]
        } else {
            return None;
        };
        let path_start = after_scheme.find(|c| c == '/' || c == '?').unwrap_or(after_scheme.len());
        if path_start == 0 {
            return None; // no authority
        }
        let rest = &after_scheme[path_start..];
        Some(if rest.starts_with('/') { rest.to_string() } else { format!("/{}", rest) })
    }

    /// `Name: value` → `Header`. The name ends at the first `:`; the value
    /// is everything after it, without surrounding spaces/tabs (RFC 9112
    /// §5.1). Splitting on every ": " used to cut a value that itself
    /// contained ": " short, and `Name:value` (no space) had an empty value.
    pub fn parse_http_request_header_string(header_string: &str) -> Header {
        let line = header_string.trim_end_matches(['\r', '\n']);
        match line.split_once(':') {
            Some((name, value)) => Header {
                name: Header::name_from(name),
                value: value.trim_matches([' ', '\t']).to_string().into(),
            },
            None => Header { name: line.to_string().into(), value: String::new().into() },
        }
    }

    /// Parse the request line, headers, and body from `cursor` into `request`.
    ///
    /// Iterative: this used to recurse once per header line, so a single
    /// request with a few thousand short header lines overflowed the worker
    /// thread's stack and aborted the whole process. Errors anywhere in the
    /// head (bad UTF-8, an invalid Content-Length) are returned instead of
    /// being logged and dropped.
    pub fn cursor_read(cursor: &mut Cursor<&[u8]>, mut iteration_number: usize, request: &mut Request, content_length: usize) -> Result<bool, String> {
        let _ = content_length;
        // Each line is a slice of the input (this used to read every line
        // into a new Vec and String); only the values kept are allocated.
        let input: &[u8] = cursor.get_ref();
        loop {
            let start = (cursor.position() as usize).min(input.len());
            let rest = &input[start..];
            let line_len = rest.iter().position(|&b| b == b'\n').map_or(rest.len(), |i| i + 1);
            cursor.set_position((start + line_len) as u64);
            let string = std::str::from_utf8(&rest[..line_len]).map_err(|e| e.to_string())?;

            let is_first_iteration = iteration_number == 0;
            if is_first_iteration {
                let (method, request_uri, http_version) =
                    Request::parse_method_and_request_uri_and_http_version_string(string)?;
                request.method = method;
                request.request_uri = request_uri;
                request.http_version = http_version;
            }

            // Blank line (end of the head) or end of input.
            if line_len == 0 || string.trim().is_empty() {
                break;
            }

            // The request line is not a header. (This used to push an empty
            // `Header { name: "", value: "" }` for it, which the reverse proxy
            // forwarded as a malformed ": " line — nginx answers 400.)
            if !is_first_iteration {
                let header = Request::parse_http_request_header_string(string);
                if header.name.eq_ignore_ascii_case(Header::_CONTENT_LENGTH) {
                    // A client-supplied value: reject garbage (RFC 9112 §6.3) instead of panicking.
                    let _: usize = header.value.trim().parse()
                        .map_err(|_| format!("invalid Content-Length: {:?}", header.value))?;
                }
                request.headers.push(header);
            }
            iteration_number += 1;
        }

        // remaining part is request body
        let start = (cursor.position() as usize).min(input.len());
        request.body.extend_from_slice(&input[start..]);
        cursor.set_position(input.len() as u64);

        Ok(true)
    }

}

impl std::fmt::Display for Request {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "Request method {} and request uri {} and http_version {}", self.method, self.request_uri, self.http_version)
    }
}