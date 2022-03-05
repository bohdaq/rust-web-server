use std::borrow::Borrow;
use std::{env, fs};
use std::fs::File;
use std::io::{BufReader, Read, Write};

use std::cmp::min;

use crate::constant::{HTTP_HEADERS, HTTP_VERSIONS, REQUEST_METHODS, RESPONSE_STATUS_CODE_REASON_PHRASES};
use crate::{CONSTANTS, Request, Response, Server};
use crate::header::Header;
use crate::mime_type::MimeType;
use crate::range::Range;

pub struct MockTcpStream {
    pub(crate) read_data: Vec<u8>,
    pub(crate) write_data: Vec<u8>,
}

impl Read for MockTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let size : usize = min(self.read_data.len(), buf.len());
        buf[..size].copy_from_slice(&self.read_data[..size]);
        Ok(size)
    }
}

impl Write for MockTcpStream {

    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.write_data = Vec::from(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Unpin for MockTcpStream {}

#[test]
fn it_generates_successful_response_with_index_html() {
    // request test data
    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let request_method = REQUEST_METHODS.GET;
    let request_uri = CONSTANTS.SLASH;
    let request_http_version = HTTP_VERSIONS.HTTP_VERSION_1_1.to_string();


    // request part
    let host = Header {
        header_name: request_host_header_name.to_string(),
        header_value: request_host_header_value.to_string()
    };

    let headers = vec![host];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes());
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.header_value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = HTTP_VERSIONS.HTTP_VERSION_1_1.to_string();
    let response_status_code = RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.STATUS_CODE;
    let response_reason_phrase = RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.REASON_PHRASE;
    let response_filepath = "index.html";
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = response_html_file.len().to_string();

    let ip_addr= "127.0.0.1".to_string();
    let port : usize = "8787".parse().unwrap();
    let static_directories = vec!["/static".to_string()];

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream);
    let response = Response::parse_response(raw_response.borrow());
    let header = response.get_header(response_content_length_header_name.to_string()).unwrap();

    let content_type_header = response.get_header(HTTP_HEADERS.CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response.get_header(HTTP_HEADERS.X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(CONSTANTS.NOSNIFF, x_content_type_options_header.header_value);
    assert_eq!(MimeType::TEXT_HTML, content_type_header.header_value);
    assert_eq!(response_content_length_header_value, header.header_value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(response_html_file.as_bytes().to_vec(), response.content_range_list.get(0).unwrap().body);
}

#[test]
fn it_generates_successful_response_with_static_file() {
    // request test data
    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let request_method = REQUEST_METHODS.GET;
    let request_uri = "/static/test.txt";
    let request_http_version = HTTP_VERSIONS.HTTP_VERSION_1_1.to_string();


    // request part
    let host = Header {
        header_name: request_host_header_name.to_string(),
        header_value: request_host_header_value.to_string()
    };

    let headers = vec![host];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes());
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.header_value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = HTTP_VERSIONS.HTTP_VERSION_1_1.to_string();
    let response_status_code = RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.STATUS_CODE;
    let response_reason_phrase = RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.REASON_PHRASE;
    let response_filepath = &request.request_uri;

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();

    let response_filepath = [working_directory, request.request_uri.as_str()].join(CONSTANTS.EMPTY_STRING);
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = response_html_file.len().to_string();

    let ip_addr= "127.0.0.1".to_string();
    let port : usize = "8787".parse().unwrap();
    let static_directories = vec!["/static".to_string()];

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream);
    let response = Response::parse_response(raw_response.borrow());
    let header = response.get_header(response_content_length_header_name.to_string()).unwrap();

    let content_type_header = response.get_header(HTTP_HEADERS.CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response.get_header(HTTP_HEADERS.X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(CONSTANTS.NOSNIFF, x_content_type_options_header.header_value);
    assert_eq!(MimeType::TEXT_PLAIN, content_type_header.header_value);

    assert_eq!(response_content_length_header_value, header.header_value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(response_html_file.into_bytes(), response.content_range_list.get(0).unwrap().body);
}

#[test]
fn it_generates_not_found_page_for_absent_static_file() {
    // request test data
    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let request_method = REQUEST_METHODS.GET;
    let request_uri = "/static/nonexistingfile";
    let request_http_version = HTTP_VERSIONS.HTTP_VERSION_1_1.to_string();


    // request part
    let host = Header {
        header_name: request_host_header_name.to_string(),
        header_value: request_host_header_value.to_string()
    };

    let headers = vec![host];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes());
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.header_value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = HTTP_VERSIONS.HTTP_VERSION_1_1;
    let response_status_code = RESPONSE_STATUS_CODE_REASON_PHRASES.N404_NOT_FOUND.STATUS_CODE;
    let response_reason_phrase = RESPONSE_STATUS_CODE_REASON_PHRASES.N404_NOT_FOUND.REASON_PHRASE;
    let response_filepath = &request.request_uri;

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();
    let not_found_page_path = "404.html";

    let response_filepath = [working_directory, CONSTANTS.SLASH, not_found_page_path].join(CONSTANTS.EMPTY_STRING);
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = response_html_file.len().to_string();

    let ip_addr= "127.0.0.1".to_string();
    let port: usize = "8787".parse().unwrap();
    let static_directories = vec!["/static".to_string()];

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream);
    let response = Response::parse_response(raw_response.borrow());
    let header = response.get_header(response_content_length_header_name.to_string()).unwrap();

    let content_type_header = response.get_header(HTTP_HEADERS.CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response.get_header(HTTP_HEADERS.X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(CONSTANTS.NOSNIFF, x_content_type_options_header.header_value);
    assert_eq!(MimeType::TEXT_HTML, content_type_header.header_value);

    assert_eq!(response_content_length_header_value, header.header_value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(response_html_file.as_bytes().to_vec(), response.content_range_list.get(0).unwrap().body);
}

#[test]
fn it_generates_not_found_page_for_absent_route() {
    // request test data
    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let request_method = REQUEST_METHODS.GET;
    let request_uri = "/nonexistingroute";
    let request_http_version = HTTP_VERSIONS.HTTP_VERSION_1_1.to_string();


    // request part
    let host = Header {
        header_name: request_host_header_name.to_string(),
        header_value: request_host_header_value.to_string()
    };

    let headers = vec![host];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes());
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.header_value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = HTTP_VERSIONS.HTTP_VERSION_1_1.to_string();
    let response_status_code = RESPONSE_STATUS_CODE_REASON_PHRASES.N404_NOT_FOUND.STATUS_CODE;
    let response_reason_phrase = RESPONSE_STATUS_CODE_REASON_PHRASES.N404_NOT_FOUND.REASON_PHRASE;
    let response_filepath = &request.request_uri;

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();
    let not_found_page_path = "404.html";

    let response_filepath = [working_directory, CONSTANTS.SLASH, not_found_page_path].join(CONSTANTS.EMPTY_STRING);
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = response_html_file.len().to_string();

    let ip_addr= "127.0.0.1".to_string();
    let port : usize = "8787".parse().unwrap();
    let static_directories = vec!["/static".to_string()];

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream);
    let response = Response::parse_response(raw_response.borrow());
    let header = response.get_header(response_content_length_header_name.to_string()).unwrap();

    let content_type_header = response.get_header(HTTP_HEADERS.CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response.get_header(HTTP_HEADERS.X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(CONSTANTS.NOSNIFF, x_content_type_options_header.header_value);
    assert_eq!(MimeType::TEXT_HTML, content_type_header.header_value);

    assert_eq!(response_content_length_header_value, header.header_value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(response_html_file.into_bytes(), response.content_range_list.get(0).unwrap().body);
}

#[test]
fn it_generates_not_found_page_for_static_directory() {
    // request test data
    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let request_method = REQUEST_METHODS.GET;
    let request_uri = "/static/";
    let request_http_version = HTTP_VERSIONS.HTTP_VERSION_1_1.to_string();


    // request part
    let host = Header {
        header_name: request_host_header_name.to_string(),
        header_value: request_host_header_value.to_string()
    };

    let headers = vec![host];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes());
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.header_value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = HTTP_VERSIONS.HTTP_VERSION_1_1.to_string();
    let response_status_code = RESPONSE_STATUS_CODE_REASON_PHRASES.N404_NOT_FOUND.STATUS_CODE;
    let response_reason_phrase = RESPONSE_STATUS_CODE_REASON_PHRASES.N404_NOT_FOUND.REASON_PHRASE;
    let response_filepath = &request.request_uri;

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();
    let not_found_page_path = "404.html";

    let response_filepath = [working_directory, CONSTANTS.SLASH, not_found_page_path].join(CONSTANTS.EMPTY_STRING);
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = response_html_file.len().to_string();

    let ip_addr= "127.0.0.1".to_string();
    let port : usize = "8787".parse().unwrap();
    let static_directories = vec!["/static".to_string()];

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream);
    let response = Response::parse_response(raw_response.borrow());
    let header = response.get_header(response_content_length_header_name.to_string()).unwrap();

    let content_type_header = response.get_header(HTTP_HEADERS.CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response.get_header(HTTP_HEADERS.X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(CONSTANTS.NOSNIFF, x_content_type_options_header.header_value);
    assert_eq!(MimeType::TEXT_HTML, content_type_header.header_value);

    assert_eq!(response_content_length_header_value, header.header_value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(response_html_file.into_bytes(), response.content_range_list.get(0).unwrap().body);
}

#[test]
fn it_generates_not_found_page_for_static_subdirectory() {
    // request test data
    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let request_method = REQUEST_METHODS.GET;
    let request_uri = "/static/subdir/";
    let request_http_version = HTTP_VERSIONS.HTTP_VERSION_1_1.to_string();


    // request part
    let host = Header {
        header_name: request_host_header_name.to_string(),
        header_value: request_host_header_value.to_string()
    };

    let headers = vec![host];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes());
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.header_value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = HTTP_VERSIONS.HTTP_VERSION_1_1.to_string();
    let response_status_code = RESPONSE_STATUS_CODE_REASON_PHRASES.N404_NOT_FOUND.STATUS_CODE;
    let response_reason_phrase = RESPONSE_STATUS_CODE_REASON_PHRASES.N404_NOT_FOUND.REASON_PHRASE;
    let response_filepath = &request.request_uri;

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();
    let not_found_page_path = "404.html";

    let response_filepath = [working_directory, CONSTANTS.SLASH, not_found_page_path].join(CONSTANTS.EMPTY_STRING);
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = response_html_file.len().to_string();

    let ip_addr= "127.0.0.1".to_string();
    let port : usize = "8787".parse().unwrap();
    let static_directories = vec!["/static".to_string()];

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream);
    let response = Response::parse_response(raw_response.borrow());
    let header = response.get_header(response_content_length_header_name.to_string()).unwrap();

    let content_type_header = response.get_header(HTTP_HEADERS.CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response.get_header(HTTP_HEADERS.X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(CONSTANTS.NOSNIFF, x_content_type_options_header.header_value);
    assert_eq!(MimeType::TEXT_HTML, content_type_header.header_value);

    assert_eq!(response_content_length_header_value, header.header_value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(response_html_file.into_bytes(), response.content_range_list.get(0).unwrap().body);
}

#[test]
fn it_generates_successful_response_with_static_file_in_subdirectory() {
    // request test data
    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let request_method = REQUEST_METHODS.GET;
    let request_uri = "/static/test.txt";
    let request_http_version = HTTP_VERSIONS.HTTP_VERSION_1_1.to_string();


    // request part
    let host = Header {
        header_name: request_host_header_name.to_string(),
        header_value: request_host_header_value.to_string()
    };

    let headers = vec![host];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes());
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.header_value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = HTTP_VERSIONS.HTTP_VERSION_1_1.to_string();
    let response_status_code = RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.STATUS_CODE;
    let response_reason_phrase = RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.REASON_PHRASE;
    let response_filepath = &request.request_uri;

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();

    let response_filepath = [working_directory, request.request_uri.as_str()].join(CONSTANTS.EMPTY_STRING);
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = response_html_file.len().to_string();

    let ip_addr= "127.0.0.1".to_string();
    let port : usize = "8787".parse().unwrap();
    let static_directories = vec!["/static".to_string()];


    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream);
    let response = Response::parse_response(raw_response.borrow());
    let header = response.get_header(response_content_length_header_name.to_string()).unwrap();

    let content_type_header = response.get_header(HTTP_HEADERS.CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response.get_header(HTTP_HEADERS.X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(CONSTANTS.NOSNIFF, x_content_type_options_header.header_value);
    assert_eq!(MimeType::TEXT_PLAIN, content_type_header.header_value);

    assert_eq!(response_content_length_header_value, header.header_value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(response_html_file.into_bytes(), response.content_range_list.get(0).unwrap().body);
}

#[test]
fn it_generates_successful_response_with_static_file_in_multiple_static_directories() {

    // 1st reading file from /static folder

    // request test data
    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let request_method = REQUEST_METHODS.GET;
    let request_uri = "/static/test.txt";
    let request_http_version = HTTP_VERSIONS.HTTP_VERSION_1_1.to_string();


    // request part
    let host = Header {
        header_name: request_host_header_name.to_string(),
        header_value: request_host_header_value.to_string()
    };

    let headers = vec![host];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes());
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.header_value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = HTTP_VERSIONS.HTTP_VERSION_1_1.to_string();
    let response_status_code = RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.STATUS_CODE;
    let response_reason_phrase = RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.REASON_PHRASE;
    let response_filepath = &request.request_uri;

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();

    let response_filepath = [working_directory, request.request_uri.as_str()].join(CONSTANTS.EMPTY_STRING);
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = response_html_file.len().to_string();

    let ip_addr= "127.0.0.1".to_string();
    let port : usize = "8787".parse().unwrap();
    let static_directories = vec!["/static".to_string(), "/assets".to_string()];

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream);
    let response = Response::parse_response(raw_response.borrow());
    let header = response.get_header(response_content_length_header_name.to_string()).unwrap();

    let content_type_header = response.get_header(HTTP_HEADERS.CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response.get_header(HTTP_HEADERS.X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(CONSTANTS.NOSNIFF, x_content_type_options_header.header_value);
    assert_eq!(MimeType::TEXT_PLAIN, content_type_header.header_value);

    assert_eq!(response_content_length_header_value, header.header_value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(response_html_file.into_bytes(), response.content_range_list.get(0).unwrap().body);






    // 2nd file read from /assets directory

    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let request_method = REQUEST_METHODS.GET;
    let request_uri = "/assets/test.txt";
    let request_http_version = HTTP_VERSIONS.HTTP_VERSION_1_1.to_string();


    // request part
    let host = Header {
        header_name: request_host_header_name.to_string(),
        header_value: request_host_header_value.to_string()
    };

    let headers = vec![host];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes());
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.header_value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = HTTP_VERSIONS.HTTP_VERSION_1_1.to_string();
    let response_status_code = RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.STATUS_CODE;
    let response_reason_phrase = RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.REASON_PHRASE;
    let response_filepath = &request.request_uri;

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();

    let response_filepath = [working_directory, request.request_uri.as_str()].join(CONSTANTS.EMPTY_STRING);
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = response_html_file.len().to_string();

    let ip_addr= "127.0.0.1".to_string();
    let port : usize = "8787".parse().unwrap();
    let static_directories = vec!["/static".to_string(), "/assets".to_string()];

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream);
    let response = Response::parse_response(raw_response.borrow());
    let header = response.get_header(response_content_length_header_name.to_string()).unwrap();

    let content_type_header = response.get_header(HTTP_HEADERS.CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response.get_header(HTTP_HEADERS.X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(CONSTANTS.NOSNIFF, x_content_type_options_header.header_value);
    assert_eq!(MimeType::TEXT_PLAIN, content_type_header.header_value);

    assert_eq!(response_content_length_header_value, header.header_value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(response_html_file.into_bytes(), response.content_range_list.get(0).unwrap().body);
}

#[test]
fn check_range_response_for_not_proper_range_header() {
    let uri = "/static/test.txt";
    let url = Server::get_static_filepath(uri);

    let file = File::open(url).unwrap();
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();

    reader.read_to_end(&mut buffer).unwrap();

    let length = buffer.len();
    let mid = length / 2;
    let end_of_first_range = mid;
    let start_of_second_range = mid + 1;
    let not_proper_end_of_second_range = length + 10;

    let range_header_value = format!("bytdgdes=0-{}, {}-{}", end_of_first_range, start_of_second_range, not_proper_end_of_second_range);

    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let host = Header {
        header_name: request_host_header_name.to_string(),
        header_value: request_host_header_value.to_string()
    };

    let range = Header {
        header_name: HTTP_HEADERS.RANGE.to_string(),
        header_value: range_header_value.to_string()
    };

    let headers = vec![host, range];
    let request = Request {
        method: REQUEST_METHODS.GET.to_string(),
        request_uri: uri.to_string(),
        http_version: HTTP_VERSIONS.HTTP_VERSION_1_1.to_string(),
        headers
    };

    let raw_request = Request::generate_request(request);
    let request: Request = Request::parse_request(&raw_request.as_bytes());
    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream);

    let response = Response::parse_response(raw_response.borrow());

    let response_string = String::from_utf8(raw_response).unwrap();
    println!("\n\n\n{}", &raw_request);
    println!("\n\n\n{}", &response_string);

    assert_eq!(HTTP_VERSIONS.HTTP_VERSION_1_1, response.http_version);
    let header = response.get_header(HTTP_HEADERS.X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();
    assert_eq!(CONSTANTS.NOSNIFF, header.header_value);
    let header = response.get_header(HTTP_HEADERS.ACCEPT_RANGES.to_string()).unwrap();
    assert_eq!(CONSTANTS.BYTES, header.header_value);

    assert_eq!(RESPONSE_STATUS_CODE_REASON_PHRASES.N416_RANGE_NOT_SATISFIABLE.STATUS_CODE, response.status_code);
    assert_eq!(RESPONSE_STATUS_CODE_REASON_PHRASES.N416_RANGE_NOT_SATISFIABLE.REASON_PHRASE, response.reason_phrase);

    let content_range = response.content_range_list.get(0).unwrap();
    assert_eq!(content_range.body, Range::ERROR_MALFORMED_RANGE_HEADER_WRONG_UNIT.as_bytes());
}

#[test]
fn check_range_response_for_not_proper_range_header_range_end_bigger_than_filesize() {
    let uri = "/static/test.txt";
    let url = Server::get_static_filepath(uri);

    let file = File::open(url).unwrap();
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();

    reader.read_to_end(&mut buffer).unwrap();

    let length = buffer.len();
    let mid = length / 2;
    let end_of_first_range = mid;
    let start_of_second_range = mid + 1;
    let not_proper_end_of_second_range = length + 10;

    let range_header_value = format!("bytes=0-{}, {}-{}", end_of_first_range, start_of_second_range, not_proper_end_of_second_range);

    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let host = Header {
        header_name: request_host_header_name.to_string(),
        header_value: request_host_header_value.to_string()
    };

    let range = Header {
        header_name: HTTP_HEADERS.RANGE.to_string(),
        header_value: range_header_value.to_string()
    };

    let headers = vec![host, range];
    let request = Request {
        method: REQUEST_METHODS.GET.to_string(),
        request_uri: uri.to_string(),
        http_version: HTTP_VERSIONS.HTTP_VERSION_1_1.to_string(),
        headers
    };

    let raw_request = Request::generate_request(request);
    let request: Request = Request::parse_request(&raw_request.as_bytes());
    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream);

    let response = Response::parse_response(raw_response.borrow());

    let response_string = String::from_utf8(raw_response).unwrap();
    println!("\n\n\n{}", &raw_request);
    println!("\n\n\n{}", &response_string);

    assert_eq!(HTTP_VERSIONS.HTTP_VERSION_1_1, response.http_version);
    let header = response.get_header(HTTP_HEADERS.X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();
    assert_eq!(CONSTANTS.NOSNIFF, header.header_value);
    let header = response.get_header(HTTP_HEADERS.ACCEPT_RANGES.to_string()).unwrap();
    assert_eq!(CONSTANTS.BYTES, header.header_value);

    assert_eq!(RESPONSE_STATUS_CODE_REASON_PHRASES.N416_RANGE_NOT_SATISFIABLE.STATUS_CODE, response.status_code);
    assert_eq!(RESPONSE_STATUS_CODE_REASON_PHRASES.N416_RANGE_NOT_SATISFIABLE.REASON_PHRASE, response.reason_phrase);

    let content_range = response.content_range_list.get(0).unwrap();
    assert_eq!(content_range.body, Range::ERROR_END_IS_BIGGER_THAN_FILESIZE_CONTENT_RANGE.as_bytes());
}

#[test]
fn check_range_response_for_not_proper_range_header_range_start_bigger_than_end() {
    let uri = "/static/test.txt";
    let url = Server::get_static_filepath(uri);

    let file = File::open(url).unwrap();
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();

    reader.read_to_end(&mut buffer).unwrap();

    let length = buffer.len();
    let mid = length / 2;
    let end_of_first_range = mid;
    let start_of_second_range = length;
    let not_proper_end_of_second_range = mid;

    let range_header_value = format!("bytes=0-{}, {}-{}", end_of_first_range, start_of_second_range, not_proper_end_of_second_range);

    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let host = Header {
        header_name: request_host_header_name.to_string(),
        header_value: request_host_header_value.to_string()
    };

    let range = Header {
        header_name: HTTP_HEADERS.RANGE.to_string(),
        header_value: range_header_value.to_string()
    };

    let headers = vec![host, range];
    let request = Request {
        method: REQUEST_METHODS.GET.to_string(),
        request_uri: uri.to_string(),
        http_version: HTTP_VERSIONS.HTTP_VERSION_1_1.to_string(),
        headers
    };

    let raw_request = Request::generate_request(request);
    let request: Request = Request::parse_request(&raw_request.as_bytes());
    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream);

    let response = Response::parse_response(raw_response.borrow());

    let response_string = String::from_utf8(raw_response).unwrap();
    println!("\n\n\n{}", &raw_request);
    println!("\n\n\n{}", &response_string);

    assert_eq!(HTTP_VERSIONS.HTTP_VERSION_1_1, response.http_version);
    let header = response.get_header(HTTP_HEADERS.X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();
    assert_eq!(CONSTANTS.NOSNIFF, header.header_value);
    let header = response.get_header(HTTP_HEADERS.ACCEPT_RANGES.to_string()).unwrap();
    assert_eq!(CONSTANTS.BYTES, header.header_value);

    assert_eq!(RESPONSE_STATUS_CODE_REASON_PHRASES.N416_RANGE_NOT_SATISFIABLE.STATUS_CODE, response.status_code);
    assert_eq!(RESPONSE_STATUS_CODE_REASON_PHRASES.N416_RANGE_NOT_SATISFIABLE.REASON_PHRASE, response.reason_phrase);

    let content_range = response.content_range_list.get(0).unwrap();
    assert_eq!(content_range.body, Range::ERROR_START_IS_AFTER_END_CONTENT_RANGE.as_bytes());
}

#[test]
fn check_range_response_for_not_proper_range_header_range_start_malformed() {
    let uri = "/static/test.txt";
    let url = Server::get_static_filepath(uri);

    let file = File::open(url).unwrap();
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();

    reader.read_to_end(&mut buffer).unwrap();

    let length = buffer.len();
    let mid = length / 2;
    let end_of_first_range = mid;
    let start_of_second_range = length + 10;
    let not_proper_end_of_second_range = mid;

    let range_header_value = format!("bytes=0-{}zaksd, {}-{}", end_of_first_range, start_of_second_range, not_proper_end_of_second_range);

    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let host = Header {
        header_name: request_host_header_name.to_string(),
        header_value: request_host_header_value.to_string()
    };

    let range = Header {
        header_name: HTTP_HEADERS.RANGE.to_string(),
        header_value: range_header_value.to_string()
    };

    let headers = vec![host, range];
    let request = Request {
        method: REQUEST_METHODS.GET.to_string(),
        request_uri: uri.to_string(),
        http_version: HTTP_VERSIONS.HTTP_VERSION_1_1.to_string(),
        headers
    };

    let raw_request = Request::generate_request(request);
    let request: Request = Request::parse_request(&raw_request.as_bytes());
    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream);

    let response = Response::parse_response(raw_response.borrow());

    let response_string = String::from_utf8(raw_response).unwrap();
    println!("\n\n\n{}", &raw_request);
    println!("\n\n\n{}", &response_string);

    assert_eq!(HTTP_VERSIONS.HTTP_VERSION_1_1, response.http_version);
    let header = response.get_header(HTTP_HEADERS.X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();
    assert_eq!(CONSTANTS.NOSNIFF, header.header_value);
    let header = response.get_header(HTTP_HEADERS.ACCEPT_RANGES.to_string()).unwrap();
    assert_eq!(CONSTANTS.BYTES, header.header_value);

    assert_eq!(RESPONSE_STATUS_CODE_REASON_PHRASES.N416_RANGE_NOT_SATISFIABLE.STATUS_CODE, response.status_code);
    assert_eq!(RESPONSE_STATUS_CODE_REASON_PHRASES.N416_RANGE_NOT_SATISFIABLE.REASON_PHRASE, response.reason_phrase);

    let content_range = response.content_range_list.get(0).unwrap();
    assert_eq!(content_range.body, Range::ERROR_UNABLE_TO_PARSE_RANGE_END.as_bytes());
}

#[test]
fn check_range_response_for_not_proper_range_header_range_end_malformed() {
    let uri = "/static/test.txt";
    let url = Server::get_static_filepath(uri);

    let file = File::open(url).unwrap();
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();

    reader.read_to_end(&mut buffer).unwrap();

    let length = buffer.len();
    let mid = length / 2;
    let end_of_first_range = mid;
    let start_of_second_range = length + 10;
    let not_proper_end_of_second_range = mid;

    let range_header_value = format!("bytes=0-{}zaksd, {}-{}", end_of_first_range, start_of_second_range, not_proper_end_of_second_range);

    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let host = Header {
        header_name: request_host_header_name.to_string(),
        header_value: request_host_header_value.to_string()
    };

    let range = Header {
        header_name: HTTP_HEADERS.RANGE.to_string(),
        header_value: range_header_value.to_string()
    };

    let headers = vec![host, range];
    let request = Request {
        method: REQUEST_METHODS.GET.to_string(),
        request_uri: uri.to_string(),
        http_version: HTTP_VERSIONS.HTTP_VERSION_1_1.to_string(),
        headers
    };

    let raw_request = Request::generate_request(request);
    let request: Request = Request::parse_request(&raw_request.as_bytes());
    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream);

    let response = Response::parse_response(raw_response.borrow());

    let response_string = String::from_utf8(raw_response).unwrap();
    println!("\n\n\n{}", &raw_request);
    println!("\n\n\n{}", &response_string);

    assert_eq!(HTTP_VERSIONS.HTTP_VERSION_1_1, response.http_version);
    let header = response.get_header(HTTP_HEADERS.X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();
    assert_eq!(CONSTANTS.NOSNIFF, header.header_value);
    let header = response.get_header(HTTP_HEADERS.ACCEPT_RANGES.to_string()).unwrap();
    assert_eq!(CONSTANTS.BYTES, header.header_value);

    assert_eq!(RESPONSE_STATUS_CODE_REASON_PHRASES.N416_RANGE_NOT_SATISFIABLE.STATUS_CODE, response.status_code);
    assert_eq!(RESPONSE_STATUS_CODE_REASON_PHRASES.N416_RANGE_NOT_SATISFIABLE.REASON_PHRASE, response.reason_phrase);

    let content_range = response.content_range_list.get(0).unwrap();
    assert_eq!(content_range.body, Range::ERROR_UNABLE_TO_PARSE_RANGE_END.as_bytes());
}