use std::{env, fs};
use std::borrow::Borrow;
use crate::constant::{HTTP_VERSIONS, REQUEST_METHODS, RESPONSE_STATUS_CODE_REASON_PHRASES};
use crate::header::Header;
use crate::{bootstrap, Config, CONSTANTS, Request, Response, Server};
use crate::cors::Cors;
use crate::mime_type::MimeType;
use crate::server::tests::MockTcpStream;

#[test]
fn cors_options_preflight_request() {
    // request test data

    let request_method = REQUEST_METHODS.options;
    let request_uri = "/static/test.json";
    let request_http_version = HTTP_VERSIONS.http_version_1_1.to_string();

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
    let request_access_control_request_headers_header_value = "content-type,x-custom-header";
    let access_control_request_headers = Header {
        header_name: request_access_control_request_headers_header_name.to_string(),
        header_value: request_access_control_request_headers_header_value.to_string()
    };

    // aplication/json content type header makes this request follow the regular CORS flow
    let content_type = Header {
        header_name: Header::CONTENT_TYPE.to_string(),
        header_value: MimeType::APPLICATION_JSON.to_string()
    };

    let headers = vec![host, origin, access_control_request_method, access_control_request_headers, content_type];
    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes()).unwrap();
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.header_value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = HTTP_VERSIONS.http_version_1_1.to_string();
    let response_status_code = RESPONSE_STATUS_CODE_REASON_PHRASES.n204_no_content.status_code;
    let response_reason_phrase = RESPONSE_STATUS_CODE_REASON_PHRASES.n204_no_content.reason_phrase;

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();

    let response_filepath = [working_directory, request.request_uri.as_str()].join(CONSTANTS.empty_string);
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
    assert_eq!(CONSTANTS.nosniff, x_content_type_options_header.header_value);

    let access_control_allow_origin_header = response.get_header(Header::ACCESS_CONTROL_ALLOW_ORIGIN.to_string()).unwrap();
    let allow_origins = format!("{}", access_control_allow_origin_header.header_value);
    assert!(allow_origins.contains(request_origin_header_value));

    let access_control_allow_methods_header = response.get_header(Header::ACCESS_CONTROL_ALLOW_METHODS.to_string()).unwrap();
    assert!(access_control_allow_methods_header.header_value.contains(request_access_control_request_method_header_value));

    let access_control_allow_headers_header = response.get_header(Header::ACCESS_CONTROL_ALLOW_HEADERS.to_string()).unwrap();
    assert_eq!(request_access_control_request_headers_header_value, access_control_allow_headers_header.header_value);

    let access_control_allow_credentials_header = response.get_header(Header::ACCESS_CONTROL_ALLOW_CREDENTIALS.to_string()).unwrap();
    assert_eq!("true", access_control_allow_credentials_header.header_value);

    let access_control_expose_headers_header = response.get_header(Header::ACCESS_CONTROL_EXPOSE_HEADERS.to_string()).unwrap();
    assert_eq!(request_access_control_request_headers_header_value, access_control_expose_headers_header.header_value);

    let access_control_max_age_header = response.get_header(Header::ACCESS_CONTROL_MAX_AGE.to_string()).unwrap();
    assert_eq!("523452", access_control_max_age_header.header_value);

    let vary_header = response.get_header(Header::VARY.to_string()).unwrap();
    assert_eq!(Header::ORIGIN, vary_header.header_value);
}

#[test]
fn actual_request_after_preflight() {
    let is_test_mode = true;
    bootstrap(is_test_mode);

    let request_method = REQUEST_METHODS.get;
    let request_uri = "/static/test.json";
    let request_http_version = HTTP_VERSIONS.http_version_1_1.to_string();


    let request_host_header_name = Header::HOST;
    let request_host_header_value = "localhost:7777";
    let host = Header {
        header_name: request_host_header_name.to_string(),
        header_value: request_host_header_value.to_string()
    };

    // aplication/json content type header makes this request follow the regular CORS flow
    let content_type = Header {
        header_name: Header::CONTENT_TYPE.to_string(),
        header_value: MimeType::APPLICATION_JSON.to_string()
    };

    let request_origin_header_name = Header::ORIGIN;
    let request_origin_header_value = "https://foo.example";
    let origin = Header {
        header_name: request_origin_header_name.to_string(),
        header_value: request_origin_header_value.to_string()
    };

    let headers = vec![host, content_type, origin];

    let request = Request {
        method: request_method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: request_http_version.to_string(),
        headers
    };

    let raw_request = Request::generate_request(request);

    let request: Request = Request::parse_request(&raw_request.as_bytes()).unwrap();
    let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

    assert_eq!(request_host_header_value.to_string(), host_header.header_value);
    assert_eq!(request_method.to_string(), request.method);
    assert_eq!(request_uri.to_string(), request.request_uri);
    assert_eq!(request_http_version.to_string(), request.http_version);

    // response part
    let response_http_version = HTTP_VERSIONS.http_version_1_1.to_string();
    let response_status_code = RESPONSE_STATUS_CODE_REASON_PHRASES.n200_ok.status_code;
    let response_reason_phrase = RESPONSE_STATUS_CODE_REASON_PHRASES.n200_ok.reason_phrase;
    let response_filepath = &request.request_uri;

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();

    let response_filepath = [working_directory, request.request_uri.as_str()].join(CONSTANTS.empty_string);
    let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
    let response_content_length_header_name = "Content-Length";
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
    assert_eq!(response_html_file.into_bytes(), response.content_range_list.get(0).unwrap().body);

    let header = response.get_header(response_content_length_header_name.to_string()).unwrap();
    assert_eq!(response_content_length_header_value, header.header_value);

    let content_type_header = response.get_header(Header::CONTENT_TYPE.to_string()).unwrap();
    assert_eq!(MimeType::APPLICATION_JSON, content_type_header.header_value);

    let x_content_type_options_header = response.get_header(Header::X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();
    assert_eq!(CONSTANTS.nosniff, x_content_type_options_header.header_value);

    let vary_header = response.get_header(Header::VARY.to_string()).unwrap();
    assert_eq!(Header::ORIGIN, vary_header.header_value);

    let access_control_allow_origin_header = response.get_header(Header::ACCESS_CONTROL_ALLOW_ORIGIN.to_string()).unwrap();
    let allow_origins = format!("{}", access_control_allow_origin_header.header_value);
    assert!(allow_origins.contains(request_origin_header_value));

    let access_control_allow_credentials_header = response.get_header(Header::ACCESS_CONTROL_ALLOW_CREDENTIALS.to_string()).unwrap();
    assert_eq!("true", access_control_allow_credentials_header.header_value);

}

#[test]
fn cors_allow_all() {
    println!("cors_allow_all");

    let origin_value = "origin-value.com";
    let custom_header = "X-CUSTOM-HEADER";

    let expected_allow_headers = format!("{},{}", Header::CONTENT_TYPE, custom_header);

    let mut request = Request {
        method: REQUEST_METHODS.options.to_string(),
        request_uri: "".to_string(),
        http_version: "".to_string(),
        headers: vec![
            Header {
                header_name: Header::ORIGIN.to_string(),
                header_value: origin_value.to_string()
            },
            Header {
                header_name: Header::ACCESS_CONTROL_REQUEST_METHOD.to_string(),
                header_value: REQUEST_METHODS.post.to_string()
            },
            Header {
                header_name: Header::ACCESS_CONTROL_REQUEST_HEADERS.to_string(),
                header_value: expected_allow_headers
            },
        ],
    };

    let mut response = Response {
        http_version: "".to_string(),
        status_code: "".to_string(),
        reason_phrase: "".to_string(),
        headers: vec![],
        content_range_list: vec![]
    };

    (request, response) = Cors::allow_all(request, response).unwrap();

    let allow_origins = response.get_header(Header::ACCESS_CONTROL_ALLOW_ORIGIN.to_string()).unwrap();
    assert_eq!(origin_value, allow_origins.header_value);

    let allow_methods = response.get_header(Header::ACCESS_CONTROL_ALLOW_METHODS.to_string()).unwrap();
    assert_eq!(REQUEST_METHODS.post, allow_methods.header_value);

    let allow_headers = response.get_header(Header::ACCESS_CONTROL_ALLOW_HEADERS.to_string()).unwrap();
    let expected_allow_headers = format!("{},{}", Header::CONTENT_TYPE.to_lowercase(), custom_header.to_lowercase());
    assert_eq!(expected_allow_headers, allow_headers.header_value);

    let allow_credentials = response.get_header(Header::ACCESS_CONTROL_ALLOW_CREDENTIALS.to_string()).unwrap();
    assert_eq!("true", allow_credentials.header_value);

    let expose_headers = response.get_header(Header::ACCESS_CONTROL_ALLOW_HEADERS.to_string()).unwrap();
    let expected_expose_headers = format!("{},{}", Header::CONTENT_TYPE.to_lowercase(), custom_header.to_lowercase());
    assert_eq!(expected_expose_headers, expose_headers.header_value);

    let max_age = response.get_header(Header::ACCESS_CONTROL_MAX_AGE.to_string()).unwrap();
    assert_eq!(Cors::MAX_AGE, max_age.header_value);

    let vary = response.get_header(Header::VARY.to_string()).unwrap();
    assert_eq!(Header::ORIGIN.to_string(), vary.header_value);

    let raw_response = Response::generate_response(response, request);
    let response_string = String::from_utf8(raw_response).unwrap();
    println!("{}", response_string);
    println!("end cors_allow_all");
}

#[test]
fn cors_process() {
    println!("cors_process");

    // Origin header indicates it is CORS request
    let origin_value = "https://foo.example";
    let mut request = Request {
        method: REQUEST_METHODS.options.to_string(),
        request_uri: "".to_string(),
        http_version: "".to_string(),
        headers: vec![
            Header {
                header_name: Header::ORIGIN.to_string(),
                header_value: origin_value.to_string()
            }
        ],
    };

    let mut response = Response {
        http_version: "".to_string(),
        status_code: "".to_string(),
        reason_phrase: "".to_string(),
        headers: vec![],
        content_range_list: vec![]
    };

    let first_domain = "https://foo.example";
    let second_domain = "https://bar.example";

    let custom_header = "x-custom-header";
    let cors_config = Cors {
        allow_all: false,
        allow_origins: vec![first_domain.to_string(), second_domain.to_string()],
        allow_methods: vec![REQUEST_METHODS.get.to_string(), REQUEST_METHODS.post.to_string(), REQUEST_METHODS.put.to_string()],
        allow_headers: vec![Header::CONTENT_TYPE.to_string(), custom_header.to_string()],
        allow_credentials: true,
        expose_headers: vec![Header::CONTENT_TYPE.to_string(), custom_header.to_string()],
        max_age: "172800".to_string()
    };

    (request, response) = Cors::process(request, response, &cors_config).unwrap();

    let allow_origins = response.get_header(Header::ACCESS_CONTROL_ALLOW_ORIGIN.to_string()).unwrap();
    let expected_allow_origins = format!("{}", origin_value);
    assert_eq!(expected_allow_origins, allow_origins.header_value);

    let allow_methods = response.get_header(Header::ACCESS_CONTROL_ALLOW_METHODS.to_string()).unwrap();
    let expected_allow_methods = format!("{},{},{}", REQUEST_METHODS.get, REQUEST_METHODS.post, REQUEST_METHODS.put);
    assert_eq!(expected_allow_methods, allow_methods.header_value);

    let allow_headers = response.get_header(Header::ACCESS_CONTROL_ALLOW_HEADERS.to_string()).unwrap();
    let expected_allow_headers = format!("{},{}", Header::CONTENT_TYPE, custom_header).to_lowercase();
    assert_eq!(expected_allow_headers, allow_headers.header_value);

    let allow_credentials = response.get_header(Header::ACCESS_CONTROL_ALLOW_CREDENTIALS.to_string()).unwrap();
    assert_eq!("true", allow_credentials.header_value);

    let expose_headers = response.get_header(Header::ACCESS_CONTROL_ALLOW_HEADERS.to_string()).unwrap();
    let expected_expose_headers = format!("{},{}", Header::CONTENT_TYPE, custom_header).to_lowercase();
    assert_eq!(expected_expose_headers, expose_headers.header_value);

    let max_age = response.get_header(Header::ACCESS_CONTROL_MAX_AGE.to_string()).unwrap();
    assert_eq!(cors_config.max_age, max_age.header_value);

    let vary = response.get_header(Header::VARY.to_string()).unwrap();
    assert_eq!(Header::ORIGIN.to_string(), vary.header_value);

    let raw_response = Response::generate_response(response, request);
    let response_string = String::from_utf8(raw_response).unwrap();
    println!("{}", response_string);

    println!("end cors_process");
}

#[test]
fn cors_process_default_config() {
    println!("cors_process_default_config");

    let is_test_mode = true;
    bootstrap(is_test_mode);

    // Origin header indicates it is CORS request
    let origin_value = "https://bar.example";
    let mut request = Request {
        method: REQUEST_METHODS.options.to_string(),
        request_uri: "".to_string(),
        http_version: "".to_string(),
        headers: vec![
            Header {
                header_name: Header::ORIGIN.to_string(),
                header_value: origin_value.to_string()
            }
        ],
    };

    let mut response = Response {
        http_version: "".to_string(),
        status_code: "".to_string(),
        reason_phrase: "".to_string(),
        headers: vec![],
        content_range_list: vec![]
    };

    let custom_header = "x-custom-header";

    (request, response) = Cors::process_using_default_config(request, response).unwrap();

    let allow_origins = response.get_header(Header::ACCESS_CONTROL_ALLOW_ORIGIN.to_string()).unwrap();
    let expected_allow_origins = format!("{}", origin_value);
    assert_eq!(expected_allow_origins, allow_origins.header_value);

    let allow_methods = response.get_header(Header::ACCESS_CONTROL_ALLOW_METHODS.to_string()).unwrap();
    let expected_allow_methods = format!("{},{}", REQUEST_METHODS.post, REQUEST_METHODS.put);
    assert_eq!(expected_allow_methods, allow_methods.header_value);

    let allow_headers = response.get_header(Header::ACCESS_CONTROL_ALLOW_HEADERS.to_string()).unwrap();
    let expected_allow_headers = format!("{},{}", Header::CONTENT_TYPE, custom_header).to_lowercase();
    assert_eq!(expected_allow_headers, allow_headers.header_value);

    let allow_credentials = response.get_header(Header::ACCESS_CONTROL_ALLOW_CREDENTIALS.to_string()).unwrap();
    assert_eq!("true", allow_credentials.header_value);

    let expose_headers = response.get_header(Header::ACCESS_CONTROL_ALLOW_HEADERS.to_string()).unwrap();
    let expected_expose_headers = format!("{},{}", Header::CONTENT_TYPE, custom_header).to_lowercase();
    assert_eq!(expected_expose_headers, expose_headers.header_value);

    let max_age = response.get_header(Header::ACCESS_CONTROL_MAX_AGE.to_string()).unwrap();
    assert_eq!("523452", max_age.header_value);

    let vary = response.get_header(Header::VARY.to_string()).unwrap();
    assert_eq!(Header::ORIGIN.to_string(), vary.header_value);

    let raw_response = Response::generate_response(response, request);
    let response_string = String::from_utf8(raw_response).unwrap();
    println!("{}", response_string);

    println!("end cors_process_default_config");
}

#[test]
fn cors_process_empty_config() {
    println!("cors_process_empty_config");

    // Origin header indicates it is CORS request
    let origin_value = "origin-value.com";
    let mut request = Request {
        method: "".to_string(),
        request_uri: "".to_string(),
        http_version: "".to_string(),
        headers: vec![
            Header {
                header_name: Header::ORIGIN.to_string(),
                header_value: origin_value.to_string()
            }
        ],
    };

    let mut response = Response {
        http_version: "".to_string(),
        status_code: "".to_string(),
        reason_phrase: "".to_string(),
        headers: vec![],
        content_range_list: vec![]
    };

    let cors_config = Cors {
        allow_all: false,
        allow_origins: vec![],
        allow_methods: vec![],
        allow_headers: vec![],
        allow_credentials: false,
        expose_headers: vec![],
        max_age: "".to_string()
    };

    (request, response) = Cors::process(request, response, &cors_config).unwrap();

    let allow_origins = response.get_header(Header::ACCESS_CONTROL_ALLOW_ORIGIN.to_string());
    assert!(allow_origins.is_none());

    let allow_methods = response.get_header(Header::ACCESS_CONTROL_ALLOW_METHODS.to_string());
    assert!(allow_methods.is_none());

    let allow_headers = response.get_header(Header::ACCESS_CONTROL_ALLOW_HEADERS.to_string());
    assert!(allow_headers.is_none());

    let boxed_allow_credentials = response.get_header(Header::ACCESS_CONTROL_ALLOW_CREDENTIALS.to_string());
    assert!(boxed_allow_credentials.is_none());

    let expose_headers = response.get_header(Header::ACCESS_CONTROL_ALLOW_HEADERS.to_string());
    assert!(expose_headers.is_none());

    let max_age = response.get_header(Header::ACCESS_CONTROL_MAX_AGE.to_string());
    assert!(max_age.is_none());

    let vary = response.get_header(Header::VARY.to_string());
    assert!(vary.is_none());

    let raw_response = Response::generate_response(response, request);
    let response_string = String::from_utf8(raw_response).unwrap();
    println!("{}", response_string);

    println!("end cors_process_empty_config");
}