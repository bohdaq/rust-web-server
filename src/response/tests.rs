use std::borrow::Borrow;
use std::env;
use std::fs::File;
use std::io::Read;
use regex::Regex;
use crate::header::Header;
use crate::http::VERSION;
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};
use crate::request::{METHOD, Request};
use crate::response::{Error, Response, STATUS_CODE_REASON_PHRASE};
use crate::symbol::SYMBOL;

#[test]
fn check_is_multipart_byteranges_content_type() {
    let content_type = Header {
        name: Header::CONTENT_TYPE.to_string(),
        value: "multipart/byteranges; boundary=String_separator".to_string(),
    };

    let is_multipart = Response::_is_multipart_byteranges_content_type(&content_type);
    assert_eq!(true, is_multipart);
}

#[test]
fn error() {
    let error = Error {
        status_code_reason_phrase: STATUS_CODE_REASON_PHRASE.n200_ok,
        message: "some msg".to_string()
    };

    let clone = error.clone();

    assert_eq!(error, clone);
    assert_eq!("some msg", error.message);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n200_ok, error.status_code_reason_phrase);
}

#[test]
fn http_version_and_status_code_and_reason_phrase_regex() {

    let re = Regex::new(Response::_HTTP_VERSION_AND_STATUS_CODE_AND_REASON_PHRASE_REGEX).unwrap();
    let caps = re.captures("HTTP/1.1 404 Not Found").unwrap();

    assert_eq!(VERSION.http_1_1, &caps["http_version"]);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n404_not_found.status_code, &caps["status_code"]);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n404_not_found.reason_phrase, &caps["reason_phrase"]);

    let re = Regex::new(Response::_HTTP_VERSION_AND_STATUS_CODE_AND_REASON_PHRASE_REGEX).unwrap();
    let caps = re.captures("HTTP/1.1 200 OK").unwrap();

    assert_eq!(VERSION.http_1_1, &caps["http_version"]);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n200_ok.status_code, &caps["status_code"]);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase, &caps["reason_phrase"]);

}


#[test]
fn it_generates_successful_response_with_additional_headers() {
    let response_http_version = VERSION.http_1_1.to_string();
    let response_status_code = "401";
    let response_reason_phrase = "Unauthorized";
    let message_body = SYMBOL.empty_string;


    let content_range = ContentRange {
        unit: Range::BYTES.to_string(),
        range: Range {
            start: 0,
            end: message_body.as_bytes().len() as u64
        },
        size: message_body.as_bytes().len().to_string(),
        body: message_body.as_bytes().to_vec(),
        content_type: MimeType::TEXT_PLAIN.to_string()
    };

    let headers = vec![];
    let response = Response {
        http_version: response_http_version.to_string(),
        status_code: response_status_code.to_string(),
        reason_phrase: response_reason_phrase.to_string(),
        headers,
        content_range_list: vec![content_range],
    };


    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = message_body.len().to_string();
    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/some-route".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![]
    };

    let raw_response = Response::generate_response(response, request);
    let response = Response::_parse_response(raw_response.borrow());


    let content_length_header = response._get_header(response_content_length_header_name.to_string()).unwrap();
    assert_eq!(response_content_length_header_value, content_length_header.value);


    assert_eq!(response_http_version, response.http_version);
    assert_eq!(response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(message_body.as_bytes().to_vec(), response.content_range_list.get(0).unwrap().body);

    let response_clone = response.clone();
    assert_eq!(response, response_clone);
}

#[test]
fn it_generates_successful_response_with_additional_headers_and_non_utf8_file() {
    let response_http_version = VERSION.http_1_1.to_string();
    let response_status_code = STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    let response_reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase;
    let filepath = "/static/content.png";

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();

    let mut response_filepath = [working_directory, filepath].join(SYMBOL.empty_string);
    let mut contents = Vec::new();
    let mut file = File::open(response_filepath).unwrap();
    file.read_to_end(&mut contents).expect("Unable to read");

    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = contents.len().to_string();

    let headers = vec![];

    let content_range = ContentRange {
        unit: Range::BYTES.to_string(),
        range: Range {
            start: 0,
            end: contents.len() as u64
        },
        size: contents.len().to_string(),
        body: contents.to_vec(),
        content_type: MimeType::IMAGE_PNG.to_string()
    };

    let response = Response {
        http_version: response_http_version.to_string(),
        status_code: response_status_code.to_string(),
        reason_phrase: response_reason_phrase.to_string(),
        headers,
        content_range_list: vec![content_range],
    };

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/some-route".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![]
    };

    let raw_response = Response::generate_response(response, request);
    let response = Response::_parse_response(raw_response.borrow());


    let content_length_header = response._get_header(response_content_length_header_name.to_string()).unwrap();
    assert_eq!(response_content_length_header_value, content_length_header.value);


    assert_eq!(response_http_version, response.http_version);
    assert_eq!(response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);

    contents = Vec::new();
    response_filepath = [working_directory, filepath].join(SYMBOL.empty_string);
    file = File::open(response_filepath).unwrap();
    file.read_to_end(&mut contents).expect("Unable to read");
    assert_eq!(contents, response.content_range_list.get(0).unwrap().body);
}
