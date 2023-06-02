use crate::http::VERSION;
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

#[test]
fn as_response() {
    let data : &[u8] = "any sequence of bytes".as_bytes();
    let range = Range { start: 0, end: data.len() as u64 };
    let content_range = ContentRange {
        unit: Range::BYTES.to_string(),
        range: range,
        size: data.len().to_string(),
        body: Vec::from(data),
        content_type: MimeType::APPLICATION_OCTET_STREAM.to_string(),
    };

    let _response = Response {
        http_version: VERSION.http_1_1.to_string(),
        status_code: *STATUS_CODE_REASON_PHRASE.n200_ok.status_code,
        reason_phrase: STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string(),
        headers: vec![],
        content_range_list: vec![content_range],
    };
}

#[test]
fn as_response_range_shortcut() {
    let data : Vec<u8> = "any sequence of bytes".as_bytes().to_vec();

    let content_range = Range::get_content_range(
        data,
        MimeType::APPLICATION_OCTET_STREAM.to_string()
    );

    let _response = Response::build(
        STATUS_CODE_REASON_PHRASE.n200_ok.clone(),
        vec![],
        vec![content_range]
    );
}