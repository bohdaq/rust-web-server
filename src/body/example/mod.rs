use std::collections::HashMap;
use crate::body::example::example_object::ExampleObject;
use crate::body::form_urlencoded::FormUrlEncoded;
use crate::body::multipart_form_data::{FormMultipartData, Part};
use crate::core::New;
use crate::header::content_disposition::{ContentDisposition, DISPOSITION_TYPE};
use crate::header::Header;
use crate::http::VERSION;
use crate::json::object::{ToJSON};
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

// user defined jsons
mod example_object;
mod example_nested_object;

#[test]
fn body_in_request() {
    // can be any piece of data
    let body : Vec<u8> = Vec::from("request body can be anything");

    let request : Request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/some/path".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body, // same as `body: body`
    };

    // replace with your logic
    assert_eq!(Vec::from("request body can be anything"), request.body);
}

#[test]
fn body_in_response() {
    // can be any piece of data
    let body : Vec<u8> = Vec::from("request body can be anything");
    let start = 0;
    let length = body.len();

    let content_range = ContentRange {
        unit: Range::BYTES.to_string(),
        range: Range {
            start, // same as `start: start,`
            end: length as u64
        },
        size: length.to_string(),
        body,
        content_type: MimeType::TEXT_PLAIN.to_string(),
    };

    let response = Response {
        http_version: VERSION.http_1_1.to_string(),
        status_code: *STATUS_CODE_REASON_PHRASE.n200_ok.status_code,
        reason_phrase: STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string(),
        headers: vec![],
        content_range_list: vec![content_range],
    };

    let response_body : &ContentRange = response.content_range_list.get(0).unwrap();

    // replace with your logic
    assert_eq!(Vec::from("request body can be anything"), response_body.body);
}

#[test]
fn multipart_form_data_body_in_request() {
    let mut part_list : Vec<Part> = vec![];

    // part one
    let content_disposition = ContentDisposition {
        disposition_type: DISPOSITION_TYPE.form_data.to_string(),
        field_name: Some("field1".to_string()),
        file_name: None,
    };

    let header = Header::parse_header(&content_disposition.as_string().unwrap()).unwrap();

    let body = "some-data".as_bytes().to_vec();
    let part = Part { headers: vec![header], body: body.clone() };

    part_list.push(part);

    // part two
    let content_disposition = ContentDisposition {
        disposition_type: DISPOSITION_TYPE.form_data.to_string(),
        field_name: Some("field2".to_string()),
        file_name: None,
    };

    let header = Header::parse_header(&content_disposition.as_string().unwrap()).unwrap();

    let body = "another-data".as_bytes().to_vec();
    let part = Part { headers: vec![header], body: body.clone() };

    part_list.push(part);
    let boundary = "------someboundary------";
    let body: Vec<u8> = FormMultipartData::generate(part_list, boundary).unwrap();
    let expected_body : Vec<u8> = body.to_vec(); // creates copy of the vector

    let host = Header {
        name: Header::_HOST.to_string(),
        value: "localhost".to_string()
    };

    let content_type = Header {
        name: Header::_CONTENT_TYPE.to_string(),
        value: format!("multipart/form-data; boundary={}", boundary).to_string()
    };

    let request : Request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/some/path".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![host, content_type],
        body, // same as `body: body`
    };

    // replace with your logic
    assert_eq!(expected_body, request.body);
}

#[test]
fn multipart_form_data_body_in_response() {
    let mut part_list : Vec<Part> = vec![];

    // part one
    let content_disposition = ContentDisposition {
        disposition_type: DISPOSITION_TYPE.form_data.to_string(),
        field_name: Some("field1".to_string()),
        file_name: None,
    };

    let header = Header::parse_header(&content_disposition.as_string().unwrap()).unwrap();

    let body = "some-data".as_bytes().to_vec();
    let part = Part { headers: vec![header], body: body.clone() };

    part_list.push(part);

    // part two
    let content_disposition = ContentDisposition {
        disposition_type: DISPOSITION_TYPE.form_data.to_string(),
        field_name: Some("field2".to_string()),
        file_name: None,
    };

    let header = Header::parse_header(&content_disposition.as_string().unwrap()).unwrap();

    let body = "another-data".as_bytes().to_vec();
    let part = Part { headers: vec![header], body: body.clone() };

    part_list.push(part);
    let boundary = "------someboundary------";

    let body: Vec<u8> = FormMultipartData::generate(part_list, boundary).unwrap();
    let expected_body : Vec<u8> = body.to_vec(); // creates copy of the vector

    let start = 0;
    let length = body.len();

    let content_range = ContentRange {
        unit: Range::BYTES.to_string(),
        range: Range {
            start, // same as `start: start,`
            end: length as u64
        },
        size: length.to_string(),
        body,
        content_type: MimeType::TEXT_PLAIN.to_string(),
    };

    let host = Header {
        name: Header::_HOST.to_string(),
        value: "localhost".to_string()
    };

    let content_type = Header {
        name: Header::_CONTENT_TYPE.to_string(),
        value: format!("multipart/form-data; boundary={}", boundary).to_string()
    };

    let response = Response {
        http_version: VERSION.http_1_1.to_string(),
        status_code: *STATUS_CODE_REASON_PHRASE.n200_ok.status_code,
        reason_phrase: STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string(),
        headers: vec![host, content_type],
        content_range_list: vec![content_range],
    };

    let response_body : &ContentRange = response.content_range_list.get(0).unwrap();

    // replace with your logic
    assert_eq!(expected_body, response_body.body);
}


#[test]
fn form_urlencoded() {
    // u may need to encode key values before inserting into map
    // https://en.wikipedia.org/wiki/URL_encoding
    let mut params_map: HashMap<String, String> = HashMap::new();
    params_map.insert("key1".to_string(), "test1".to_string());
    params_map.insert("key2".to_string(), "test2".to_string());

    let body: Vec<u8> = FormUrlEncoded::generate(params_map).as_bytes().to_vec();
    let expected_body : Vec<u8> = body.to_vec(); // creates copy of the vector

    let host = Header {
        name: Header::_HOST.to_string(),
        value: "localhost".to_string()
    };

    let content_type = Header {
        name: Header::_CONTENT_TYPE.to_string(),
        value: "application/x-www-form-urlencoded".to_string()
    };

    let request : Request = Request {
        method: METHOD.post.to_string(), // in get request query string is sent as part of request_uri, not a body
        request_uri: "/some/path".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![host, content_type],
        body, // same as `body: body`
    };

    // replace with your logic
    assert_eq!(expected_body, request.body);
}


#[test]
fn json_in_request() {
    let first_object = ExampleObject::new();

    let second_object = ExampleObject {
        prop_a: "test".to_string(),
        prop_b: true,
        prop_c: false,
        prop_d: 10,
        prop_e: 2.2,
        prop_f: None,
        prop_g: None,
    };

    let list  = vec![first_object, second_object];


    let json_array : String = ExampleObject::to_json_list(list).unwrap();

    let body: Vec<u8> = json_array.as_bytes().to_vec();
    let expected_body : Vec<u8> = body.to_vec(); // creates copy of the vector

    let host = Header {
        name: Header::_HOST.to_string(),
        value: "localhost".to_string()
    };

    let content_type = Header {
        name: Header::_CONTENT_TYPE.to_string(),
        value: "application/json".to_string()
    };

    let request : Request = Request {
        method: METHOD.post.to_string(), // in get request query string is sent as part of request_uri, not a body
        request_uri: "/some/path".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![host, content_type],
        body, // same as `body: body`
    };

    // replace with your logic
    assert_eq!(expected_body, request.body);

    let actual_json_string = String::from_utf8(request.body).unwrap();
    let _actual_list : Vec<ExampleObject> = ExampleObject::from_json_list(actual_json_string).unwrap();
}

#[test]
fn json_body_in_response() {
    let object = ExampleObject {
        prop_a: "test".to_string(),
        prop_b: true,
        prop_c: false,
        prop_d: 10,
        prop_e: 2.2,
        prop_f: None,
        prop_g: None,
    };

    let json_object : String = object.to_json_string();

    let body: Vec<u8> = json_object.as_bytes().to_vec();
    let expected_body : Vec<u8> = body.to_vec(); // creates copy of the vector

    let start = 0;
    let length = body.len();

    let content_range = ContentRange {
        unit: Range::BYTES.to_string(),
        range: Range {
            start, // same as `start: start,`
            end: length as u64
        },
        size: length.to_string(),
        body,
        content_type: MimeType::TEXT_PLAIN.to_string(),
    };

    let host = Header {
        name: Header::_HOST.to_string(),
        value: "localhost".to_string()
    };

    let content_type = Header {
        name: Header::_CONTENT_TYPE.to_string(),
        value: "application/json".to_string()
    };

    let response = Response {
        http_version: VERSION.http_1_1.to_string(),
        status_code: *STATUS_CODE_REASON_PHRASE.n200_ok.status_code,
        reason_phrase: STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string(),
        headers: vec![host, content_type],
        content_range_list: vec![content_range],
    };

    // replace with your logic

    let response_body : &ContentRange = response.content_range_list.get(0).unwrap();
    assert_eq!(expected_body, response_body.body);
    let json_string = String::from_utf8(response_body.body.to_vec()).unwrap();
    ExampleObject::parse(json_string).unwrap();
}


#[test]
fn multipart_body_in_response() {
    let resource_uri = "/static/content.png";

    // building body (content_range_list)
    let range = Header {
        name: Header::_RANGE.to_string(),
        value: "bytes=200-1000, 1200-1400".to_string()
    };

    let content_range_list : Vec<ContentRange> = Range::get_content_range_list(resource_uri, &range).unwrap();

    // building response
    let host = Header {
        name: Header::_HOST.to_string(),
        value: "localhost".to_string()
    };

    let content_type = Header {
        name: Header::_CONTENT_TYPE.to_string(),
        value: Range::MULTIPART_BYTERANGES_CONTENT_TYPE.to_string()
    };

    let response = Response {
        http_version: VERSION.http_1_1.to_string(),
        status_code: *STATUS_CODE_REASON_PHRASE.n206_partial_content.status_code,
        reason_phrase: STATUS_CODE_REASON_PHRASE.n206_partial_content.reason_phrase.to_string(),
        headers: vec![host, content_type],
        content_range_list // same as `content_range_list: content_range_list`
    };


    // replace with your logic
    let response_body : &ContentRange = response.content_range_list.get(0).unwrap();
    assert_eq!(801, response_body.body.len());

    let response_body : &ContentRange = response.content_range_list.get(1).unwrap();
    assert_eq!(201, response_body.body.len());

}


