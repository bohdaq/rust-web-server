use std::borrow::Borrow;
use std::fs::{File, metadata};
use std::io::{BufReader, Read, Seek, SeekFrom};
use regex::Regex;
use crate::constant::{HTTP_HEADERS, HTTP_VERSIONS, REQUEST_METHODS};
use crate::{CONSTANTS, Request, Response, Server};
use crate::header::Header;
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};
use crate::test::server_test::MockTcpStream;

#[test]
fn check_range_response_is_ok_two_part() {
    let uri = "/static/test.txt";
    let url = Server::get_static_filepath(uri);

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
        header_name: request_host_header_name.to_string(),
        header_value: request_host_header_value.to_string()
    };

    let range = Header {
        header_name: HTTP_HEADERS.RANGE.to_string(),
        header_value: range_header_value.to_string()
    };

    let headers = vec![host, range];
    let request = Request {
        method: REQUEST_METHODS.GET.to_string(),
        request_uri: uri.to_string(),
        http_version: HTTP_VERSIONS.HTTP_VERSION_1_1.to_string(),
        headers
    };

    let raw_request = Request::generate_request(request);

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream);

    let response = Response::parse_response(raw_response.borrow());

    let response_string = String::from_utf8(raw_response).unwrap();
    println!("\n\n\n{}", &raw_request);
    println!("\n\n\n{}", &response_string);

    assert_eq!(HTTP_VERSIONS.HTTP_VERSION_1_1, response.http_version);
    let header = response.get_header(HTTP_HEADERS.X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();
    assert_eq!(CONSTANTS.NOSNIFF, header.header_value);
    let header = response.get_header(HTTP_HEADERS.ACCEPT_RANGES.to_string()).unwrap();
    assert_eq!(CONSTANTS.BYTES, header.header_value);
    let header = response.get_header(HTTP_HEADERS.CONTENT_TYPE.to_string()).unwrap();
    let value = [
        CONSTANTS.MULTIPART,
        CONSTANTS.SLASH,
        CONSTANTS.BYTERANGES,
        CONSTANTS.SEMICOLON,
        CONSTANTS.WHITESPACE,
        CONSTANTS.BOUNDARY,
        CONSTANTS.EQUALS,
        CONSTANTS.STRING_SEPARATOR
    ].join("");
    assert_eq!(value, header.header_value);

    let mut response_result_body : Vec<u8> = vec![];
    let first_range = response.content_range_list.get(0).unwrap();
    let mut first_body = first_range.body.clone();
    println!("first range:\n{:?}", &first_body);

    let second_range = response.content_range_list.get(1).unwrap();
    let mut second_body = second_range.body.clone();
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
    let url = Server::get_static_filepath(uri);

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
        header_name: request_host_header_name.to_string(),
        header_value: request_host_header_value.to_string()
    };

    let range = Header {
        header_name: HTTP_HEADERS.RANGE.to_string(),
        header_value: range_header_value.to_string()
    };

    let headers = vec![host, range];
    let request = Request {
        method: REQUEST_METHODS.GET.to_string(),
        request_uri: uri.to_string(),
        http_version: HTTP_VERSIONS.HTTP_VERSION_1_1.to_string(),
        headers
    };

    let raw_request = Request::generate_request(request);

    let mock_tcp_stream = MockTcpStream {
        read_data: raw_request.as_bytes().to_vec(),
        write_data: vec![],
    };
    let raw_response: Vec<u8> = Server::process_request(mock_tcp_stream);

    let response = Response::parse_response(raw_response.borrow());

    let response_string = String::from_utf8(raw_response).unwrap();
    println!("\n\n\n{}", &raw_request);
    println!("\n\n\n{}", &response_string);

    assert_eq!(HTTP_VERSIONS.HTTP_VERSION_1_1, response.http_version);
    let header = response.get_header(HTTP_HEADERS.X_CONTENT_TYPE_OPTIONS.to_string()).unwrap();
    assert_eq!(CONSTANTS.NOSNIFF, header.header_value);
    let header = response.get_header(HTTP_HEADERS.ACCEPT_RANGES.to_string()).unwrap();
    assert_eq!(CONSTANTS.BYTES, header.header_value);
    let header = response.get_header(HTTP_HEADERS.CONTENT_TYPE.to_string()).unwrap();
    let value = MimeType::TEXT_PLAIN.to_string();
    assert_eq!(value, header.header_value);

    let mut response_result_body : Vec<u8> = vec![];
    let first_range = response.content_range_list.get(0).unwrap();
    let mut first_body = first_range.body.clone();
    println!("first range:\n{:?}", &first_body);

    assert_eq!(buffer, first_body);

}

#[test]
fn get_right_content_range_of_a_file() {
    let image_path = "/static/content.png";
    let static_filepath = Server::get_static_filepath(image_path);
    let md = metadata(&static_filepath).unwrap();
    let file_size = md.len();

    let header = Header {
        header_name: HTTP_HEADERS.RANGE.to_string(),
        header_value: "bytes=200-1000, 1200-1400, 2000-2300, 11000-, -500, 0-, 0-1".to_string()
    };

    let content_range_list : Vec<ContentRange> = Range::get_content_range_list(image_path, &header).unwrap();

    let start = 200;
    let end = 1000;
    let content_range = content_range_list.get(0).unwrap();
    assert_eq!(content_range.content_type, MimeType::IMAGE_PNG);
    assert_eq!(content_range.size.parse::<u64>().unwrap(), file_size);
    assert_eq!(content_range.unit, CONSTANTS.BYTES);
    assert_eq!(content_range.range.start, start);
    assert_eq!(content_range.range.end, end);
    let mut file = File::open(&static_filepath).unwrap();
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(start)).unwrap();
    let mut buff_length = (end - start) + 1;
    let mut buffer = Vec::new();
    reader.take(buff_length).read_to_end(&mut buffer).expect("Unable to read");
    assert_eq!(content_range.body, buffer);


    let start = 1200;
    let end = 1400;
    let content_range = content_range_list.get(1).unwrap();
    assert_eq!(content_range.content_type, MimeType::IMAGE_PNG);
    assert_eq!(content_range.size.parse::<u64>().unwrap(), file_size);
    assert_eq!(content_range.unit, CONSTANTS.BYTES);
    assert_eq!(content_range.range.start, start);
    assert_eq!(content_range.range.end, end);
    let mut file = File::open(&static_filepath).unwrap();
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(start)).unwrap();
    let mut buff_length = (end - start) + 1;
    let mut buffer = Vec::new();
    reader.take(buff_length).read_to_end(&mut buffer).expect("Unable to read");
    assert_eq!(content_range.body, buffer);

    let start = 2000;
    let end = 2300;
    let content_range = content_range_list.get(2).unwrap();
    assert_eq!(content_range.content_type, MimeType::IMAGE_PNG);
    assert_eq!(content_range.size.parse::<u64>().unwrap(), file_size);
    assert_eq!(content_range.unit, CONSTANTS.BYTES);
    assert_eq!(content_range.range.start, start);
    assert_eq!(content_range.range.end, end);
    let mut file = File::open(&static_filepath).unwrap();
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(start)).unwrap();
    let mut buff_length = (end - start) + 1;
    let mut buffer = Vec::new();
    reader.take(buff_length).read_to_end(&mut buffer).expect("Unable to read");
    assert_eq!(content_range.body, buffer);

    let start = 11000;
    let end = file_size;
    let content_range = content_range_list.get(3).unwrap();
    assert_eq!(content_range.content_type, MimeType::IMAGE_PNG);
    assert_eq!(content_range.size.parse::<u64>().unwrap(), file_size);
    assert_eq!(content_range.unit, CONSTANTS.BYTES);
    assert_eq!(content_range.range.start, start);
    assert_eq!(content_range.range.end, end);
    let mut file = File::open(&static_filepath).unwrap();
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(start)).unwrap();
    let mut buff_length = (end - start) + 1;
    let mut buffer = Vec::new();
    reader.take(buff_length).read_to_end(&mut buffer).expect("Unable to read");
    assert_eq!(content_range.body, buffer);

    let content_range = content_range_list.get(4).unwrap();
    assert_eq!(content_range.content_type, MimeType::IMAGE_PNG);
    assert_eq!(content_range.size.parse::<u64>().unwrap(), file_size);
    assert_eq!(content_range.unit, CONSTANTS.BYTES);
    let start = file_size - 500;
    let end = file_size;
    assert_eq!(content_range.range.start, start);
    assert_eq!(content_range.range.end, end);
    let mut file = File::open(&static_filepath).unwrap();
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(start)).unwrap();
    let mut buff_length = (end - start) + 1;
    let mut buffer = Vec::new();
    reader.take(buff_length).read_to_end(&mut buffer).expect("Unable to read");
    assert_eq!(content_range.body, buffer);

    let start = 0;
    let end = file_size;
    let content_range = content_range_list.get(5).unwrap();
    assert_eq!(content_range.content_type, MimeType::IMAGE_PNG);
    assert_eq!(content_range.size.parse::<u64>().unwrap(), file_size);
    assert_eq!(content_range.unit, CONSTANTS.BYTES);
    assert_eq!(content_range.range.start, start);
    assert_eq!(content_range.range.end, end);
    let mut file = File::open(&static_filepath).unwrap();
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(start)).unwrap();
    let mut buff_length = (end - start) + 1;
    let mut buffer = Vec::new();
    reader.take(buff_length).read_to_end(&mut buffer).expect("Unable to read");
    assert_eq!(content_range.body, buffer);


    let start = 0;
    let end = 1;
    let content_range = content_range_list.get(6).unwrap();
    assert_eq!(content_range.content_type, MimeType::IMAGE_PNG);
    assert_eq!(content_range.size.parse::<u64>().unwrap(), file_size);
    assert_eq!(content_range.unit, CONSTANTS.BYTES);
    assert_eq!(content_range.range.start, start);
    assert_eq!(content_range.range.end, end);
    let mut file = File::open(&static_filepath).unwrap();
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(start)).unwrap();
    let mut buff_length = (end - start) + 1;
    let mut buffer = Vec::new();
    reader.take(buff_length).read_to_end(&mut buffer).expect("Unable to read");
    assert_eq!(content_range.body, buffer);
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

    let byte = "  11000- ";
    let range = Range::parse_range_in_content_range(file_length, byte).unwrap();
    assert_eq!(range.start, 11000);
    assert_eq!(range.end, file_length);

    let byte = " -500 ";
    let range = Range::parse_range_in_content_range(file_length, byte).unwrap();
    assert_eq!(range.start, file_length - 500);
    assert_eq!(range.end, file_length);

    let byte = " 0- ";
    let range = Range::parse_range_in_content_range(file_length, byte).unwrap();
    assert_eq!(range.start, 0);
    assert_eq!(range.end, file_length);

    let byte = ["0-", file_length.to_string().as_str()].join("");
    let range = Range::parse_range_in_content_range(file_length, &byte).unwrap();
    assert_eq!(range.start, 0);
    assert_eq!(range.end, file_length);

    let byte = " 0-1 ";
    let range = Range::parse_range_in_content_range(file_length, byte).unwrap();
    assert_eq!(range.start, 0);
    assert_eq!(range.end, 1);
}

#[test]
fn content_range_regex() {
    let start_num = 123;
    let end_num = 3212350;
    let size_num = 191238270;

    let string = format!("bytes {}-{}/{}", start_num, end_num, size_num);
    let re = Regex::new(Range::CONTENT_RANGE_REGEX).unwrap();
    let caps = re.captures(string.as_str()).unwrap();

    let start= &caps["start"];
    let end = &caps["end"];
    let size = &caps["size"];

    let size = size.parse().unwrap();
    let start = start.parse().unwrap();
    let end = end.parse().unwrap();

    assert_eq!(start_num, start);
    assert_eq!(end_num, end);
    assert_eq!(size_num, size);
}

#[test]
fn parse_content_range_header_value() {
    let start_num = 123;
    let end_num = 3212350;
    let size_num = 191238270;

    let string = format!("bytes {}-{}/{}", start_num, end_num, size_num);
    let (size, start, end) = Range::parse_content_range_header_value(string).unwrap();

    assert_eq!(start_num, start);
    assert_eq!(end_num, end);
    assert_eq!(size_num, size.parse().unwrap());
}

#[test]
fn start_after_end_parse_content_range_header_value() {
    let start_num = 3212350;
    let end_num = 123;
    let size_num = 191238270;

    let string = format!("bytes {}-{}/{}", start_num, end_num, size_num);
    let boxed_value = Range::parse_content_range_header_value(string);
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
    let boxed_value = Range::parse_content_range_header_value(string);
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
    let boxed_value = Range::parse_content_range_header_value(string);
    assert_eq!(false, boxed_value.is_ok());

    let err = boxed_value.err().unwrap();

    assert_eq!(Range::ERROR_END_IS_BIGGER_THAN_FILESIZE_CONTENT_RANGE.to_string().to_string(), err);
}

#[test]
fn malformed_header_parse_content_range_header_value() {
    let string = format!("abracadabra");
    let boxed_value = Range::parse_content_range_header_value(string);
    assert_eq!(false, boxed_value.is_ok());

    let err = boxed_value.err().unwrap();

    assert_eq!(Range::ERROR_UNABLE_TO_PARSE_CONTENT_RANGE.to_string().to_string(), err);
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

    let boxed_result = Range::parse_multipart_body(&mut buff, content_range_list);
    assert!(boxed_result.is_ok());
    content_range_list = boxed_result.unwrap();

    assert_eq!(2, content_range_list.len());

    let first_range = content_range_list.get(0).unwrap();
    assert_eq!(first_range.size, size.to_string());
    assert_eq!(first_range.range.start, first_range_start);
    assert_eq!(first_range.range.end, first_range_end);

    let mut first_body = first_range.body.clone();
    assert_eq!(first_body, first_range_body.as_bytes().to_vec());

    let second_range = content_range_list.get(1).unwrap();
    assert_eq!(second_range.size, size.to_string());
    assert_eq!(second_range.range.start, second_range_start);
    assert_eq!(second_range.range.end, second_range_end);

    let mut second_body = second_range.body.clone();
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

    let boxed_result = Range::parse_multipart_body(&mut buff, content_range_list);
    assert!(boxed_result.is_err());
    let error = boxed_result.err().unwrap();
    assert_eq!(error, Range::ERROR_NO_EMPTY_LINE_BETWEEN_CONTENT_RANGE_HEADER_AND_BODY);

}