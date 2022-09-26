#[cfg(test)]
mod tests;

use std::io;
use std::io::{BufRead, Cursor, Read};
use crate::header::Header;
use regex::Regex;
use crate::http::VERSION;
use crate::range::{ContentRange, Range};
use crate::request::{METHOD, Request};
use crate::server::Server;
use crate::symbol::SYMBOL;

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Error {
    pub status_code_reason_phrase: &'static StatusCodeReasonPhrase,
    pub message: String,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Response {
    pub(crate) http_version: String,
    pub(crate) status_code: i16,
    pub(crate) reason_phrase: String,
    pub(crate) headers: Vec<Header>,
    pub(crate) content_range_list: Vec<ContentRange>
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct StatusCodeReasonPhrase {
    pub(crate) status_code: &'static i16,
    pub(crate) reason_phrase: &'static str,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ResponseStatusCodeReasonPhrase {
    pub(crate) n100_continue: &'static StatusCodeReasonPhrase,
    pub(crate) n101_switching_protocols: &'static StatusCodeReasonPhrase,
    pub(crate) n102_processing: &'static StatusCodeReasonPhrase,
    pub(crate) n103_early_hints: &'static StatusCodeReasonPhrase,
    pub(crate) n200_ok: &'static StatusCodeReasonPhrase,
    pub(crate) n201_created: &'static StatusCodeReasonPhrase,
    pub(crate) n202_accepted: &'static StatusCodeReasonPhrase,
    pub(crate) n203_non_authoritative_information: &'static StatusCodeReasonPhrase,
    pub(crate) n204_no_content: &'static StatusCodeReasonPhrase,
    pub(crate) n205_reset_content: &'static StatusCodeReasonPhrase,
    pub(crate) n206_partial_content: &'static StatusCodeReasonPhrase,
    pub(crate) n207_multi_status: &'static StatusCodeReasonPhrase,
    pub(crate) n208_already_reported: &'static StatusCodeReasonPhrase,
    pub(crate) n226_im_used: &'static StatusCodeReasonPhrase,
    pub(crate) n300_multiple_choices: &'static StatusCodeReasonPhrase,
    pub(crate) n301_moved_permanently: &'static StatusCodeReasonPhrase,
    pub(crate) n302_found: &'static StatusCodeReasonPhrase,
    pub(crate) n303_see_other: &'static StatusCodeReasonPhrase,
    pub(crate) n304_not_modified: &'static StatusCodeReasonPhrase,
    pub(crate) n307_temporary_redirect: &'static StatusCodeReasonPhrase,
    pub(crate) n308_permanent_redirect: &'static StatusCodeReasonPhrase,
    pub(crate) n400_bad_request: &'static StatusCodeReasonPhrase,
    pub(crate) n401_unauthorized: &'static StatusCodeReasonPhrase,
    pub(crate) n402_payment_required: &'static StatusCodeReasonPhrase,
    pub(crate) n403_forbidden: &'static StatusCodeReasonPhrase,
    pub(crate) n404_not_found: &'static StatusCodeReasonPhrase,
    pub(crate) n405_method_not_allowed: &'static StatusCodeReasonPhrase,
    pub(crate) n406_not_acceptable: &'static StatusCodeReasonPhrase,
    pub(crate) n407_proxy_authentication_required: &'static StatusCodeReasonPhrase,
    pub(crate) n408_request_timeout: &'static StatusCodeReasonPhrase,
    pub(crate) n409_conflict: &'static StatusCodeReasonPhrase,
    pub(crate) n410_gone: &'static StatusCodeReasonPhrase,
    pub(crate) n411_length_required: &'static StatusCodeReasonPhrase,
    pub(crate) n412_precondition_failed: &'static StatusCodeReasonPhrase,
    pub(crate) n413_payload_too_large: &'static StatusCodeReasonPhrase,
    pub(crate) n414_uri_too_long: &'static StatusCodeReasonPhrase,
    pub(crate) n415_unsupported_media_type: &'static StatusCodeReasonPhrase,
    pub(crate) n416_range_not_satisfiable: &'static StatusCodeReasonPhrase,
    pub(crate) n417_expectation_failed: &'static StatusCodeReasonPhrase,
    pub(crate) n418_im_a_teapot: &'static StatusCodeReasonPhrase,
    pub(crate) n421_misdirected_request: &'static StatusCodeReasonPhrase,
    pub(crate) n422_unprocessable_entity: &'static StatusCodeReasonPhrase,
    pub(crate) n423_locked: &'static StatusCodeReasonPhrase,
    pub(crate) n424_failed_dependency: &'static StatusCodeReasonPhrase,
    pub(crate) n425_too_early: &'static StatusCodeReasonPhrase,
    pub(crate) n426_upgrade_required: &'static StatusCodeReasonPhrase,
    pub(crate) n428_precondition_required: &'static StatusCodeReasonPhrase,
    pub(crate) n429_too_many_requests: &'static StatusCodeReasonPhrase,
    pub(crate) n431_request_header_fields_too_large: &'static StatusCodeReasonPhrase,
    pub(crate) n451_unavailable_for_legal_reasons: &'static StatusCodeReasonPhrase,
    pub(crate) n500_internal_server_error: &'static StatusCodeReasonPhrase,
    pub(crate) n501_not_implemented: &'static StatusCodeReasonPhrase,
    pub(crate) n502_bad_gateway: &'static StatusCodeReasonPhrase,
    pub(crate) n503_service_unavailable: &'static StatusCodeReasonPhrase,
    pub(crate) n504_gateway_timeout: &'static StatusCodeReasonPhrase,
    pub(crate) n505_http_version_not_supported: &'static StatusCodeReasonPhrase,
    pub(crate) n506_variant_also_negotiates: &'static StatusCodeReasonPhrase,
    pub(crate) n507_insufficient_storage: &'static StatusCodeReasonPhrase,
    pub(crate) n508_loop_detected: &'static StatusCodeReasonPhrase,
    pub(crate) n510_not_extended: &'static StatusCodeReasonPhrase,
    pub(crate) n511_network_authentication_required: &'static StatusCodeReasonPhrase,
}

pub const STATUS_CODE_REASON_PHRASE: ResponseStatusCodeReasonPhrase = ResponseStatusCodeReasonPhrase {
    n100_continue: &StatusCodeReasonPhrase { status_code: &100, reason_phrase: "Continue" },
    n101_switching_protocols: &StatusCodeReasonPhrase { status_code: &101, reason_phrase: "Switching Protocols" },
    n102_processing: &StatusCodeReasonPhrase { status_code: &102, reason_phrase: "Processing" },
    n103_early_hints: &StatusCodeReasonPhrase { status_code: &103, reason_phrase: "Early Hints" },
    n200_ok: &StatusCodeReasonPhrase {
        status_code: &200,
        reason_phrase: "OK"
    },

    n201_created: &StatusCodeReasonPhrase { status_code: &201, reason_phrase: "Created" },
    n202_accepted: &StatusCodeReasonPhrase { status_code: &202, reason_phrase: "Accepted" },
    n203_non_authoritative_information: &StatusCodeReasonPhrase { status_code: &203, reason_phrase: "Non Authoritative Information" },
    n204_no_content: &StatusCodeReasonPhrase {
        status_code: &204,
        reason_phrase: "No Content"
    },

    n205_reset_content: &StatusCodeReasonPhrase { status_code: &205, reason_phrase: "Reset Content" },
    n206_partial_content: &StatusCodeReasonPhrase {
        status_code: &206,
        reason_phrase: "Partial Content"
    },

    n207_multi_status: &StatusCodeReasonPhrase { status_code: &207, reason_phrase: "Multi-Status" },
    n208_already_reported: &StatusCodeReasonPhrase { status_code: &208, reason_phrase: "Already Reported" },
    n226_im_used: &StatusCodeReasonPhrase { status_code: &226, reason_phrase: "IM Used" },
    n300_multiple_choices: &StatusCodeReasonPhrase { status_code: &300, reason_phrase: "Multiple Choices" },
    n301_moved_permanently: &StatusCodeReasonPhrase { status_code: &301, reason_phrase: "Moved Permanently" },
    n302_found: &StatusCodeReasonPhrase { status_code: &302, reason_phrase: "Found" },
    n303_see_other: &StatusCodeReasonPhrase { status_code: &303, reason_phrase: "See Other" },
    n304_not_modified: &StatusCodeReasonPhrase { status_code: &304, reason_phrase: "Not Modified" },
    n307_temporary_redirect: &StatusCodeReasonPhrase { status_code: &307, reason_phrase: "Temporary Redirect" },
    n308_permanent_redirect: &StatusCodeReasonPhrase { status_code: &308, reason_phrase: "Permanent Redirect" },
    n400_bad_request: &StatusCodeReasonPhrase {
        status_code: &400,
        reason_phrase: "Bad Request"
    },

    n401_unauthorized: &StatusCodeReasonPhrase { status_code: &401, reason_phrase: "Unauthorized" },
    n402_payment_required: &StatusCodeReasonPhrase { status_code: &402, reason_phrase: "Payment Required" },
    n403_forbidden: &StatusCodeReasonPhrase { status_code: &403, reason_phrase: "Forbidden" },
    n404_not_found: &StatusCodeReasonPhrase {
        status_code: &404,
        reason_phrase: "Not Found"
    },

    n405_method_not_allowed: &StatusCodeReasonPhrase { status_code: &405, reason_phrase: "Method Not Allowed" },
    n406_not_acceptable: &StatusCodeReasonPhrase { status_code: &406, reason_phrase: "Not Acceptable" },
    n407_proxy_authentication_required: &StatusCodeReasonPhrase { status_code: &407, reason_phrase: "Proxy Authentication Required" },
    n408_request_timeout: &StatusCodeReasonPhrase { status_code: &408, reason_phrase: "Request Timeout" },
    n409_conflict: &StatusCodeReasonPhrase { status_code: &409, reason_phrase: "Conflict" },
    n410_gone: &StatusCodeReasonPhrase { status_code: &410, reason_phrase: "Gone" },
    n411_length_required: &StatusCodeReasonPhrase { status_code: &411, reason_phrase: "Length Required" },
    n412_precondition_failed: &StatusCodeReasonPhrase { status_code: &412, reason_phrase: "Precondition Failed" },
    n413_payload_too_large: &StatusCodeReasonPhrase { status_code: &413, reason_phrase: "Payload Too Large" },
    n414_uri_too_long: &StatusCodeReasonPhrase { status_code: &414, reason_phrase: "URI Too Long" },
    n415_unsupported_media_type: &StatusCodeReasonPhrase { status_code: &415, reason_phrase: "Unsupported Media Type" },
    n416_range_not_satisfiable: &StatusCodeReasonPhrase {
        status_code: &416,
        reason_phrase: "Range Not Satisfiable"
    },

    n417_expectation_failed: &StatusCodeReasonPhrase { status_code: &417, reason_phrase: "Expectation Failed" },
    n418_im_a_teapot: &StatusCodeReasonPhrase { status_code: &418, reason_phrase: "I'm A Teapot" },
    n421_misdirected_request: &StatusCodeReasonPhrase { status_code: &421, reason_phrase: "Misdirected Request" },
    n422_unprocessable_entity: &StatusCodeReasonPhrase { status_code: &422, reason_phrase: "Unprocessable Entity" },
    n423_locked: &StatusCodeReasonPhrase { status_code: &423, reason_phrase: "Locked" },
    n424_failed_dependency: &StatusCodeReasonPhrase { status_code: &424, reason_phrase: "Failed Dependency" },
    n425_too_early: &StatusCodeReasonPhrase { status_code: &425, reason_phrase: "Too Early" },
    n426_upgrade_required: &StatusCodeReasonPhrase { status_code: &426, reason_phrase: "Upgrade Required" },
    n428_precondition_required: &StatusCodeReasonPhrase { status_code: &428, reason_phrase: "Precondition Required" },
    n429_too_many_requests: &StatusCodeReasonPhrase { status_code: &429, reason_phrase: "Too Many Requests" },
    n431_request_header_fields_too_large: &StatusCodeReasonPhrase { status_code: &431, reason_phrase: "Request Header Fields Too Large" },
    n451_unavailable_for_legal_reasons: &StatusCodeReasonPhrase { status_code: &451, reason_phrase: "Unavailable For Legal Reasons" },
    n500_internal_server_error: &StatusCodeReasonPhrase {
        status_code: &500,
        reason_phrase: "Internal Server Error"
    },
    n501_not_implemented: &StatusCodeReasonPhrase { status_code: &501, reason_phrase: "Not Implemented" },
    n502_bad_gateway: &StatusCodeReasonPhrase { status_code: &502, reason_phrase: "Bad Gateway" },
    n503_service_unavailable: &StatusCodeReasonPhrase { status_code: &503, reason_phrase: "Service Unavailable" },
    n504_gateway_timeout: &StatusCodeReasonPhrase { status_code: &504, reason_phrase: "Gateway Timeout" },
    n505_http_version_not_supported: &StatusCodeReasonPhrase { status_code: &505, reason_phrase: "HTTP Version Not Supported" },
    n506_variant_also_negotiates: &StatusCodeReasonPhrase { status_code: &506, reason_phrase: "Variant Also Negotiates" },
    n507_insufficient_storage: &StatusCodeReasonPhrase { status_code: &507, reason_phrase: "Insufficient Storage" },
    n508_loop_detected: &StatusCodeReasonPhrase { status_code: &508, reason_phrase: "Loop Detected" },
    n510_not_extended: &StatusCodeReasonPhrase { status_code: &510, reason_phrase: "Not Extended" },
    n511_network_authentication_required: &StatusCodeReasonPhrase { status_code: &511, reason_phrase: "Network Authentication Required" }
};

impl Response {

    pub const _HTTP_VERSION_AND_STATUS_CODE_AND_REASON_PHRASE_REGEX: &'static str = "(?P<http_version>\\w+/\\w+.\\w)\\s(?P<status_code>\\w+)\\s(?P<reason_phrase>.+)";

    pub(crate) fn _get_header(&self, name: String) -> Option<&Header> {
        let header =  self.headers.iter().find(|x| x.name == name);
        header
    }

    pub(crate) fn generate_body(content_range_list: Vec<ContentRange>) -> Vec<u8> {
        let mut body = vec![];
        let one = 1;

        if content_range_list.len() == one {
            let index = 0;
            let content_range = content_range_list.get(index).unwrap();
            body = content_range.body.to_vec();
        }

        if content_range_list.len() > one {
            for (i, content_range) in content_range_list.iter().enumerate() {
                let mut body_str = SYMBOL.empty_string.to_string();
                if i != 0 {
                    body_str.push_str(SYMBOL.new_line_carriage_return);
                }
                body_str.push_str(SYMBOL.hyphen);
                body_str.push_str(SYMBOL.hyphen);
                body_str.push_str(Range::STRING_SEPARATOR);
                body_str.push_str(SYMBOL.new_line_carriage_return);
                let content_type = [Header::CONTENT_TYPE, Header::NAME_VALUE_SEPARATOR, SYMBOL.whitespace, &content_range.content_type.to_string()].join("");
                body_str.push_str(content_type.as_str());
                body_str.push_str(SYMBOL.new_line_carriage_return);
                let content_range_header = [Header::CONTENT_RANGE, Header::NAME_VALUE_SEPARATOR, SYMBOL.whitespace, Range::BYTES, SYMBOL.whitespace, &content_range.range.start.to_string(), SYMBOL.hyphen, &content_range.range.end.to_string(), SYMBOL.slash, &content_range.size].join("");
                body_str.push_str(content_range_header.as_str());
                body_str.push_str(SYMBOL.new_line_carriage_return);
                body_str.push_str(SYMBOL.new_line_carriage_return);

                let inner_body = [body_str.as_bytes(), &content_range.body].concat();
                body = [body, inner_body].concat();
            }
            let mut trailing_separator = SYMBOL.empty_string.to_string();
            trailing_separator.push_str(SYMBOL.new_line_carriage_return);
            trailing_separator.push_str(SYMBOL.hyphen);
            trailing_separator.push_str(SYMBOL.hyphen);
            trailing_separator.push_str(Range::STRING_SEPARATOR);
            body = [&body, trailing_separator.as_bytes()].concat();
        }

        body
    }

    pub(crate) fn generate_response(mut response: Response, request: Request) -> Vec<u8> {
        let mut headers = vec![
            Response::get_x_content_type_options_header(),
            Response::get_accept_ranges_header(),
        ];

        headers.append(&mut response.headers);

        if response.content_range_list.len() == 1 {
            let content_range_index = 0;
            let content_range = response.content_range_list.get(content_range_index).unwrap();
            headers.push(Header {
                name: Header::CONTENT_TYPE.to_string(),
                value: content_range.content_type.to_string()
            });

            let content_range_header_value = [
                Range::BYTES,
                SYMBOL.whitespace,
                &content_range.range.start.to_string(),
                SYMBOL.hyphen,
                &content_range.range.end.to_string(),
                SYMBOL.slash,
                &content_range.size
            ].join("");
            headers.push(Header {
                name: Header::CONTENT_RANGE.to_string(),
                value: content_range_header_value.to_string()
            });

            headers.push(Header {
                name: Header::CONTENT_LENGTH.to_string(),
                value: content_range.body.len().to_string()
            });
        }

        if response.content_range_list.len() > 1 {
            let content_range_header_value = [
                Range::MULTIPART,
                SYMBOL.slash,
                Range::BYTERANGES,
                SYMBOL.semicolon,
                SYMBOL.whitespace,
                Range::BOUNDARY,
                SYMBOL.equals,
                Range::STRING_SEPARATOR
            ].join("");
            headers.push(Header {
                name: Header::CONTENT_TYPE.to_string(),
                value: content_range_header_value,
            });
        }

        let body = Response::generate_body(response.content_range_list);

        let mut headers_str = SYMBOL.new_line_carriage_return.to_string();
        for header in headers {
            let mut header_string = SYMBOL.empty_string.to_string();
            header_string.push_str(&header.name);
            header_string.push_str(Header::NAME_VALUE_SEPARATOR);
            header_string.push_str(&header.value);
            header_string.push_str(SYMBOL.new_line_carriage_return);
            headers_str.push_str(&header_string);
        }
        let status = [response.http_version, response.status_code.to_string(), response.reason_phrase].join(SYMBOL.whitespace);
        let response_without_body = format!(
            "{}{}{}",
            status,
            headers_str,
            SYMBOL.new_line_carriage_return,
        );

        let is_head = request.method == METHOD.head;
        let is_options = request.method == METHOD.options;

        return if is_head || is_options {
            response_without_body.into_bytes()
        } else {
            [response_without_body.into_bytes(), body].concat()
        }

    }

    pub(crate) fn _parse_response(response_vec_u8: &[u8]) -> Response {
        let mut cursor = io::Cursor::new(response_vec_u8);

        let mut response = Response {
            http_version: "".to_string(),
            status_code: 0,
            reason_phrase: "".to_string(),
            headers: vec![],
            content_range_list: vec![],
        };

        let content_length: usize = 0;
        let iteration_number : usize = 0;

        Response::_parse_raw_response_via_cursor(&mut cursor, iteration_number, &mut response, content_length);

        return response;
    }

    pub(crate)  fn _parse_http_version_status_code_reason_phrase_string(http_version_status_code_reason_phrase: &str) -> Result<(String, i16, String), String> {
        let re = Regex::new(Response::_HTTP_VERSION_AND_STATUS_CODE_AND_REASON_PHRASE_REGEX).unwrap();
        let caps = re.captures(&http_version_status_code_reason_phrase).unwrap();

        let http_version= String::from(&caps["http_version"]);
        let status_code = String::from(&caps["status_code"]);
        let boxed_status_code_i16 = status_code.parse::<i16>();
        if boxed_status_code_i16.is_err() {
            let message = [
                "unable to parse status code: ",
                boxed_status_code_i16.err().unwrap().to_string().as_str()
            ].join("");
            return Err(message)
        }
        let status_code_i16 : i16 = boxed_status_code_i16.unwrap();
        let mut reason_phrase = String::from(&caps["reason_phrase"]);
        reason_phrase = Server::truncate_new_line_carriage_return(&reason_phrase);

        return Ok((http_version, status_code_i16, reason_phrase))
    }

    pub(crate)  fn _parse_http_response_header_string(header_string: &str) -> Header {
        let header_parts: Vec<&str> = header_string.split(Header::NAME_VALUE_SEPARATOR).collect();
        let header_name = header_parts[0].to_string();
        let raw_header_value = header_parts[1].to_string();
        let header_value = Server::truncate_new_line_carriage_return(&raw_header_value);


        Header {
            name: header_name.to_string(),
            value: header_value.to_string()
        }
    }

    pub(crate) fn _parse_raw_response_via_cursor(
        cursor: &mut Cursor<&[u8]>,
        mut iteration_number: usize,
        response: &mut Response,
        mut content_length: usize) {

        let mut buffer = vec![];
        let bytes_offset = cursor.read_until(b'\n', &mut buffer).unwrap();
        let mut buffer_as_u8_array: &[u8] = &buffer;
        let string = String::from_utf8(Vec::from(buffer_as_u8_array)).unwrap();

        let is_first_iteration = iteration_number == 0;
        let new_line_char_found = bytes_offset != 0;
        let current_string_is_empty = string.trim().len() == 0;

        if is_first_iteration {
            let boxed_http_version_status_code_reason_phrase = Response::_parse_http_version_status_code_reason_phrase_string(&string);
            if boxed_http_version_status_code_reason_phrase.is_err() {
                let error = boxed_http_version_status_code_reason_phrase.err().unwrap();
                eprintln!("{}", error);
                return;
            }

            let (http_version, status_code, reason_phrase) = boxed_http_version_status_code_reason_phrase.unwrap();

            response.http_version = http_version;
            response.status_code = status_code;
            response.reason_phrase = reason_phrase;
        }

        if current_string_is_empty {
            let content_type = response._get_header(Header::CONTENT_TYPE.to_string()).unwrap();
            let is_multipart = Response::_is_multipart_byteranges_content_type(&content_type);

            if is_multipart {
                let content_range_list : Vec<ContentRange> = vec![];

                let mut buf = vec![];
                cursor.read_until(b'\n', &mut buf).unwrap();
                let boxed_value = Range::_parse_multipart_body(cursor, content_range_list);
                let mut range_list = vec![];
                if boxed_value.is_ok() {
                    range_list = boxed_value.unwrap();
                }
                response.content_range_list = range_list;
            } else {
                buffer = vec![];
                let boxed_read = cursor.read_to_end(&mut buffer);
                if boxed_read.is_ok() {
                    buffer_as_u8_array = &buffer;

                    let content_range = ContentRange {
                        unit: Range::BYTES.to_string(),
                        range: Range {
                            start: 0,
                            end: buffer_as_u8_array.len() as u64
                        },
                        size: buffer_as_u8_array.len().to_string(),
                        body: Vec::from(buffer_as_u8_array),
                        content_type: content_type.value.to_string()
                    };
                    response.content_range_list = vec![content_range];
                } else {
                    let reason = boxed_read.err().unwrap();
                    eprintln!("error reading file: {}", reason.to_string())
                }

            }

            return;
        }

        if new_line_char_found && !current_string_is_empty {
            let mut header = Header { name: "".to_string(), value: "".to_string() };
            if !is_first_iteration {
                header = Response::_parse_http_response_header_string(&string);
                if header.name == Header::CONTENT_LENGTH {
                    content_length = header.value.parse().unwrap();
                }
            }

            response.headers.push(header);
            iteration_number += 1;
            Response::_parse_raw_response_via_cursor(cursor, iteration_number, response, content_length);
        }
    }

    pub(crate) fn _is_multipart_byteranges_content_type(content_type: &Header) -> bool {
        let multipart_byteranges =
            [
                Range::MULTIPART,
                SYMBOL.slash,
                Range::BYTERANGES
            ].join("");
        let is_multipart_byteranges = content_type.value.starts_with(&multipart_byteranges);
        is_multipart_byteranges
    }

    pub(crate) fn get_x_content_type_options_header() -> Header {
        Header {
            name: Header::X_CONTENT_TYPE_OPTIONS.to_string(),
            value: Header::X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF.to_string(),
        }
    }

    pub(crate) fn get_accept_ranges_header() -> Header {
        Header {
            name: Header::ACCEPT_RANGES.to_string(),
            value: Range::BYTES.to_string(),
        }
    }

    pub fn get_response(
        status_code_reason_phrase: &StatusCodeReasonPhrase,
        boxed_header_list: Option<Vec<Header>>,
        boxed_content_range_list: Option<Vec<ContentRange>>) -> Result<Response, String> {

        let mut header_list: Vec<Header> = vec![];
        if boxed_header_list.is_some() {
            header_list = boxed_header_list.unwrap();
        }

        let mut content_range_list: Vec<ContentRange> = vec![];
        if boxed_content_range_list.is_some() {
            content_range_list = boxed_content_range_list.unwrap();
        }

        let response = Response {
            http_version: VERSION.http_1_1.to_string(),
            status_code: *status_code_reason_phrase.status_code,
            reason_phrase: status_code_reason_phrase.reason_phrase.to_string(),
            headers: header_list,
            content_range_list
        };

        Ok(response)
    }
}