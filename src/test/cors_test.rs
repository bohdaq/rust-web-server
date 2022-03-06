use std::{env, fs};
use std::borrow::Borrow;
use crate::constant::{HTTP_VERSIONS, REQUEST_METHODS, RESPONSE_STATUS_CODE_REASON_PHRASES};
use crate::header::Header;
use crate::{CONSTANTS, Request, Response, Server};
use crate::mime_type::MimeType;
use crate::test::server_test::MockTcpStream;

#[test]
fn cors_options_preflight_request() {
    // request test data

    let request_method = REQUEST_METHODS.OPTIONS;
    let request_uri = "/static/test.json";
    let request_http_version = HTTP_VERSIONS.HTTP_VERSION_1_1.to_string();

    let request_host_header_name = Header::HOST;
    let request_host_header_value = "localhost:7777";
    let host = Header {
        header_name: request_host_header_name.to_string(),
        header_value: request_host_header_value.to_string()
    };

    let request_origin_header_name = Header::ORIGIN;
    let request_origin_header_value = "https://foo.example";
    let origin = Header {
        header_name: request_origin_header_name.to_string(),
        header_value: request_origin_header_value.to_string()
    };

    let request_access_control_request_method_header_name = Header::ACCESS_CONTROL_REQUEST_METHOD;
    let request_access_control_request_method_header_value = "POST";
    let access_control_request_method = Header {
        header_name: request_access_control_request_method_header_name.to_string(),
        header_value: request_access_control_request_method_header_value.to_string()
    };

    let request_access_control_request_headers_header_name = Header::ACCESS_CONTROL_REQUEST_HEADERS;
    let request_access_control_request_headers_header_value = "X-PINGOTHER, Content-Type";
    let access_control_request_headers = Header {
        header_name: request_access_control_request_headers_header_name.to_string(),
        header_value: request_access_control_request_headers_header_value.to_string()
    };

    let headers = vec![host, origin, access_control_request_method, access_control_request_headers];
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
    let response_status_code = RESPONSE_STATUS_CODE_REASON_PHRASES.N204_NO_CONTENT.STATUS_CODE;
    let response_reason_phrase = RESPONSE_STATUS_CODE_REASON_PHRASES.N204_NO_CONTENT.REASON_PHRASE;

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();

    let response_filepath = [working_directory, request.request_uri.as_str()].join(CONSTANTS.EMPTY_STRING);
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = Header::CONTENT_LENGTH;
    let response_content_length_header_value = response_html_file.len().to_string();

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream);
    let response = Response::parse_response(raw_response.borrow());


    assert_eq!(response_http_version, response.http_version);
    assert_eq!(response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);

    let content_length_header = response.get_header(response_content_length_header_name.to_string()).unwrap();
    assert_eq!(response_content_length_header_value, content_length_header.header_value);

    let content_type_header = response.get_header(Header::CONTENT_TYPE.to_string()).unwrap();
    assert_eq!(MimeType::APPLICATION_JSON, content_type_header.header_value);

    let x_content_type_options_header = response.get_header(Header::X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();
    assert_eq!(CONSTANTS.NOSNIFF, x_content_type_options_header.header_value);


}

#[test]
fn it_generates_successful_response_with_static_file() {
    // request test data
    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let request_method = REQUEST_METHODS.GET;
    let request_uri = "/static/test.json";
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

    let content_type_header = response.get_header(Header::CONTENT_TYPE.to_string()).unwrap();
    let x_content_type_options_header = response.get_header(Header::X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();

    assert_eq!(CONSTANTS.NOSNIFF, x_content_type_options_header.header_value);
    assert_eq!(MimeType::APPLICATION_JSON, content_type_header.header_value);

    assert_eq!(response_content_length_header_value, header.header_value);
    assert_eq!(response_http_version, response.http_version);
    assert_eq!(response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(response_html_file.into_bytes(), response.content_range_list.get(0).unwrap().body);
}