use file_ext::FileExt;
use crate::header::Header;
use crate::http::VERSION;
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

#[test]
fn parse() {
    // in this example response is from reading a file
    let path = FileExt::build_path(&["src", "response", "example", "response.multipart.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let response_raw_bytes : Vec<u8> = FileExt::read_file(absolute_file_path.as_str()).unwrap();

    let response_parse : Result<Response, String> = Response::parse(response_raw_bytes.as_ref());
    if response_parse.is_err() {
        let _message = response_parse.clone().err().unwrap();
        // handle error
    }
    let response : Response = response_parse.unwrap();


    // asserts, replace with your logic
    assert_eq!(&response.status_code, STATUS_CODE_REASON_PHRASE.n200_ok.status_code);
    assert_eq!(&response.reason_phrase, STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase);
    assert_eq!(&response.http_version, VERSION.http_1_1);

    let host_header : &Header = response.get_header(Header::_HOST.to_string()).unwrap();
    assert_eq!(host_header.value, "localhost");

    let content_type_header : &Header = response.get_header(Header::_CONTENT_TYPE.to_string()).unwrap();
    assert_eq!(content_type_header.value, "multipart/byteranges; boundary=String_separator");

    let body : Vec<ContentRange> = response.content_range_list;
    let number_of_parts : usize = body.len();
    assert_eq!(2, number_of_parts);

    let first_part : &ContentRange = body.get(0).unwrap();
    assert_eq!(first_part.content_type, MimeType::TEXT_PLAIN);
    assert_eq!(first_part.range.start, 0);
    assert_eq!(first_part.range.end, 9);
    assert_eq!(first_part.size, "9");
    assert_eq!(first_part.body, "some text".as_bytes());

    let second_part : &ContentRange = body.get(1).unwrap();
    assert_eq!(second_part.content_type, MimeType::TEXT_PLAIN);
    assert_eq!(second_part.range.start, 0);
    assert_eq!(second_part.range.end, 12);
    assert_eq!(second_part.size, "12");
    assert_eq!(second_part.body, "another text".as_bytes());

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

    let response_as_array : Vec<u8> = response.generate();


    // asserts, replace with your logic
    let path = FileExt::build_path(&["src", "response", "example", "response.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let expected_file : Vec<u8> = FileExt::read_file(absolute_file_path.as_str()).unwrap();

    assert_eq!(expected_file, response_as_array);

}