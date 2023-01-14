use std::borrow::Borrow;
use std::{env, fs};
use std::fs::File;
use std::io::{BufReader, Read, Write};

use std::cmp::min;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use crate::entry_point::config_file::override_environment_variables_from_config;
use file_ext::FileExt;

use crate::header::Header;
use crate::http::VERSION;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::Server;
use crate::symbol::SYMBOL;

pub struct MockTcpStream {
    pub read_data: Vec<u8>,
    pub write_data: Vec<u8>,
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
    let request_method = METHOD.get;
    let request_uri = SYMBOL.slash;
    let request_http_version = VERSION.http_1_1.to_string();


    // request part
    let host = Header {
        name: request_host_header_name.to_string(),
        value: request_host_header_value.to_string()
    };

    let headers = vec![host];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::_generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes()).unwrap();
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = VERSION.http_1_1.to_string();
    let response_status_code = STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    let response_reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase;
    let response_filepath = "index.html";
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = response_html_file.len().to_string();

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream, peer_addr);
    let response = Response::_parse_response(raw_response.borrow());
    let header = response._get_header(response_content_length_header_name.to_string()).unwrap();

    let content_type_header = response._get_header(Header::_CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response._get_header(Header::_X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF, x_content_type_options_header.value);
    assert_eq!(MimeType::TEXT_HTML, content_type_header.value);
    assert_eq!(response_content_length_header_value, header.value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(*response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(response_html_file.as_bytes().to_vec(), response.content_range_list.get(0).unwrap().body);
}

#[test]
fn it_generates_successful_response_with_index_html_as_symlink() {
    // request test data
    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let request_method = METHOD.get;
    let request_uri = [SYMBOL.slash, "index_rewrite"].join("");
    let request_http_version = VERSION.http_1_1.to_string();


    // request part
    let host = Header {
        name: request_host_header_name.to_string(),
        value: request_host_header_value.to_string()
    };

    let headers = vec![host];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::_generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes()).unwrap();
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = VERSION.http_1_1.to_string();
    let response_status_code = STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    let response_reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase;
    let response_filepath = "index.html";
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = response_html_file.len().to_string();

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream, peer_addr);
    let response = Response::_parse_response(raw_response.borrow());
    let header = response._get_header(response_content_length_header_name.to_string()).unwrap();

    let content_type_header = response._get_header(Header::_CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response._get_header(Header::_X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF, x_content_type_options_header.value);
    assert_eq!(MimeType::TEXT_HTML, content_type_header.value);
    assert_eq!(response_content_length_header_value, header.value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(*response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(response_html_file.as_bytes().to_vec(), response.content_range_list.get(0).unwrap().body);
}

#[test]
fn it_generates_successful_response_with_static_file() {
    override_environment_variables_from_config(Some("/src/test/rws.config.toml"));

    // request test data
    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let request_method = METHOD.get;
    let request_uri = "/static/test.txt";
    let request_http_version = VERSION.http_1_1.to_string();


    // request part
    let host = Header {
        name: request_host_header_name.to_string(),
        value: request_host_header_value.to_string()
    };

    let headers = vec![host];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::_generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes()).unwrap();
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = VERSION.http_1_1.to_string();
    let response_status_code = STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    let response_reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase;

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();

    let response_filepath = [working_directory, request.request_uri.as_str()].join(SYMBOL.empty_string);
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = response_html_file.len().to_string();

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };

    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream, peer_addr);
    let response = Response::_parse_response(raw_response.borrow());
    let header = response._get_header(response_content_length_header_name.to_string()).unwrap();

    let content_type_header = response._get_header(Header::_CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response._get_header(Header::_X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF, x_content_type_options_header.value);
    assert_eq!(MimeType::TEXT_PLAIN, content_type_header.value);

    assert_eq!(response_content_length_header_value, header.value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(*response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(response_html_file.into_bytes(), response.content_range_list.get(0).unwrap().body);
}

#[test]
fn it_generates_not_found_page_for_absent_static_file() {
    // request test data
    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let request_method = METHOD.get;
    let request_uri = "/static/nonexistingfile";
    let request_http_version = VERSION.http_1_1.to_string();


    // request part
    let host = Header {
        name: request_host_header_name.to_string(),
        value: request_host_header_value.to_string()
    };

    let headers = vec![host];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::_generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes()).unwrap();
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = VERSION.http_1_1;
    let response_status_code = STATUS_CODE_REASON_PHRASE.n404_not_found.status_code;
    let response_reason_phrase = STATUS_CODE_REASON_PHRASE.n404_not_found.reason_phrase;

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();
    let not_found_page_path = "404.html";

    let response_filepath = [working_directory, SYMBOL.slash, not_found_page_path].join(SYMBOL.empty_string);
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = response_html_file.len().to_string();

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream, peer_addr);
    let response = Response::_parse_response(raw_response.borrow());
    let header = response._get_header(response_content_length_header_name.to_string()).unwrap();

    let content_type_header = response._get_header(Header::_CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response._get_header(Header::_X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF, x_content_type_options_header.value);
    assert_eq!(MimeType::TEXT_HTML, content_type_header.value);

    assert_eq!(response_content_length_header_value, header.value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(*response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(response_html_file.as_bytes().to_vec(), response.content_range_list.get(0).unwrap().body);
}

#[test]
fn it_generates_not_found_page_for_absent_route() {
    // request test data
    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let request_method = METHOD.get;
    let request_uri = "/nonexistingroute";
    let request_http_version = VERSION.http_1_1.to_string();


    // request part
    let host = Header {
        name: request_host_header_name.to_string(),
        value: request_host_header_value.to_string()
    };

    let headers = vec![host];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::_generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes()).unwrap();
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = VERSION.http_1_1.to_string();
    let response_status_code = STATUS_CODE_REASON_PHRASE.n404_not_found.status_code;
    let response_reason_phrase = STATUS_CODE_REASON_PHRASE.n404_not_found.reason_phrase;

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();
    let not_found_page_path = "404.html";

    let response_filepath = [working_directory, SYMBOL.slash, not_found_page_path].join(SYMBOL.empty_string);
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = response_html_file.len().to_string();

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream, peer_addr);
    let response = Response::_parse_response(raw_response.borrow());
    let header = response._get_header(response_content_length_header_name.to_string()).unwrap();

    let content_type_header = response._get_header(Header::_CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response._get_header(Header::_X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF, x_content_type_options_header.value);
    assert_eq!(MimeType::TEXT_HTML, content_type_header.value);

    assert_eq!(response_content_length_header_value, header.value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(*response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(response_html_file.into_bytes(), response.content_range_list.get(0).unwrap().body);
}

#[test]
fn it_generates_not_found_page_for_static_directory() {
    // request test data
    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let request_method = METHOD.get;
    let request_uri = "/static/";
    let request_http_version = VERSION.http_1_1.to_string();


    // request part
    let host = Header {
        name: request_host_header_name.to_string(),
        value: request_host_header_value.to_string()
    };

    let headers = vec![host];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::_generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes()).unwrap();
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = VERSION.http_1_1.to_string();
    let response_status_code = STATUS_CODE_REASON_PHRASE.n404_not_found.status_code;
    let response_reason_phrase = STATUS_CODE_REASON_PHRASE.n404_not_found.reason_phrase;

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();
    let not_found_page_path = "404.html";

    let response_filepath = [working_directory, SYMBOL.slash, not_found_page_path].join(SYMBOL.empty_string);
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = response_html_file.len().to_string();

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream, peer_addr);
    let response = Response::_parse_response(raw_response.borrow());
    let header = response._get_header(response_content_length_header_name.to_string()).unwrap();

    let content_type_header = response._get_header(Header::_CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response._get_header(Header::_X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF, x_content_type_options_header.value);
    assert_eq!(MimeType::TEXT_HTML, content_type_header.value);

    assert_eq!(response_content_length_header_value, header.value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(*response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(response_html_file.into_bytes(), response.content_range_list.get(0).unwrap().body);
}

#[test]
fn it_generates_not_found_page_for_static_subdirectory() {
    // request test data
    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let request_method = METHOD.get;
    let request_uri = "/static/subdir/";
    let request_http_version = VERSION.http_1_1.to_string();


    // request part
    let host = Header {
        name: request_host_header_name.to_string(),
        value: request_host_header_value.to_string()
    };

    let headers = vec![host];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::_generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes()).unwrap();
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = VERSION.http_1_1.to_string();
    let response_status_code = STATUS_CODE_REASON_PHRASE.n404_not_found.status_code;
    let response_reason_phrase = STATUS_CODE_REASON_PHRASE.n404_not_found.reason_phrase;

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();
    let not_found_page_path = "404.html";

    let response_filepath = [working_directory, SYMBOL.slash, not_found_page_path].join(SYMBOL.empty_string);
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = response_html_file.len().to_string();

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream, peer_addr);
    let response = Response::_parse_response(raw_response.borrow());
    let header = response._get_header(response_content_length_header_name.to_string()).unwrap();

    let content_type_header = response._get_header(Header::_CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response._get_header(Header::_X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF, x_content_type_options_header.value);
    assert_eq!(MimeType::TEXT_HTML, content_type_header.value);

    assert_eq!(response_content_length_header_value, header.value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(*response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(response_html_file.into_bytes(), response.content_range_list.get(0).unwrap().body);
}

#[test]
fn it_generates_bad_request_for_non_ut8_char_in_request() {
    // request test data
    let mut non_utf8_char: Vec<u8> = vec![255];

    let method = METHOD.get.to_lowercase();
    let request_uri = "/path";
    let http_version = VERSION.http_1_1.to_lowercase();

    let mut request_vec : Vec<u8> = vec![];
    request_vec.append(&mut method.as_bytes().to_vec());
    request_vec.append(&mut request_uri.as_bytes().to_vec());
    request_vec.append(&mut non_utf8_char);
    request_vec.append(&mut http_version.as_bytes().to_vec());

    let mock_tcp_stream = MockTcpStream {
        read_data: request_vec,
        write_data: vec![],
    };
    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream, peer_addr);
    let str = String::from_utf8(raw_response.clone()).unwrap();
    println!("{}", str);

    let response = Response::_parse_response(&raw_response);
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n400_bad_request.status_code, response.status_code);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n400_bad_request.reason_phrase, response.reason_phrase);
    assert_eq!(VERSION.http_1_1, response.http_version);

    let content_type_header = response._get_header(Header::_CONTENT_TYPE.to_string()).unwrap();
    assert_eq!(MimeType::TEXT_PLAIN, content_type_header.value);

    let response_body = response.content_range_list.get(0).unwrap();
    assert_eq!("invalid utf-8 sequence of 1 bytes from index 8", String::from_utf8(response_body.clone().body).unwrap());

}

#[test]
fn it_generates_successful_response_with_static_file_in_subdirectory() {
    // request test data
    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let request_method = METHOD.get;
    let request_uri = "/static/test.txt";
    let request_http_version = VERSION.http_1_1.to_string();


    // request part
    let host = Header {
        name: request_host_header_name.to_string(),
        value: request_host_header_value.to_string()
    };

    let headers = vec![host];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::_generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes()).unwrap();
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = VERSION.http_1_1.to_string();
    let response_status_code = STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    let response_reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase;

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();

    let response_filepath = [working_directory, request.request_uri.as_str()].join(SYMBOL.empty_string);
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = response_html_file.len().to_string();

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };

    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream, peer_addr);
    let response = Response::_parse_response(raw_response.borrow());
    let header = response._get_header(response_content_length_header_name.to_string()).unwrap();

    let content_type_header = response._get_header(Header::_CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response._get_header(Header::_X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF, x_content_type_options_header.value);
    assert_eq!(MimeType::TEXT_PLAIN, content_type_header.value);

    assert_eq!(response_content_length_header_value, header.value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(*response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(response_html_file.into_bytes(), response.content_range_list.get(0).unwrap().body);
}

#[test]
fn it_generates_successful_response_with_static_file_in_subdirectory_to_head_request() {
    // request test data
    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let request_method = METHOD.head;
    let request_uri = "/static/test.txt";
    let request_http_version = VERSION.http_1_1.to_string();


    // request part
    let host = Header {
        name: request_host_header_name.to_string(),
        value: request_host_header_value.to_string()
    };

    let headers = vec![host];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::_generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes()).unwrap();
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = VERSION.http_1_1.to_string();
    let response_status_code = STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    let response_reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase;

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();

    let response_filepath = [working_directory, request.request_uri.as_str()].join(SYMBOL.empty_string);
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = response_html_file.len().to_string();

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream, peer_addr);
    let response = Response::_parse_response(raw_response.borrow());
    let header = response._get_header(response_content_length_header_name.to_string()).unwrap();

    let content_type_header = response._get_header(Header::_CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response._get_header(Header::_X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF, x_content_type_options_header.value);
    assert_eq!(MimeType::TEXT_PLAIN, content_type_header.value);

    assert_eq!(response_content_length_header_value, header.value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(*response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(0, response.content_range_list.get(0).unwrap().range.end);
    assert_eq!("0", response.content_range_list.get(0).unwrap().size);
}

#[test]
fn it_generates_successful_response_with_static_file_in_multiple_static_directories() {

    // 1st reading file from /static folder

    // request test data
    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let request_method = METHOD.get;
    let request_uri = "/static/test.txt";
    let request_http_version = VERSION.http_1_1.to_string();


    // request part
    let host = Header {
        name: request_host_header_name.to_string(),
        value: request_host_header_value.to_string()
    };

    let headers = vec![host];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::_generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes()).unwrap();
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = VERSION.http_1_1.to_string();
    let response_status_code = STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    let response_reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase;

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();

    let response_filepath = [working_directory, request.request_uri.as_str()].join(SYMBOL.empty_string);
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = response_html_file.len().to_string();

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream, peer_addr);
    let response = Response::_parse_response(raw_response.borrow());
    let header = response._get_header(response_content_length_header_name.to_string()).unwrap();

    let content_type_header = response._get_header(Header::_CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response._get_header(Header::_X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF, x_content_type_options_header.value);
    assert_eq!(MimeType::TEXT_PLAIN, content_type_header.value);

    assert_eq!(response_content_length_header_value, header.value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(*response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(response_html_file.into_bytes(), response.content_range_list.get(0).unwrap().body);






    // 2nd file read from /assets directory

    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let request_method = METHOD.get;
    let request_uri = "/assets/test.txt";
    let request_http_version = VERSION.http_1_1.to_string();


    // request part
    let host = Header {
        name: request_host_header_name.to_string(),
        value: request_host_header_value.to_string()
    };

    let headers = vec![host];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::_generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes()).unwrap();
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = VERSION.http_1_1.to_string();
    let response_status_code = STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    let response_reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase;

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();

    let response_filepath = [working_directory, request.request_uri.as_str()].join(SYMBOL.empty_string);
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = response_html_file.len().to_string();

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream, peer_addr);
    let response = Response::_parse_response(raw_response.borrow());
    let header = response._get_header(response_content_length_header_name.to_string()).unwrap();

    let content_type_header = response._get_header(Header::_CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response._get_header(Header::_X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF, x_content_type_options_header.value);
    assert_eq!(MimeType::TEXT_PLAIN, content_type_header.value);

    assert_eq!(response_content_length_header_value, header.value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(*response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(response_html_file.into_bytes(), response.content_range_list.get(0).unwrap().body);
}

#[test]
fn check_range_response_for_not_proper_range_header() {
    let uri = "/static/test.txt";
    let url = FileExt::get_static_filepath(uri).unwrap();

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
        name: request_host_header_name.to_string(),
        value: request_host_header_value.to_string()
    };

    let range = Header {
        name: Header::_RANGE.to_string(),
        value: range_header_value.to_string()
    };

    let headers = vec![host, range];
    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers
    };

    let raw_request = Request::_generate_request(request);
    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream, peer_addr);

    let response = Response::_parse_response(raw_response.borrow());

    let response_string = String::from_utf8(raw_response).unwrap();
    println!("\n\n\n{}", &raw_request);
    println!("\n\n\n{}", &response_string);

    assert_eq!(VERSION.http_1_1, response.http_version);
    let header = response._get_header(Header::_X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();
    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF, header.value);
    let header = response._get_header(Header::_ACCEPT_RANGES.to_string()).unwrap();
    assert_eq!(Range::BYTES, header.value);

    assert_eq!(*STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable.status_code, response.status_code);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable.reason_phrase, response.reason_phrase);

    let content_range = response.content_range_list.get(0).unwrap();
    assert_eq!(content_range.body, Range::ERROR_MALFORMED_RANGE_HEADER_WRONG_UNIT.as_bytes());
}

#[test]
fn check_range_response_for_not_proper_range_header_range_end_bigger_than_filesize() {
    let uri = "/static/test.txt";
    let url = FileExt::get_static_filepath(uri).unwrap();

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
        name: request_host_header_name.to_string(),
        value: request_host_header_value.to_string()
    };

    let range = Header {
        name: Header::_RANGE.to_string(),
        value: range_header_value.to_string()
    };

    let headers = vec![host, range];
    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers
    };

    let raw_request = Request::_generate_request(request);
    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream, peer_addr);

    let response = Response::_parse_response(raw_response.borrow());

    let response_string = String::from_utf8(raw_response).unwrap();
    println!("\n\n\n{}", &raw_request);
    println!("\n\n\n{}", &response_string);

    assert_eq!(VERSION.http_1_1, response.http_version);
    let header = response._get_header(Header::_X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();
    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF, header.value);
    let header = response._get_header(Header::_ACCEPT_RANGES.to_string()).unwrap();
    assert_eq!(Range::BYTES, header.value);

    assert_eq!(*STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable.status_code, response.status_code);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable.reason_phrase, response.reason_phrase);

    let content_range = response.content_range_list.get(0).unwrap();
    assert_eq!(content_range.body, Range::ERROR_END_IS_BIGGER_THAN_FILESIZE_CONTENT_RANGE.as_bytes());
}

#[test]
fn check_range_response_for_not_proper_range_header_range_start_bigger_than_end() {
    let uri = "/static/test.txt";
    let url = FileExt::get_static_filepath(uri).unwrap();

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
        name: request_host_header_name.to_string(),
        value: request_host_header_value.to_string()
    };

    let range = Header {
        name: Header::_RANGE.to_string(),
        value: range_header_value.to_string()
    };

    let headers = vec![host, range];
    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers
    };

    let raw_request = Request::_generate_request(request);
    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream, peer_addr);

    let response = Response::_parse_response(raw_response.borrow());

    let response_string = String::from_utf8(raw_response).unwrap();
    println!("\n\n\n{}", &raw_request);
    println!("\n\n\n{}", &response_string);

    assert_eq!(VERSION.http_1_1, response.http_version);
    let header = response._get_header(Header::_X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();
    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF, header.value);
    let header = response._get_header(Header::_ACCEPT_RANGES.to_string()).unwrap();
    assert_eq!(Range::BYTES, header.value);

    assert_eq!(*STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable.status_code, response.status_code);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable.reason_phrase, response.reason_phrase);

    let content_range = response.content_range_list.get(0).unwrap();
    assert_eq!(content_range.body, Range::ERROR_START_IS_AFTER_END_CONTENT_RANGE.as_bytes());
}

#[test]
fn check_range_response_for_not_proper_range_header_range_start_malformed() {
    let uri = "/static/test.txt";
    let url = FileExt::get_static_filepath(uri).unwrap();

    let file = File::open(url).unwrap();
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();

    reader.read_to_end(&mut buffer).unwrap();

    let length = buffer.len();
    let mid = length / 2;
    let end_of_first_range = mid;
    let start_of_second_range = length + 10;
    let not_proper_end_of_second_range = mid;

    let range_header_value = format!("bytes=0-{}, {}zaksd-{}", end_of_first_range, start_of_second_range, not_proper_end_of_second_range);

    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let host = Header {
        name: request_host_header_name.to_string(),
        value: request_host_header_value.to_string()
    };

    let range = Header {
        name: Header::_RANGE.to_string(),
        value: range_header_value.to_string()
    };

    let headers = vec![host, range];
    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers
    };

    let raw_request = Request::_generate_request(request);
    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream, peer_addr);

    let response = Response::_parse_response(raw_response.borrow());

    let response_string = String::from_utf8(raw_response).unwrap();
    println!("\n\n\n{}", &raw_request);
    println!("\n\n\n{}", &response_string);

    assert_eq!(VERSION.http_1_1, response.http_version);
    let header = response._get_header(Header::_X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();
    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF, header.value);
    let header = response._get_header(Header::_ACCEPT_RANGES.to_string()).unwrap();
    assert_eq!(Range::BYTES, header.value);

    assert_eq!(*STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable.status_code, response.status_code);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable.reason_phrase, response.reason_phrase);

    let content_range = response.content_range_list.get(0).unwrap();
    assert_eq!(content_range.body, Range::ERROR_UNABLE_TO_PARSE_RANGE_START.as_bytes());
}

#[test]
fn check_range_response_for_not_proper_range_header_range_end_malformed() {
    let uri = "/static/test.txt";
    let url = FileExt::get_static_filepath(uri).unwrap();

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
        name: request_host_header_name.to_string(),
        value: request_host_header_value.to_string()
    };

    let range = Header {
        name: Header::_RANGE.to_string(),
        value: range_header_value.to_string()
    };

    let headers = vec![host, range];
    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers
    };

    let raw_request = Request::_generate_request(request);
    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream, peer_addr);

    let response = Response::_parse_response(raw_response.borrow());

    let response_string = String::from_utf8(raw_response).unwrap();
    println!("\n\n\n{}", &raw_request);
    println!("\n\n\n{}", &response_string);

    assert_eq!(VERSION.http_1_1, response.http_version);
    let header = response._get_header(Header::_X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();
    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF, header.value);
    let header = response._get_header(Header::_ACCEPT_RANGES.to_string()).unwrap();
    assert_eq!(Range::BYTES, header.value);

    assert_eq!(*STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable.status_code, response.status_code);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable.reason_phrase, response.reason_phrase);

    let content_range = response.content_range_list.get(0).unwrap();
    assert_eq!(content_range.body, Range::ERROR_UNABLE_TO_PARSE_RANGE_END.as_bytes());
}

#[test]
fn check_range_response_for_not_proper_range_header_malformed() {
    let uri = "/static/test.txt";
    let url = FileExt::get_static_filepath(uri).unwrap();

    let file = File::open(url).unwrap();
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();

    reader.read_to_end(&mut buffer).unwrap();


    let range_header_value = format!("bytes=zaksd");

    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let host = Header {
        name: request_host_header_name.to_string(),
        value: request_host_header_value.to_string()
    };

    let range = Header {
        name: Header::_RANGE.to_string(),
        value: range_header_value.to_string()
    };

    let headers = vec![host, range];
    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers
    };

    let raw_request = Request::_generate_request(request);
    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream, peer_addr);

    let response = Response::_parse_response(raw_response.borrow());

    let response_string = String::from_utf8(raw_response).unwrap();
    println!("\n\n\n{}", &raw_request);
    println!("\n\n\n{}", &response_string);

    assert_eq!(VERSION.http_1_1, response.http_version);
    let header = response._get_header(Header::_X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();
    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF, header.value);
    let header = response._get_header(Header::_ACCEPT_RANGES.to_string()).unwrap();
    assert_eq!(Range::BYTES, header.value);

    assert_eq!(*STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable.status_code, response.status_code);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable.reason_phrase, response.reason_phrase);

    let content_range = response.content_range_list.get(0).unwrap();
    assert_eq!(content_range.body, Range::ERROR_UNABLE_TO_PARSE_RANGE_START.as_bytes());
}