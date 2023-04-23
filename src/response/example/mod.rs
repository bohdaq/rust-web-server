use file_ext::FileExt;
use crate::header::Header;
use crate::http::VERSION;
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

#[test]
fn parse() {
    // TODO

    let path = FileExt::build_path(&["src", "response", "example", "response.multipart.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let response_raw_bytes : Vec<u8> = FileExt::read_file(absolute_file_path.as_str()).unwrap();

    let response = Response::_parse_response(response_raw_bytes.as_ref());

    assert_eq!(&response.status_code, STATUS_CODE_REASON_PHRASE.n200_ok.status_code);
}

#[test]
fn build() {

    // host header
    let host = Header {
        name: Header::_HOST.to_string(),
        value: "localhost".to_string(),
    };

    // body contains one part
    let data : &str = "some text";
    let length : usize = data.len();

    // range is required to build content range
    let range = Range { start: 0, end: length as u64 };

    let content_range = ContentRange {
        unit: Range::BYTES.to_string(),
        range: range,
        size: length.to_string(),
        body: data.as_bytes().to_vec(),
        content_type: MimeType::TEXT_PLAIN.to_string(),
    };

    // response body represented by content_range_list field.
    // by design response may contain several parts of different data
    let mut response = Response {
        http_version: VERSION.http_1_1.to_string(),
        status_code: *STATUS_CODE_REASON_PHRASE.n200_ok.status_code,
        reason_phrase: STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string(),
        headers: vec![host],
        content_range_list: vec![content_range],
    };

    let response_as_array = response.generate();


    // asserts, replace with your logic
    let path = FileExt::build_path(&["src", "response", "example", "response.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let expected_file = FileExt::read_file(absolute_file_path.as_str()).unwrap();

    assert_eq!(expected_file, response_as_array);

}