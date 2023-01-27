use crate::http::VERSION;
use crate::request::{METHOD, Request};
use crate::symbol::SYMBOL;

#[test]
fn method() {
    assert_eq!(METHOD.get, "GET");
    assert_eq!(METHOD.head, "HEAD");
    assert_eq!(METHOD.post, "POST");
    assert_eq!(METHOD.put, "PUT");
    assert_eq!(METHOD.delete, "DELETE");
    assert_eq!(METHOD.connect, "CONNECT");
    assert_eq!(METHOD.options, "OPTIONS");
    assert_eq!(METHOD.trace, "TRACE");
    assert_eq!(METHOD.patch, "PATCH");
}

#[test]
fn method_and_request_uri_and_http_version_regex() {
    let method_request_uri_version = "GET / HTTP/1.1";
    let (method, request_uri, http_version) = Request::parse_method_and_request_uri_and_http_version_string(method_request_uri_version).unwrap();

    assert_eq!(VERSION.http_1_1, http_version.to_uppercase());
    assert_eq!(METHOD.get, method.to_uppercase());
    assert_eq!(SYMBOL.slash, &request_uri);


    let method_request_uri_version = "GET /draho-brat_pt2/drahobrat_pt2_ver2.mp4 HTTP/1.1";
    let (method, request_uri, http_version) = Request::parse_method_and_request_uri_and_http_version_string(method_request_uri_version).unwrap();


    assert_eq!(VERSION.http_1_1, http_version.to_uppercase());
    assert_eq!(METHOD.get, method.to_uppercase());
    assert_eq!("/draho-brat_pt2/drahobrat_pt2_ver2.mp4", &request_uri);

}

#[test]
fn test_request_ok() {
    let method = METHOD.get;
    let request_uri = SYMBOL.slash;
    let http_version = VERSION.http_1_1;

    let request_data = [method, request_uri, http_version].join(" ");
    let raw_request = [request_data, SYMBOL.new_line_carriage_return.to_string()].join("");

    let request = Request::parse_request(raw_request.as_bytes()).unwrap();

    assert_eq!(method, request.method);
    assert_eq!(request_uri, request.request_uri);
    assert_eq!(http_version, request.http_version);
}

#[test]
fn test_request_ok_with_special_characters() {
    let method = METHOD.get;
    let special_characters = "_:;.,/\"'?!(){}[]@<>=-+*#$&`|~^%";
    let request_uri = [SYMBOL.slash, special_characters].join("");
    let http_version = VERSION.http_1_1;


    let request_data = [method, request_uri.as_str(), http_version].join(" ");
    let raw_request = [request_data, SYMBOL.new_line_carriage_return.to_string()].join("");

    let request = Request::parse_request(raw_request.as_bytes()).unwrap();

    assert_eq!(method, request.method);
    assert_eq!(request_uri, request.request_uri);
    assert_eq!(http_version, request.http_version);
}

#[test]
fn test_request_ok_with_ukrainian_characters() {
    let method = METHOD.get;
    let ukrainian_characters = "АаБбВвГгҐґДдЕеЄєЖжЗзИиІіЇїЙйКкЛлМмНнОоПпРрСсТтУуФфХхЦцЧчШшЩщЬьЮюЯя";
    let request_uri = [SYMBOL.slash, ukrainian_characters].join("");
    let http_version = VERSION.http_1_1;


    let request_data = [method, request_uri.as_str(), http_version].join(" ");
    let raw_request = [request_data, SYMBOL.new_line_carriage_return.to_string()].join("");

    let request = Request::parse_request(raw_request.as_bytes()).unwrap();

    assert_eq!(method, request.method);
    assert_eq!(request_uri, request.request_uri);
    assert_eq!(http_version, request.http_version);
}

#[test]
fn test_request_not_ok() {
    let method = METHOD.get;
    let request_uri = [SYMBOL.slash, SYMBOL.whitespace, SYMBOL.hyphen].join("");
    let http_version = VERSION.http_1_1;

    let request_data = [method, request_uri.as_str(), http_version].join(" ");
    let raw_request = [request_data, SYMBOL.new_line_carriage_return.to_string()].join("");

    let boxed_request = Request::parse_request(raw_request.as_bytes());
    assert_eq!(true, boxed_request.is_err());

    assert_eq!(Request::_ERROR_UNABLE_TO_PARSE_METHOD_AND_REQUEST_URI_AND_HTTP_VERSION, boxed_request.err().unwrap());
}

#[test]
fn test_request_not_ok_empty_request() {
    let boxed_request = Request::parse_request(b"");
    assert_eq!(true, boxed_request.is_err());

    assert_eq!(Request::_ERROR_UNABLE_TO_PARSE_METHOD_AND_REQUEST_URI_AND_HTTP_VERSION, boxed_request.err().unwrap());
}

#[test]
fn test_request_not_ok_dummy_not_valid_request() {
    let dummy_request = "some dummy not valid request";
    let boxed_request = Request::parse_request(dummy_request.as_bytes());
    assert_eq!(true, boxed_request.is_err());

    assert_eq!(Request::_ERROR_UNABLE_TO_PARSE_METHOD_AND_REQUEST_URI_AND_HTTP_VERSION, boxed_request.err().unwrap());
}

#[test]
fn test_request_not_ok_zeros_request() {
    let dummy_request = b"00000000";
    let boxed_request = Request::parse_request(dummy_request);
    assert_eq!(true, boxed_request.is_err());

    assert_eq!(Request::_ERROR_UNABLE_TO_PARSE_METHOD_AND_REQUEST_URI_AND_HTTP_VERSION, boxed_request.err().unwrap());
}

#[test]
fn test_request_empty_request_uri() {
    let method = METHOD.get;
    let request_uri = "";
    let http_version = VERSION.http_1_1;

    let request_line = [method, request_uri, http_version].join(" ");

    let boxed_request = Request::parse_request(request_line.as_bytes());

    let request = boxed_request.unwrap();

    assert_eq!(method, request.method);
    assert_eq!(request_uri, request.request_uri);
    assert_eq!(http_version, request.http_version);
}

#[test]
fn test_request_lowercase() {
    let method = METHOD.get.to_lowercase();
    let request_uri = "/path";
    let http_version = VERSION.http_1_1.to_lowercase();

    let request_line = [method.to_string(), request_uri.to_string(), http_version.to_string()].join(" ");

    let boxed_request = Request::parse_request(request_line.as_bytes());

    let request = boxed_request.unwrap();

    assert_eq!(method, request.method);
    assert_eq!(request_uri, request.request_uri);
    assert_eq!(http_version, request.http_version);
}

#[test]
fn test_request_lowercase_not_valid_utf_8() {
    let mut non_utf8_char: Vec<u8> = vec![255];

    let method = METHOD.get.to_lowercase();
    let request_uri = "/path";
    let http_version = VERSION.http_1_1.to_lowercase();

    let mut request_vec : Vec<u8> = vec![];
    request_vec.append(&mut method.as_bytes().to_vec());
    request_vec.append(&mut request_uri.as_bytes().to_vec());
    request_vec.append(&mut non_utf8_char);
    request_vec.append(&mut http_version.as_bytes().to_vec());

    let boxed_request = Request::parse_request(&request_vec);

    assert_eq!(true, boxed_request.is_err());
    assert_eq!("invalid utf-8 sequence of 1 bytes from index 8", boxed_request.err().unwrap());
}

#[test]
fn test_request_randomcase() {
    let method = "GeT";
    let request_uri = "/path";
    let http_version = "HtTP/1.1";

    let request_line = [method.to_string(), request_uri.to_string(), http_version.to_string()].join(" ");

    let boxed_request = Request::parse_request(request_line.as_bytes());

    let request = boxed_request.unwrap();

    assert_eq!(method, request.method);
    assert_eq!(request_uri, request.request_uri);
    assert_eq!(http_version, request.http_version);
}

#[test]
fn file_upload_text_plain_content_type() {

    //let raw_request = String::from_utf8(Vec::from(request)).unwrap();
    //println!("\n\n______{}______\n\n", raw_request);

    let raw_request_1 = format!("POST /file-upload HTTP/1.1{}", SYMBOL.new_line_carriage_return);
    let raw_request_2 = format!("Content-Type: text/plain{}", SYMBOL.new_line_carriage_return);
    let raw_request_3 = format!("Host: 127.0.0.1:7888{}", SYMBOL.new_line_carriage_return);
    let raw_request_4 = SYMBOL.new_line_carriage_return.to_string();
    let raw_request_5 = format!("some-prop 1234{}", SYMBOL.new_line_carriage_return);

    let raw_request = [
        raw_request_1,
        raw_request_2,
        raw_request_3,
        raw_request_4,
        raw_request_5,
    ].join(SYMBOL.empty_string);

    let boxed_request = Request::parse_request(raw_request.as_bytes());

    assert!(boxed_request.is_ok());
}


#[test]
fn file_upload_multipart_form_data_content_type() {

    //let raw_request = String::from_utf8(Vec::from(request)).unwrap();
    //println!("\n\n______{}______\n\n", raw_request);
    let boundary = "------hdfkjshdfkljashdgkh";


    let new_line = SYMBOL.new_line_carriage_return.to_string();


    let payload = "123".to_string();
    let payload_boundary = format!("{}{}", boundary,  SYMBOL.new_line_carriage_return);
    let content_disposition = format!("Content-Disposition: form-data; name=\"some\"{}", SYMBOL.new_line_carriage_return);;
    let raw_payload_1 = [
        payload_boundary,
        content_disposition,
        new_line.to_string(),
        payload,
        new_line.to_string(),
    ].join(SYMBOL.empty_string);


    let payload = "45678".to_string();
    let payload_boundary = format!("{}{}", boundary,  SYMBOL.new_line_carriage_return);
    let content_disposition = format!("Content-Disposition: form-data; name=\"key\"{}", SYMBOL.new_line_carriage_return);;
    let raw_payload_2 = [
        payload_boundary,
        content_disposition,
        new_line.to_string(),
        payload,
        new_line.to_string(),
    ].join(SYMBOL.empty_string);

    let raw_payload = [
        raw_payload_1,
        raw_payload_2,
        boundary.to_string(),
    ].join(SYMBOL.empty_string);

    let uri = "/file-upload";
    let method = "POST";
    let http_version = "HTTP/1.1";
    let content_type = format!("multipart/form-data; boundary={}", boundary);

    let raw_request_1 = format!("{} {} {} {}", method, uri, http_version, SYMBOL.new_line_carriage_return);
    let raw_request_2 = format!("Content-Type: {}{}", content_type, SYMBOL.new_line_carriage_return);
    let raw_request_3 = format!("Host: 127.0.0.1:7888{}", SYMBOL.new_line_carriage_return);
    let raw_request_4 = SYMBOL.new_line_carriage_return.to_string();
    let raw_request_5 = raw_payload;

    let raw_request = [
        raw_request_1,
        raw_request_2,
        raw_request_3,
        raw_request_4,
        raw_request_5,
    ].join(SYMBOL.empty_string);

    let boxed_request = Request::parse_request(raw_request.as_bytes());

    assert!(boxed_request.is_ok());

    let request = boxed_request.unwrap();
    assert_eq!(uri, request.request_uri);
    assert_eq!(method, request.method);
    assert_eq!(http_version, request.http_version);
}