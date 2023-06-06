use std::borrow::Borrow;
use std::env;
use std::fs::File;
use std::io::Read;
use file_ext::FileExt;
use crate::header::Header;
use crate::http::VERSION;
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};
use crate::request::{METHOD, Request};
use crate::response::{Error, Response, STATUS_CODE_REASON_PHRASE};
use crate::symbol::SYMBOL;

#[test]
fn check_is_multipart_byteranges_content_type() {
    let content_type = Header {
        name: Header::_CONTENT_TYPE.to_string(),
        value: "multipart/byteranges; boundary=String_separator".to_string(),
    };

    let is_multipart = Response::_is_multipart_byteranges_content_type(&content_type);
    assert_eq!(true, is_multipart);
}

#[test]
fn error() {
    let error = Error {
        status_code_reason_phrase: STATUS_CODE_REASON_PHRASE.n200_ok,
        message: "some msg".to_string()
    };

    let clone = error.clone();

    assert_eq!(error, clone);
    assert_eq!("some msg", error.message);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n200_ok, error.status_code_reason_phrase);
}

#[test]
fn http_version_and_status_code_and_reason_phrase_404_regex() {

    let http_version_and_status_code_and_reason_phrase = "HTTP/1.1 404 Not Found";
    let boxed_parse = Response::_parse_http_version_status_code_reason_phrase_string(http_version_and_status_code_and_reason_phrase);

    assert!(boxed_parse.is_ok());
    let (http_version, status_code, reason_phrase) = boxed_parse.unwrap();

    assert_eq!(VERSION.http_1_1, http_version);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n404_not_found.status_code, &status_code);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n404_not_found.reason_phrase, reason_phrase);

}

#[test]
fn http_version_and_status_code_and_reason_phrase_200_regex() {

    let http_version_and_status_code_and_reason_phrase = "HTTP/1.1 200 OK";
    let boxed_parse = Response::_parse_http_version_status_code_reason_phrase_string(http_version_and_status_code_and_reason_phrase);

    assert!(boxed_parse.is_ok());
    let (http_version, status_code, reason_phrase) = boxed_parse.unwrap();

    assert_eq!(VERSION.http_1_1, http_version);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n200_ok.status_code, &status_code);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase, reason_phrase);
}

#[test]
fn http_version_and_status_code_and_reason_phrase_200_regex_random_case() {

    let http_version_and_status_code_and_reason_phrase = "hTTp/1.1 200 Ok";
    let boxed_parse = Response::_parse_http_version_status_code_reason_phrase_string(http_version_and_status_code_and_reason_phrase);

    assert!(boxed_parse.is_ok());
    let (http_version, status_code, reason_phrase) = boxed_parse.unwrap();

    assert_eq!(VERSION.http_1_1.to_uppercase(), http_version.to_uppercase());
    assert_eq!(STATUS_CODE_REASON_PHRASE.n200_ok.status_code, &status_code);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_uppercase(), reason_phrase.to_uppercase());
}

#[test]
fn http_version_and_status_code_and_reason_phrase_empty_string() {

    let http_version_and_status_code_and_reason_phrase = "";
    let boxed_parse = Response::_parse_http_version_status_code_reason_phrase_string(http_version_and_status_code_and_reason_phrase);

    assert!(!boxed_parse.is_ok());
    let error_msg = boxed_parse.err().unwrap();

    assert_eq!("Unable to parse status code", error_msg);
}

#[test]
fn http_version_and_status_code_and_reason_phrase_empty_string_newline() {

    let http_version_and_status_code_and_reason_phrase = "hTTp/1.1 200 Ok\r\n";
    let boxed_parse = Response::_parse_http_version_status_code_reason_phrase_string(http_version_and_status_code_and_reason_phrase);

    assert!(boxed_parse.is_ok());
    let (http_version, status_code, reason_phrase) = boxed_parse.unwrap();

    assert_eq!(VERSION.http_1_1, http_version.to_uppercase());
    assert_eq!(STATUS_CODE_REASON_PHRASE.n200_ok.status_code, &status_code);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase, reason_phrase.to_uppercase());

}


#[test]
fn it_generates_successful_response_with_additional_headers() {
    let response_http_version = VERSION.http_1_1.to_string();
    let response_status_code = 401;
    let response_reason_phrase = "Unauthorized";
    let message_body = SYMBOL.empty_string;


    let content_range = ContentRange {
        unit: Range::BYTES.to_string(),
        range: Range {
            start: 0,
            end: message_body.as_bytes().len() as u64
        },
        size: message_body.as_bytes().len().to_string(),
        body: message_body.as_bytes().to_vec(),
        content_type: MimeType::TEXT_PLAIN.to_string()
    };

    let headers = vec![];
    let response = Response {
        http_version: response_http_version.to_string(),
        status_code: response_status_code,
        reason_phrase: response_reason_phrase.to_string(),
        headers,
        content_range_list: vec![content_range],
    };


    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = message_body.len().to_string();
    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/some-route".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let raw_response = Response::generate_response(response, request);
    let response = Response::_parse_response(raw_response.borrow());


    let content_length_header = response._get_header(response_content_length_header_name.to_string()).unwrap();
    assert_eq!(response_content_length_header_value, content_length_header.value);


    assert_eq!(response_http_version, response.http_version);
    assert_eq!(response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);
    assert_eq!(message_body.as_bytes().to_vec(), response.content_range_list.get(0).unwrap().body);

    let response_clone = response.clone();
    assert_eq!(response, response_clone);
}

#[test]
fn it_generates_successful_response_with_additional_headers_and_non_utf8_file() {
    let response_http_version = VERSION.http_1_1.to_string();
    let response_status_code = STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    let response_reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase;
    let filepath = "/static/content.png";

    let dir = env::current_dir().unwrap();
    let working_directory = dir.as_path().to_str().unwrap();

    let mut response_filepath = [working_directory, filepath].join(SYMBOL.empty_string);
    let mut contents = Vec::new();
    let mut file = File::open(response_filepath).unwrap();
    file.read_to_end(&mut contents).expect("Unable to read");

    let response_content_length_header_name = "Content-Length";
    let response_content_length_header_value = contents.len().to_string();

    let headers = vec![];

    let content_range = ContentRange {
        unit: Range::BYTES.to_string(),
        range: Range {
            start: 0,
            end: contents.len() as u64
        },
        size: contents.len().to_string(),
        body: contents.to_vec(),
        content_type: MimeType::IMAGE_PNG.to_string()
    };

    let response = Response {
        http_version: response_http_version.to_string(),
        status_code: *response_status_code,
        reason_phrase: response_reason_phrase.to_string(),
        headers,
        content_range_list: vec![content_range],
    };

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/some-route".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let raw_response = Response::generate_response(response, request);
    let response = Response::_parse_response(raw_response.borrow());


    let content_length_header = response._get_header(response_content_length_header_name.to_string()).unwrap();
    assert_eq!(response_content_length_header_value, content_length_header.value);


    assert_eq!(response_http_version, response.http_version);
    assert_eq!(*response_status_code, response.status_code);
    assert_eq!(response_reason_phrase, response.reason_phrase);

    contents = Vec::new();
    response_filepath = [working_directory, filepath].join(SYMBOL.empty_string);
    file = File::open(response_filepath).unwrap();
    file.read_to_end(&mut contents).expect("Unable to read");
    assert_eq!(contents, response.content_range_list.get(0).unwrap().body);
}

#[test]
fn status_code_reason_phrase() {
    assert_eq!(STATUS_CODE_REASON_PHRASE.n500_internal_server_error.status_code, &500);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n500_internal_server_error.reason_phrase, "Internal Server Error");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n501_not_implemented.status_code, &501);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n501_not_implemented.reason_phrase, "Not Implemented");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n502_bad_gateway.status_code, &502);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n502_bad_gateway.reason_phrase, "Bad Gateway");


    assert_eq!(STATUS_CODE_REASON_PHRASE.n503_service_unavailable.status_code, &503);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n503_service_unavailable.reason_phrase, "Service Unavailable");


    assert_eq!(STATUS_CODE_REASON_PHRASE.n504_gateway_timeout.status_code, &504);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n504_gateway_timeout.reason_phrase, "Gateway Timeout");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n505_http_version_not_supported.status_code, &505);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n505_http_version_not_supported.reason_phrase, "HTTP Version Not Supported");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n506_variant_also_negotiates.status_code, &506);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n506_variant_also_negotiates.reason_phrase, "Variant Also Negotiates");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n507_insufficient_storage.status_code, &507);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n507_insufficient_storage.reason_phrase, "Insufficient Storage");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n508_loop_detected.status_code, &508);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n508_loop_detected.reason_phrase, "Loop Detected");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n510_not_extended.status_code, &510);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n510_not_extended.reason_phrase, "Not Extended");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n511_network_authentication_required.status_code, &511);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n511_network_authentication_required.reason_phrase, "Network Authentication Required");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n200_ok.status_code, &200);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase, "OK");


    assert_eq!(STATUS_CODE_REASON_PHRASE.n201_created.status_code, &201);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n201_created.reason_phrase, "Created");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n202_accepted.status_code, &202);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n202_accepted.reason_phrase, "Accepted");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n203_non_authoritative_information.status_code, &203);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n203_non_authoritative_information.reason_phrase, "Non Authoritative Information");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n204_no_content.status_code, &204);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n204_no_content.reason_phrase, "No Content");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n205_reset_content.status_code, &205);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n205_reset_content.reason_phrase, "Reset Content");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n206_partial_content.status_code, &206);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n206_partial_content.reason_phrase, "Partial Content");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n207_multi_status.status_code, &207);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n207_multi_status.reason_phrase, "Multi-Status");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n208_already_reported.status_code, &208);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n208_already_reported.reason_phrase, "Already Reported");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n226_im_used.status_code, &226);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n226_im_used.reason_phrase, "IM Used");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n100_continue.status_code, &100);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n100_continue.reason_phrase, "Continue");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n101_switching_protocols.status_code, &101);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n101_switching_protocols.reason_phrase, "Switching Protocols");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n102_processing.status_code, &102);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n102_processing.reason_phrase, "Processing");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n103_early_hints.status_code, &103);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n103_early_hints.reason_phrase, "Early Hints");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n300_multiple_choices.status_code, &300);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n300_multiple_choices.reason_phrase, "Multiple Choices");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n301_moved_permanently.status_code, &301);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n301_moved_permanently.reason_phrase, "Moved Permanently");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n302_found.status_code, &302);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n302_found.reason_phrase, "Found");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n303_see_other.status_code, &303);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n303_see_other.reason_phrase, "See Other");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n304_not_modified.status_code, &304);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n304_not_modified.reason_phrase, "Not Modified");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n307_temporary_redirect.status_code, &307);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n307_temporary_redirect.reason_phrase, "Temporary Redirect");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n308_permanent_redirect.status_code, &308);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n308_permanent_redirect.reason_phrase, "Permanent Redirect");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n400_bad_request.status_code, &400);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n400_bad_request.reason_phrase, "Bad Request");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n401_unauthorized.status_code, &401);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n401_unauthorized.reason_phrase, "Unauthorized");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n402_payment_required.status_code, &402);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n402_payment_required.reason_phrase, "Payment Required");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n403_forbidden.status_code, &403);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n403_forbidden.reason_phrase, "Forbidden");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n404_not_found.status_code, &404);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n404_not_found.reason_phrase, "Not Found");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n405_method_not_allowed.status_code, &405);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n405_method_not_allowed.reason_phrase, "Method Not Allowed");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n406_not_acceptable.status_code, &406);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n406_not_acceptable.reason_phrase, "Not Acceptable");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n407_proxy_authentication_required.status_code, &407);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n407_proxy_authentication_required.reason_phrase, "Proxy Authentication Required");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n408_request_timeout.status_code, &408);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n408_request_timeout.reason_phrase, "Request Timeout");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n409_conflict.status_code, &409);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n409_conflict.reason_phrase, "Conflict");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n410_gone.status_code, &410);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n410_gone.reason_phrase, "Gone");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n411_length_required.status_code, &411);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n411_length_required.reason_phrase, "Length Required");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n412_precondition_failed.status_code, &412);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n412_precondition_failed.reason_phrase, "Precondition Failed");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n413_payload_too_large.status_code, &413);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n413_payload_too_large.reason_phrase, "Payload Too Large");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n414_uri_too_long.status_code, &414);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n414_uri_too_long.reason_phrase, "URI Too Long");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n415_unsupported_media_type.status_code, &415);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n415_unsupported_media_type.reason_phrase, "Unsupported Media Type");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable.status_code, &416);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable.reason_phrase, "Range Not Satisfiable");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n417_expectation_failed.status_code, &417);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n417_expectation_failed.reason_phrase, "Expectation Failed");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n418_im_a_teapot.status_code, &418);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n418_im_a_teapot.reason_phrase, "I'm A Teapot");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n421_misdirected_request.status_code, &421);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n421_misdirected_request.reason_phrase, "Misdirected Request");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n422_unprocessable_entity.status_code, &422);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n422_unprocessable_entity.reason_phrase, "Unprocessable Entity");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n423_locked.status_code, &423);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n423_locked.reason_phrase, "Locked");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n424_failed_dependency.status_code, &424);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n424_failed_dependency.reason_phrase, "Failed Dependency");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n425_too_early.status_code, &425);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n425_too_early.reason_phrase, "Too Early");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n426_upgrade_required.status_code, &426);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n426_upgrade_required.reason_phrase, "Upgrade Required");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n428_precondition_required.status_code, &428);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n428_precondition_required.reason_phrase, "Precondition Required");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n429_too_many_requests.status_code, &429);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n429_too_many_requests.reason_phrase, "Too Many Requests");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n431_request_header_fields_too_large.status_code, &431);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n431_request_header_fields_too_large.reason_phrase, "Request Header Fields Too Large");

    assert_eq!(STATUS_CODE_REASON_PHRASE.n451_unavailable_for_legal_reasons.status_code, &451);
    assert_eq!(STATUS_CODE_REASON_PHRASE.n451_unavailable_for_legal_reasons.reason_phrase, "Unavailable For Legal Reasons");

    assert_eq!(STATUS_CODE_REASON_PHRASE, STATUS_CODE_REASON_PHRASE.clone());

}

#[test]
fn parse_no_boundary_at_the_end() {
    // in this example response is from reading a file
    let path = FileExt::build_path(&["src", "response", "response.multipart.no_boundary_at_the_end.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let response_raw_bytes : Vec<u8> = FileExt::read_file(absolute_file_path.as_str()).unwrap();

    let response_parse : Result<Response, String> = Response::parse(response_raw_bytes.as_ref());
    if response_parse.is_err() {
        let message = response_parse.clone().err().unwrap();
        assert_eq!("Unable to parse multipart form body, reached the end of stream and it does not contain boundary", message);
    }
}

#[test]
fn parse_no_boundary_at_the_beginning() {
    // in this example response is from reading a file
    let path = FileExt::build_path(&["src", "response", "response.multipart.no_boundary_at_the_beginning.txt"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let response_raw_bytes : Vec<u8> = FileExt::read_file(absolute_file_path.as_str()).unwrap();

    let response_parse : Result<Response, String> = Response::parse(response_raw_bytes.as_ref());
    if response_parse.is_err() {
        let message = response_parse.clone().err().unwrap();
        assert_eq!("Response body doesn't start with a boundary", message);
    }
}

