use std::borrow::Borrow;
use std::fs::{File, metadata};
use std::io::{BufReader, Read, Seek, SeekFrom};
use regex::Regex;
use crate::constant::{HTTP_HEADERS, HTTP_VERSIONS, REQUEST_METHODS};
use crate::{CONSTANTS, Request, Response, Server};
use crate::header::Header;
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};

#[test]
fn check_range_response_is_ok() {
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
    let request: Request = Request::parse_request(&raw_request.as_bytes());
    let raw_response = Server::process_request(raw_request.as_bytes());

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
    first_body.pop(); // remove \n
    first_body.pop(); // remove \r
    println!("first range:\n{:?}", &first_body);

    let second_range = response.content_range_list.get(1).unwrap();
    let mut second_body = second_range.body.clone();
    second_body.pop(); // remove \n
    second_body.pop(); // remove \r
    println!("second range:\n{:?}", &second_body);

    response_result_body = [first_body, second_body].concat();
    println!("concatenated ranges :\n{:?}", &response_result_body);

    assert_eq!(buffer, response_result_body);

    let result_string = String::from_utf8(response_result_body).unwrap();
    println!("result_string:\n{}", result_string);
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

    let content_range_list : Vec<ContentRange> = Range::get_content_range_list(image_path, &header);

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
    reader.seek(SeekFrom::Start(start));
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
    reader.seek(SeekFrom::Start(start));
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
    reader.seek(SeekFrom::Start(start));
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
    reader.seek(SeekFrom::Start(start));
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
    reader.seek(SeekFrom::Start(start));
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
    reader.seek(SeekFrom::Start(start));
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
    reader.seek(SeekFrom::Start(start));
    let mut buff_length = (end - start) + 1;
    let mut buffer = Vec::new();
    reader.take(buff_length).read_to_end(&mut buffer).expect("Unable to read");
    assert_eq!(content_range.body, buffer);
}

#[test]
fn parse_range_test() {
    let file_length = 2504382;

    let byte = "200-1000";
    let range = Range::parse_range(file_length, byte);
    assert_eq!(range.start, 200);
    assert_eq!(range.end, 1000);

    let byte = " 1200-1400";
    let range = Range::parse_range(file_length, byte);
    assert_eq!(range.start, 1200);
    assert_eq!(range.end, 1400);

    let byte = " 2000-2300 ";
    let range = Range::parse_range(file_length, byte);
    assert_eq!(range.start, 2000);
    assert_eq!(range.end, 2300);

    let byte = "  11000- ";
    let range = Range::parse_range(file_length, byte);
    assert_eq!(range.start, 11000);
    assert_eq!(range.end, file_length);

    let byte = " -500 ";
    let range = Range::parse_range(file_length, byte);
    assert_eq!(range.start, file_length - 500);
    assert_eq!(range.end, file_length);

    let byte = " 0- ";
    let range = Range::parse_range(file_length, byte);
    assert_eq!(range.start, 0);
    assert_eq!(range.end, file_length);

    let byte = ["0-", file_length.to_string().as_str()].join("");
    let range = Range::parse_range(file_length, &byte);
    assert_eq!(range.start, 0);
    assert_eq!(range.end, file_length);

    let byte = " 0-1 ";
    let range = Range::parse_range(file_length, byte);
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

