use crate::http::VERSION;
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

#[test]
fn body_in_request() {
    // can be any piece of data
    let body : Vec<u8> = Vec::from("request body can be anythings");

    let request : Request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/some/path".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body, // same as `body: body`
    };

    // replace with your logic
    assert_eq!(Vec::from("request body can be anythings"), request.body);
}

#[test]
fn body_in_response() {
    // can be any piece of data
    let body : Vec<u8> = Vec::from("request body can be anythings");
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
    assert_eq!(Vec::from("request body can be anythings"), response_body.body);
}


