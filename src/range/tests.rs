use std::borrow::Borrow;
use std::fs::{File, metadata};
use std::io::{BufReader, Read};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use file_ext::FileExt;
use crate::header::Header;
use crate::http::VERSION;
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::Server;
use crate::server::tests::MockTcpStream;

#[test]
fn check_range_response_is_ok_two_part() {
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

    let range_header_value = format!("bytes=0-{}, {}-{}", end_of_first_range, start_of_second_range ,buffer.len());

    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let host = Header {
        name: request_host_header_name.to_string().into(),
        value: request_host_header_value.to_string().into()
    };

    let range = Header {
        name: Header::_RANGE.into(),
        value: range_header_value.to_string().into()
    };

    let headers = vec![host, range];
    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers,
        body: vec![],
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

    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n206_partial_content.status_code);
    assert_eq!(response.reason_phrase, STATUS_CODE_REASON_PHRASE.n206_partial_content.reason_phrase);

    assert_eq!(VERSION.http_1_1, response.http_version);
    let header = response._get_header(Header::_X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();
    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF, header.value);
    let header = response._get_header(Header::_ACCEPT_RANGES.to_string()).unwrap();
    assert_eq!(Range::BYTES, header.value);
    let header = response._get_header(Header::_CONTENT_TYPE.to_string()).unwrap();
    assert_eq!(Range::multipart_content_type(&response.content_range_list), header.value);
    assert!(header.value.starts_with("multipart/byteranges; boundary=rws-"));

    let response_result_body : Vec<u8>;
    let first_range = response.content_range_list.get(0).unwrap();
    let first_body = first_range.body.clone();
    println!("first range:\n{:?}", &first_body);

    let second_range = response.content_range_list.get(1).unwrap();
    let second_body = second_range.body.clone();
    println!("second range:\n{:?}", &second_body);

    response_result_body = [first_body, second_body].concat();
    println!("concatenated ranges :\n{:?}", &response_result_body);

    assert_eq!(buffer, response_result_body);

    let result_string = String::from_utf8(response_result_body).unwrap();
    println!("result_string:\n{}", result_string);
}

#[test]
fn check_range_response_is_ok_single_part() {
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


    buffer = buffer[0..start_of_second_range].to_owned();


    let range_header_value = format!("bytes=0-{}", end_of_first_range);

    let request_host_header_name = "Host";
    let request_host_header_value = "localhost:7777";
    let host = Header {
        name: request_host_header_name.to_string().into(),
        value: request_host_header_value.to_string().into()
    };

    let range = Header {
        name: Header::_RANGE.into(),
        value: range_header_value.to_string().into()
    };

    let headers = vec![host, range];
    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers,
        body: vec![],
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

    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n206_partial_content.status_code);
    assert_eq!(response.reason_phrase, STATUS_CODE_REASON_PHRASE.n206_partial_content.reason_phrase);

    assert_eq!(VERSION.http_1_1, response.http_version);
    let header = response._get_header(Header::_X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();
    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF, header.value);
    let header = response._get_header(Header::_ACCEPT_RANGES.to_string()).unwrap();
    assert_eq!(Range::BYTES, header.value);
    let header = response._get_header(Header::_CONTENT_TYPE.to_string()).unwrap();
    let value = MimeType::TEXT_PLAIN.to_string();
    assert_eq!(value, header.value);

    let first_range = response.content_range_list.get(0).unwrap();
    let first_body = first_range.body.clone();
    println!("first range:\n{:?}", &first_body);

    assert_eq!(buffer, first_body);

}

#[test]
fn get_right_content_range_of_a_file() {
    let image_path = "/static/content.png";
    let static_filepath = FileExt::get_static_filepath(image_path).unwrap();
    let file_size = metadata(&static_filepath).unwrap().len();
    let whole = std::fs::read(&static_filepath).unwrap();

    // Disjoint ranges come back as requested, in order.
    let header = Header { name: Header::_RANGE.into(), value: "bytes=1200-1400, 200-1000, 2000-2300".into() };
    let list: Vec<ContentRange> = Range::get_content_range_list(image_path, &header).unwrap();
    let got: Vec<(u64, u64)> = list.iter().map(|c| (c.range.start, c.range.end)).collect();
    assert_eq!(vec![(1200, 1400), (200, 1000), (2000, 2300)], got);
    for c in &list {
        assert_eq!(c.content_type, MimeType::IMAGE_PNG);
        assert_eq!(c.size.parse::<u64>().unwrap(), file_size);
        assert_eq!(c.unit, Range::BYTES);
        assert_eq!(&whole[c.range.start as usize..=c.range.end as usize], &c.body[..]);
    }

    // Overlapping ranges (`0-` covers the rest) are merged (#46): one part.
    let header = Header { name: Header::_RANGE.into(), value: "bytes=200-1000, 1200-1400, 2000-2300, 11000-, -500, 0-, 0-1".into() };
    let list: Vec<ContentRange> = Range::get_content_range_list(image_path, &header).unwrap();
    assert_eq!(1, list.len());
    assert_eq!((0, file_size - 1), (list[0].range.start, list[0].range.end));
    assert_eq!(whole, list[0].body);
}

#[test]
fn parse_range_test() {
    let file_length = 2504382;

    let byte = "200-1000";
    let range = Range::parse_range_in_content_range(file_length, byte).unwrap();
    assert_eq!(range.start, 200);
    assert_eq!(range.end, 1000);

    let byte = " 1200-1400";
    let range = Range::parse_range_in_content_range(file_length, byte).unwrap();
    assert_eq!(range.start, 1200);
    assert_eq!(range.end, 1400);

    let byte = " 2000-2300 ";
    let range = Range::parse_range_in_content_range(file_length, byte).unwrap();
    assert_eq!(range.start, 2000);
    assert_eq!(range.end, 2300);

    // `end` is inclusive: open-ended and suffix ranges stop at the last byte.
    let byte = "  11000- ";
    let range = Range::parse_range_in_content_range(file_length, byte).unwrap();
    assert_eq!(range.start, 11000);
    assert_eq!(range.end, file_length - 1);

    let byte = " -500 ";
    let range = Range::parse_range_in_content_range(file_length, byte).unwrap();
    assert_eq!(range.start, file_length - 500);
    assert_eq!(range.end, file_length - 1);

    let byte = " 0- ";
    let range = Range::parse_range_in_content_range(file_length, byte).unwrap();
    assert_eq!(range.start, 0);
    assert_eq!(range.end, file_length - 1);

    // last-pos == length is one past the end: clamped to the last byte.
    let byte = ["0-", file_length.to_string().as_str()].join("");
    let range = Range::parse_range_in_content_range(file_length, &byte).unwrap();
    assert_eq!(range.start, 0);
    assert_eq!(range.end, file_length - 1);

    let byte = " 0-1 ";
    let range = Range::parse_range_in_content_range(file_length, byte).unwrap();
    assert_eq!(range.start, 0);
    assert_eq!(range.end, 1);
}


#[test]
fn content_range_raw_regex() {
    let start_num = 123;
    let end_num = 3212350;
    let size_num = 191238270;

    let string = format!("bytes {}-{}/{}", start_num, end_num, size_num);
    let (start, end, size) = Range::_parse_raw_content_range_header_value(string.as_str()).unwrap();


    assert_eq!(start_num, start);
    assert_eq!(end_num, end);
    assert_eq!(size_num, size);
}

#[test]
fn content_range_raw_regex_bytes_random_case() {
    let start_num = 123;
    let end_num = 3212350;
    let size_num = 191238270;

    let string = format!("bYtEs {}-{}/{}", start_num, end_num, size_num);
    let (start, end, size) = Range::_parse_raw_content_range_header_value(string.as_str()).unwrap();


    assert_eq!(start_num, start);
    assert_eq!(end_num, end);
    assert_eq!(size_num, size);
}

#[test]
fn content_range_raw_regex_malformed_bytes() {
    let start_num = 123;
    let end_num = 3212350;
    let size_num = 191238270;

    let string = format!("byytes {}-{}/{}", start_num, end_num, size_num);
    let boxed_result = Range::_parse_raw_content_range_header_value(string.as_str());
    assert!(boxed_result.is_err());

    assert_eq!(Range::_ERROR_UNABLE_TO_PARSE_CONTENT_RANGE.to_string(), boxed_result.err().unwrap());
}

#[test]
fn content_range_raw_regex_whitespace() {
    let start_num = 123;
    let end_num = 3212350;
    let size_num = 191238270;

    let string = format!("  bytes {}-{}/{}  ", start_num, end_num, size_num);
    let (start, end, size) = Range::_parse_raw_content_range_header_value(string.as_str()).unwrap();


    assert_eq!(start_num, start);
    assert_eq!(end_num, end);
    assert_eq!(size_num, size);
}

#[test]
fn content_range_raw_regex_whitespace_uppercase() {
    let start_num = 123;
    let end_num = 3212350;
    let size_num = 191238270;

    let string = format!("  BYTES {}-{}/{}  ", start_num, end_num, size_num);
    let (start, end, size) = Range::_parse_raw_content_range_header_value(string.as_str()).unwrap();


    assert_eq!(start_num, start);
    assert_eq!(end_num, end);
    assert_eq!(size_num, size);
}

#[test]
fn content_range_raw_regex_random_string() {
    let string = format!(" some string ");
    let boxed_result = Range::_parse_raw_content_range_header_value(string.as_str());
    assert!(boxed_result.is_err());

    assert_eq!(Range::_ERROR_UNABLE_TO_PARSE_CONTENT_RANGE.to_string(), boxed_result.err().unwrap());
}

#[test]
fn content_range_raw_regex_start_not_a_number() {
    let start_num = "text";
    let end_num = 3212350;
    let size_num = 191238270;

    let string = format!("  bytes {}-{}/{}  ", start_num, end_num, size_num);
    let boxed_result = Range::_parse_raw_content_range_header_value(string.as_str());
    assert!(boxed_result.is_err());

    assert_eq!(Range::_ERROR_UNABLE_TO_PARSE_CONTENT_RANGE.to_string(), boxed_result.err().unwrap());
}

#[test]
fn content_range_raw_regex_end_not_a_number() {
    let start_num = 123;
    let end_num = 3212350;
    let size_num = "text";

    let string = format!("  bytes {}-{}/{}  ", start_num, end_num, size_num);
    let boxed_result = Range::_parse_raw_content_range_header_value(string.as_str());
    assert!(boxed_result.is_err());

    assert_eq!(Range::_ERROR_UNABLE_TO_PARSE_CONTENT_RANGE.to_string(), boxed_result.err().unwrap());
}

#[test]
fn content_range_raw_regex_size_not_a_number() {
    let start_num = 123;
    let end_num = "sometext";
    let size_num = 191238270;

    let string = format!("  bytes {}-{}/{}  ", start_num, end_num, size_num);
    let boxed_result = Range::_parse_raw_content_range_header_value(string.as_str());
    assert!(boxed_result.is_err());

    assert_eq!(Range::_ERROR_UNABLE_TO_PARSE_CONTENT_RANGE.to_string(), boxed_result.err().unwrap());
}

#[test]
fn content_range_raw_regex_empty_string() {
    let string = format!("");
    let boxed_result = Range::_parse_raw_content_range_header_value(string.as_str());
    assert!(boxed_result.is_err());

    assert_eq!(Range::_ERROR_UNABLE_TO_PARSE_CONTENT_RANGE.to_string(), boxed_result.err().unwrap());
}

#[test]
fn parse_content_range_header_value() {
    let start_num = 123;
    let end_num = 3212350;
    let size_num = 191238270;

    let string = format!("bytes {}-{}/{}", start_num, end_num, size_num);
    let (start, end, size) = Range::_parse_content_range_header_value(string).unwrap();

    assert_eq!(start_num, start);
    assert_eq!(end_num, end);
    assert_eq!(size_num, size);
}

#[test]
fn start_after_end_parse_content_range_header_value() {
    let start_num = 3212350;
    let end_num = 123;
    let size_num = 191238270;

    let string = format!("bytes {}-{}/{}", start_num, end_num, size_num);
    let boxed_value = Range::_parse_content_range_header_value(string);
    assert_eq!(false, boxed_value.is_ok());

    let err = boxed_value.err().unwrap();

    assert_eq!(Range::ERROR_START_IS_AFTER_END_CONTENT_RANGE.to_string().to_string(), err);
}

#[test]
fn start_bigger_than_filesize_parse_content_range_header_value() {
    let start_num = 32000;
    let end_num = 32001;
    let size_num = 31000;

    let string = format!("bytes {}-{}/{}", start_num, end_num, size_num);
    let boxed_value = Range::_parse_content_range_header_value(string);
    assert_eq!(false, boxed_value.is_ok());

    let err = boxed_value.err().unwrap();

    assert_eq!(Range::ERROR_START_IS_BIGGER_THAN_FILESIZE_CONTENT_RANGE.to_string().to_string(), err);
}

#[test]
fn end_bigger_than_filesize_parse_content_range_header_value() {
    let start_num = 32000;
    let end_num = 32005;
    let size_num = 32001;

    let string = format!("bytes {}-{}/{}", start_num, end_num, size_num);
    let boxed_value = Range::_parse_content_range_header_value(string);
    assert_eq!(false, boxed_value.is_ok());

    let err = boxed_value.err().unwrap();

    assert_eq!(Range::ERROR_END_IS_BIGGER_THAN_FILESIZE_CONTENT_RANGE.to_string().to_string(), err);
}

#[test]
fn malformed_header_parse_content_range_header_value() {
    let string = format!("abracadabra");
    let boxed_value = Range::_parse_content_range_header_value(string);
    assert_eq!(false, boxed_value.is_ok());

    let err = boxed_value.err().unwrap();

    assert_eq!(Range::_ERROR_UNABLE_TO_PARSE_CONTENT_RANGE.to_string().to_string(), err);
}

#[test]
fn parse_multipart_body() {
    let size = 27;

    let first_range_start = 0;
    let first_range_end = 13;
    let first_range_body = "some text data";
    let first_range_content_type = MimeType::TEXT_PLAIN.to_string();

    let second_range_start = 14;
    let second_range_end = 27;
    let second_range_body = "\najlkdasjdasd";
    let second_range_content_type = MimeType::TEXT_PLAIN.to_string();


    let data = [
        "--String_separator\n",
        format!("Content-Type: {}\n", first_range_content_type).as_str(),
        format!("Content-Range: bytes {}-{}/{}\n", first_range_start, first_range_end, size).as_str(),
        "\n", // empty line - separator between header and body
        format!("{}\r\n", first_range_body).as_str(),
        "--String_separator\n",
        format!("Content-Type: {}\n", second_range_content_type).as_str(),
        format!("Content-Range: bytes {}-{}/{}\n", second_range_start, second_range_end, size).as_str(),
        "\n", // empty line - separator between header and body
        format!("{}\r\n", second_range_body).as_str(),
        "--String_separator"
    ].join("").to_string();

    use std::io::Cursor;
    let mut buff = Cursor::new(data.as_bytes());
    let mut content_range_list: Vec<ContentRange> = vec![];

    let boxed_result = Range::_parse_multipart_body(&mut buff, content_range_list);
    assert!(boxed_result.is_ok());
    content_range_list = boxed_result.unwrap();

    assert_eq!(2, content_range_list.len());

    let first_range = content_range_list.get(0).unwrap();
    assert_eq!(first_range.size, size.to_string());
    assert_eq!(first_range.range.start, first_range_start);
    assert_eq!(first_range.range.end, first_range_end);

    let first_body = first_range.body.clone();
    assert_eq!(first_body, first_range_body.as_bytes().to_vec());

    let second_range = content_range_list.get(1).unwrap();
    assert_eq!(second_range.size, size.to_string());
    assert_eq!(second_range.range.start, second_range_start);
    assert_eq!(second_range.range.end, second_range_end);

    let second_body = second_range.body.clone();
    assert_eq!(second_body, second_range_body.as_bytes().to_vec());
}

#[test]
fn no_empty_string_between_header_and_body_in_parse_multipart_body() {
    let size = 27;

    let first_range_start = 0;
    let first_range_end = 13;
    let first_range_body = "some text data";
    let first_range_content_type = MimeType::TEXT_PLAIN.to_string();

    let second_range_start = 14;
    let second_range_end = 27;
    let second_range_body = "\najlkdasjdasd";
    let second_range_content_type = MimeType::TEXT_PLAIN.to_string();


    let data = [
        "--String_separator\n",
        format!("Content-Type: {}\n", first_range_content_type).as_str(),
        format!("Content-Range: bytes {}-{}/{}\n", first_range_start, first_range_end, size).as_str(),
        format!("{}\r\n", first_range_body).as_str(),
        "--String_separator\n",
        format!("Content-Type: {}\n", second_range_content_type).as_str(),
        format!("Content-Range: bytes {}-{}/{}\n", second_range_start, second_range_end, size).as_str(),
        format!("{}\r\n", second_range_body).as_str(),
        "--String_separator"
    ].join("").to_string();

    use std::io::Cursor;
    let mut buff = Cursor::new(data.as_bytes());
    let content_range_list: Vec<ContentRange> = vec![];

    let boxed_result = Range::_parse_multipart_body(&mut buff, content_range_list);
    assert!(boxed_result.is_err());
    let error = boxed_result.err().unwrap();
    assert_eq!(error, Range::_ERROR_NO_EMPTY_LINE_BETWEEN_CONTENT_RANGE_HEADER_AND_BODY);

}
#[test]
fn parse_range_follows_rfc_9110_edge_cases() {
    let len = 10;
    let r = |s: &str| Range::parse_range_in_content_range(len, s);
    assert_eq!((0, 0), r("0-0").map(|r| (r.start, r.end)).unwrap(), "one byte");
    assert_eq!((2, 9), r("2-99").map(|r| (r.start, r.end)).unwrap(), "end past EOF is clamped");
    assert_eq!((0, 9), r("-99").map(|r| (r.start, r.end)).unwrap(), "oversized suffix = whole file");
    assert_eq!((7, 9), r("7-").map(|r| (r.start, r.end)).unwrap());
    assert!(r("10-").is_err(), "start at EOF");
    assert!(r("5-4").is_err(), "start after end");
    assert!(r("-0").is_err(), "empty suffix");
    assert!(r("abc-").is_err());
    assert!(Range::parse_range_in_content_range(0, "0-").is_err(), "empty resource");
}

#[test]
fn content_range_header_only_on_partial_content() {
    use crate::response::content_range_header_value;
    let cr = ContentRange {
        unit: Range::BYTES.to_string(),
        range: Range { start: 2, end: 5 },
        size: "10".to_string(),
        body: b"2345".to_vec(),
        content_type: MimeType::TEXT_PLAIN.to_string(),
    };
    assert_eq!(Some("bytes 2-5/10".to_string()), content_range_header_value(206, &cr));
    assert_eq!(None, content_range_header_value(200, &cr));
}

#[test]
fn static_file_range_requests_end_to_end() {
    use crate::test_client::TestClient;
    use crate::app::App;
    use crate::core::New;
    let size = metadata(FileExt::get_static_filepath("/static/content.png").unwrap()).unwrap().len();
    let client = TestClient::new(App::new());

    let full = client.get("/static/content.png").send();
    assert_eq!(200, full.status());
    assert_eq!(None, full.header("Content-Range"), "no Content-Range on 200");

    let tail = client.get("/static/content.png").header("Range", "bytes=-10").send();
    assert_eq!(206, tail.status());
    assert_eq!(Some(format!("bytes {}-{}/{}", size - 10, size - 1, size).as_str()), tail.header("Content-Range"));
    assert_eq!(Some("10"), tail.header("Content-Length"));

    let clamped = client.get("/static/content.png").header("Range", &format!("bytes=0-{}", size + 100)).send();
    assert_eq!(206, clamped.status());
    assert_eq!(Some(format!("bytes 0-{}/{}", size - 1, size).as_str()), clamped.header("Content-Range"));
}

// ── #46: byte-range limits ───────────────────────────────────────────────────

#[test]
fn many_overlapping_ranges_cost_one_copy_of_the_file() {
    let file = crate::test_env::TempPath::new("range_flood.bin");
    let content: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(&*file, &content).unwrap();
    // `bytes=0-,0-,…` used to read the whole file once per range.
    let header = format!("bytes={}", vec!["0-"; 2_000].join(","));
    let list = Range::parse_content_range(file.as_str(), content.len() as u64, &header).unwrap();
    assert_eq!(1, list.len());
    assert_eq!(content, list[0].body);
}

#[test]
fn ranges_are_sorted_merged_and_capped() {
    let r = |start, end| Range { start, end };
    // With an overlap: sorted, overlapping ones merged (adjacent ones stay).
    let merged = Range::coalesce(vec![r(50, 59), r(0, 9), r(10, 19), r(55, 70)], 100);
    assert_eq!(vec![(0, 9), (10, 19), (50, 70)], merged.iter().map(|r| (r.start, r.end)).collect::<Vec<_>>());
    // Without one: exactly as requested, order included.
    let as_asked = Range::coalesce(vec![r(50, 59), r(0, 9)], 100);
    assert_eq!(vec![(50, 59), (0, 9)], as_asked.iter().map(|r| (r.start, r.end)).collect::<Vec<_>>());
    // Disjoint ranges up to the cap stay parts.
    let sixteen: Vec<Range> = (0..16).map(|i| r(i * 10, i * 10 + 1)).collect();
    assert_eq!(16, Range::coalesce(sixteen, 1000).len());
    // Past the cap: the whole file as one range.
    let seventeen: Vec<Range> = (0..17).map(|i| r(i * 10, i * 10 + 1)).collect();
    let whole = Range::coalesce(seventeen, 1000);
    assert_eq!(vec![(0, 999)], whole.iter().map(|r| (r.start, r.end)).collect::<Vec<_>>());
    // One range is left alone.
    assert_eq!(vec![(5, 9)], Range::coalesce(vec![r(5, 9)], 100).iter().map(|r| (r.start, r.end)).collect::<Vec<_>>());
}
