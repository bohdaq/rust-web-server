use crate::body::multipart_form_data::{FormMultipartData, Part};
use crate::header::content_disposition::{ContentDisposition, DISPOSITION_TYPE};
use crate::header::Header;
use crate::http::VERSION;
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

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

    let request : Request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/some/path".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body, // same as `body: body`
    };

    // replace with your logic
    assert_eq!(expected_body, request.body);
}


