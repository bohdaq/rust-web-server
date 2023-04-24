#[cfg(test)]
mod tests;
#[cfg(test)]
mod example;

use std::io;
use std::io::{BufRead, Cursor, Read};
use crate::header::Header;
use crate::ext::string_ext::StringExt;
use crate::http::{HTTP, VERSION};
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};
use crate::request::{METHOD, Request};
use crate::symbol::SYMBOL;

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Error {
    pub status_code_reason_phrase: &'static StatusCodeReasonPhrase,
    pub message: String,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Response {
    pub http_version: String,
    pub status_code: i16,
    pub reason_phrase: String,
    pub headers: Vec<Header>,
    pub content_range_list: Vec<ContentRange>
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct StatusCodeReasonPhrase {
    pub status_code: &'static i16,
    pub reason_phrase: &'static str,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ResponseStatusCodeReasonPhrase {
    pub n100_continue: &'static StatusCodeReasonPhrase,
    pub n101_switching_protocols: &'static StatusCodeReasonPhrase,
    pub n102_processing: &'static StatusCodeReasonPhrase,
    pub n103_early_hints: &'static StatusCodeReasonPhrase,
    pub n200_ok: &'static StatusCodeReasonPhrase,
    pub n201_created: &'static StatusCodeReasonPhrase,
    pub n202_accepted: &'static StatusCodeReasonPhrase,
    pub n203_non_authoritative_information: &'static StatusCodeReasonPhrase,
    pub n204_no_content: &'static StatusCodeReasonPhrase,
    pub n205_reset_content: &'static StatusCodeReasonPhrase,
    pub n206_partial_content: &'static StatusCodeReasonPhrase,
    pub n207_multi_status: &'static StatusCodeReasonPhrase,
    pub n208_already_reported: &'static StatusCodeReasonPhrase,
    pub n226_im_used: &'static StatusCodeReasonPhrase,
    pub n300_multiple_choices: &'static StatusCodeReasonPhrase,
    pub n301_moved_permanently: &'static StatusCodeReasonPhrase,
    pub n302_found: &'static StatusCodeReasonPhrase,
    pub n303_see_other: &'static StatusCodeReasonPhrase,
    pub n304_not_modified: &'static StatusCodeReasonPhrase,
    pub n307_temporary_redirect: &'static StatusCodeReasonPhrase,
    pub n308_permanent_redirect: &'static StatusCodeReasonPhrase,
    pub n400_bad_request: &'static StatusCodeReasonPhrase,
    pub n401_unauthorized: &'static StatusCodeReasonPhrase,
    pub n402_payment_required: &'static StatusCodeReasonPhrase,
    pub n403_forbidden: &'static StatusCodeReasonPhrase,
    pub n404_not_found: &'static StatusCodeReasonPhrase,
    pub n405_method_not_allowed: &'static StatusCodeReasonPhrase,
    pub n406_not_acceptable: &'static StatusCodeReasonPhrase,
    pub n407_proxy_authentication_required: &'static StatusCodeReasonPhrase,
    pub n408_request_timeout: &'static StatusCodeReasonPhrase,
    pub n409_conflict: &'static StatusCodeReasonPhrase,
    pub n410_gone: &'static StatusCodeReasonPhrase,
    pub n411_length_required: &'static StatusCodeReasonPhrase,
    pub n412_precondition_failed: &'static StatusCodeReasonPhrase,
    pub n413_payload_too_large: &'static StatusCodeReasonPhrase,
    pub n414_uri_too_long: &'static StatusCodeReasonPhrase,
    pub n415_unsupported_media_type: &'static StatusCodeReasonPhrase,
    pub n416_range_not_satisfiable: &'static StatusCodeReasonPhrase,
    pub n417_expectation_failed: &'static StatusCodeReasonPhrase,
    pub n418_im_a_teapot: &'static StatusCodeReasonPhrase,
    pub n421_misdirected_request: &'static StatusCodeReasonPhrase,
    pub n422_unprocessable_entity: &'static StatusCodeReasonPhrase,
    pub n423_locked: &'static StatusCodeReasonPhrase,
    pub n424_failed_dependency: &'static StatusCodeReasonPhrase,
    pub n425_too_early: &'static StatusCodeReasonPhrase,
    pub n426_upgrade_required: &'static StatusCodeReasonPhrase,
    pub n428_precondition_required: &'static StatusCodeReasonPhrase,
    pub n429_too_many_requests: &'static StatusCodeReasonPhrase,
    pub n431_request_header_fields_too_large: &'static StatusCodeReasonPhrase,
    pub n451_unavailable_for_legal_reasons: &'static StatusCodeReasonPhrase,
    pub n500_internal_server_error: &'static StatusCodeReasonPhrase,
    pub n501_not_implemented: &'static StatusCodeReasonPhrase,
    pub n502_bad_gateway: &'static StatusCodeReasonPhrase,
    pub n503_service_unavailable: &'static StatusCodeReasonPhrase,
    pub n504_gateway_timeout: &'static StatusCodeReasonPhrase,
    pub n505_http_version_not_supported: &'static StatusCodeReasonPhrase,
    pub n506_variant_also_negotiates: &'static StatusCodeReasonPhrase,
    pub n507_insufficient_storage: &'static StatusCodeReasonPhrase,
    pub n508_loop_detected: &'static StatusCodeReasonPhrase,
    pub n510_not_extended: &'static StatusCodeReasonPhrase,
    pub n511_network_authentication_required: &'static StatusCodeReasonPhrase,
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

    pub fn status_code_reason_phrase_list() -> Vec<&'static StatusCodeReasonPhrase> {
        let list = vec![
            STATUS_CODE_REASON_PHRASE.n100_continue,
            STATUS_CODE_REASON_PHRASE.n101_switching_protocols,
            STATUS_CODE_REASON_PHRASE.n102_processing,
            STATUS_CODE_REASON_PHRASE.n103_early_hints,
            STATUS_CODE_REASON_PHRASE.n200_ok,
            STATUS_CODE_REASON_PHRASE.n201_created,
            STATUS_CODE_REASON_PHRASE.n202_accepted,
            STATUS_CODE_REASON_PHRASE.n203_non_authoritative_information,
            STATUS_CODE_REASON_PHRASE.n204_no_content,
            STATUS_CODE_REASON_PHRASE.n205_reset_content,
            STATUS_CODE_REASON_PHRASE.n206_partial_content,
            STATUS_CODE_REASON_PHRASE.n207_multi_status,
            STATUS_CODE_REASON_PHRASE.n208_already_reported,
            STATUS_CODE_REASON_PHRASE.n226_im_used,
            STATUS_CODE_REASON_PHRASE.n300_multiple_choices,
            STATUS_CODE_REASON_PHRASE.n301_moved_permanently,
            STATUS_CODE_REASON_PHRASE.n302_found,
            STATUS_CODE_REASON_PHRASE.n303_see_other,
            STATUS_CODE_REASON_PHRASE.n304_not_modified,
            STATUS_CODE_REASON_PHRASE.n307_temporary_redirect,
            STATUS_CODE_REASON_PHRASE.n308_permanent_redirect,
            STATUS_CODE_REASON_PHRASE.n400_bad_request,
            STATUS_CODE_REASON_PHRASE.n401_unauthorized,
            STATUS_CODE_REASON_PHRASE.n402_payment_required,
            STATUS_CODE_REASON_PHRASE.n403_forbidden,
            STATUS_CODE_REASON_PHRASE.n404_not_found,
            STATUS_CODE_REASON_PHRASE.n405_method_not_allowed,
            STATUS_CODE_REASON_PHRASE.n406_not_acceptable,
            STATUS_CODE_REASON_PHRASE.n407_proxy_authentication_required,
            STATUS_CODE_REASON_PHRASE.n408_request_timeout,
            STATUS_CODE_REASON_PHRASE.n409_conflict,
            STATUS_CODE_REASON_PHRASE.n410_gone,
            STATUS_CODE_REASON_PHRASE.n411_length_required,
            STATUS_CODE_REASON_PHRASE.n412_precondition_failed,
            STATUS_CODE_REASON_PHRASE.n413_payload_too_large,
            STATUS_CODE_REASON_PHRASE.n414_uri_too_long,
            STATUS_CODE_REASON_PHRASE.n415_unsupported_media_type,
            STATUS_CODE_REASON_PHRASE.n416_range_not_satisfiable,
            STATUS_CODE_REASON_PHRASE.n417_expectation_failed,
            STATUS_CODE_REASON_PHRASE.n418_im_a_teapot,
            STATUS_CODE_REASON_PHRASE.n421_misdirected_request,
            STATUS_CODE_REASON_PHRASE.n422_unprocessable_entity,
            STATUS_CODE_REASON_PHRASE.n423_locked,
            STATUS_CODE_REASON_PHRASE.n424_failed_dependency,
            STATUS_CODE_REASON_PHRASE.n425_too_early,
            STATUS_CODE_REASON_PHRASE.n426_upgrade_required,
            STATUS_CODE_REASON_PHRASE.n428_precondition_required,
            STATUS_CODE_REASON_PHRASE.n429_too_many_requests,
            STATUS_CODE_REASON_PHRASE.n431_request_header_fields_too_large,
            STATUS_CODE_REASON_PHRASE.n451_unavailable_for_legal_reasons,
            STATUS_CODE_REASON_PHRASE.n500_internal_server_error,
            STATUS_CODE_REASON_PHRASE.n501_not_implemented,
            STATUS_CODE_REASON_PHRASE.n502_bad_gateway,
            STATUS_CODE_REASON_PHRASE.n503_service_unavailable,
            STATUS_CODE_REASON_PHRASE.n504_gateway_timeout,
            STATUS_CODE_REASON_PHRASE.n505_http_version_not_supported,
            STATUS_CODE_REASON_PHRASE.n506_variant_also_negotiates,
            STATUS_CODE_REASON_PHRASE.n507_insufficient_storage,
            STATUS_CODE_REASON_PHRASE.n508_loop_detected,
            STATUS_CODE_REASON_PHRASE.n510_not_extended,
            STATUS_CODE_REASON_PHRASE.n511_network_authentication_required,
        ];
        list
    }

    pub const _ERROR_UNABLE_TO_PARSE_HTTP_VERSION_STATUS_CODE: &'static str = "Unable to parse status code";

    pub const _HTTP_VERSION_AND_STATUS_CODE_AND_REASON_PHRASE_REGEX: &'static str = "(?P<http_version>\\w+/\\w+.\\w)\\s(?P<status_code>\\w+)\\s(?P<reason_phrase>.+)";

    pub fn _get_header(&self, name: String) -> Option<&Header> {
        let header =  self.headers.iter().find(|x| x.name == name);
        header
    }

    pub fn generate_body(content_range_list: Vec<ContentRange>) -> Vec<u8> {
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
                let content_type = [Header::_CONTENT_TYPE, Header::NAME_VALUE_SEPARATOR, SYMBOL.whitespace, &content_range.content_type.to_string()].join("");
                body_str.push_str(content_type.as_str());
                body_str.push_str(SYMBOL.new_line_carriage_return);
                let content_range_header = [Header::_CONTENT_RANGE, Header::NAME_VALUE_SEPARATOR, SYMBOL.whitespace, Range::BYTES, SYMBOL.whitespace, &content_range.range.start.to_string(), SYMBOL.hyphen, &content_range.range.end.to_string(), SYMBOL.slash, &content_range.size].join("");
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

    pub fn generate_response(mut response: Response, request: Request) -> Vec<u8> {

        if response.content_range_list.len() == 1 {
            let content_range_index = 0;
            let content_range = response.content_range_list.get(content_range_index).unwrap();
            response.headers.push(Header {
                name: Header::_CONTENT_TYPE.to_string(),
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
            response.headers.push(Header {
                name: Header::_CONTENT_RANGE.to_string(),
                value: content_range_header_value.to_string()
            });

            response.headers.push(Header {
                name: Header::_CONTENT_LENGTH.to_string(),
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
            response.headers.push(Header {
                name: Header::_CONTENT_TYPE.to_string(),
                value: content_range_header_value,
            });
        }

        let body = Response::generate_body(response.content_range_list);

        let mut headers_str = SYMBOL.new_line_carriage_return.to_string();
        for header in response.headers {
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

    pub fn _parse_response(response_vec_u8: &[u8]) -> Response {
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

    pub fn _parse_http_version_status_code_reason_phrase_string(http_version_status_code_reason_phrase: &str) -> Result<(String, i16, String), String> {
        let truncated = StringExt::truncate_new_line_carriage_return(http_version_status_code_reason_phrase);

        let boxed_split = truncated.split_once(SYMBOL.whitespace);
        if boxed_split.is_none() {
            return Err(Response::_ERROR_UNABLE_TO_PARSE_HTTP_VERSION_STATUS_CODE.to_string())
        }

        let (http_version, status_code_reason_phrase) = boxed_split.unwrap();
        let supported_http_versions = HTTP::version_list();
        if !supported_http_versions.contains(&http_version.to_uppercase().to_string()) {
            return Err(Response::_ERROR_UNABLE_TO_PARSE_HTTP_VERSION_STATUS_CODE.to_string())
        }

        let boxed_split = status_code_reason_phrase.split_once(SYMBOL.whitespace);
        if boxed_split.is_none() {
            return Err(Response::_ERROR_UNABLE_TO_PARSE_HTTP_VERSION_STATUS_CODE.to_string())
        }
        let (status_code, reason_phrase) = boxed_split.unwrap();

        let boxed_status_code_i16 = status_code.parse::<i16>();
        if boxed_status_code_i16.is_err() {
            return Err(Response::_ERROR_UNABLE_TO_PARSE_HTTP_VERSION_STATUS_CODE.to_string())
        }

        let status_code_i16 = boxed_status_code_i16.unwrap();

        let list = Response::status_code_reason_phrase_list();
        let boxed_search =
            list
                .iter()
                .find(|x| {
                    return x.status_code == &status_code_i16
                });

        if boxed_search.is_none() {
            return Err(Response::_ERROR_UNABLE_TO_PARSE_HTTP_VERSION_STATUS_CODE.to_string())
        }

        let found_status_code_reason_phrase = boxed_search.unwrap();
        let uppercase_reason_phrase = reason_phrase.to_uppercase();
        let is_equal =
            &found_status_code_reason_phrase.reason_phrase
            .to_uppercase()
                .eq(uppercase_reason_phrase.as_str());

        if !is_equal {
            return Err(Response::_ERROR_UNABLE_TO_PARSE_HTTP_VERSION_STATUS_CODE.to_string())
        }

        return Ok((http_version.to_string(), status_code_i16, reason_phrase.to_string()))
    }

    pub fn _parse_http_response_header_string(header_string: &str) -> Header {
        let header_parts: Vec<&str> = header_string.split(Header::NAME_VALUE_SEPARATOR).collect();
        let header_name = header_parts[0].to_string();
        let raw_header_value = header_parts[1].to_string();
        let header_value = StringExt::truncate_new_line_carriage_return(&raw_header_value);


        Header {
            name: header_name.to_string(),
            value: header_value.to_string()
        }
    }

    pub fn _parse_raw_response_via_cursor(
        cursor: &mut Cursor<&[u8]>,
        mut iteration_number: usize,
        response: &mut Response,
        mut content_length: usize) {

        let mut buffer = vec![];
        let boxed_read = cursor.read_until(b'\n', &mut buffer);
        if boxed_read.is_err() {
            eprintln!("unable to parse raw response via cursor {}", boxed_read.err().unwrap());
            return;
        }
        let bytes_offset = boxed_read.unwrap();
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
            let content_type = response._get_header(Header::_CONTENT_TYPE.to_string()).unwrap();
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
                if header.name == Header::_CONTENT_LENGTH {
                    content_length = header.value.parse().unwrap();
                }
            }

            response.headers.push(header);
            iteration_number += 1;
            Response::_parse_raw_response_via_cursor(cursor, iteration_number, response, content_length);
        }
    }

    pub fn _is_multipart_byteranges_content_type(content_type: &Header) -> bool {
        let multipart_byteranges =
            [
                Range::MULTIPART,
                SYMBOL.slash,
                Range::BYTERANGES
            ].join("");
        let is_multipart_byteranges = content_type.value.starts_with(&multipart_byteranges);
        is_multipart_byteranges
    }


    pub fn get_response(
        status_code_reason_phrase: &StatusCodeReasonPhrase,
        boxed_header_list: Option<Vec<Header>>,
        boxed_content_range_list: Option<Vec<ContentRange>>) -> Response {

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

        response
    }

    pub fn generate(&mut self) -> Vec<u8> {
        let response = &mut self.clone();

        if response.content_range_list.len() == 1 {
            let content_range_index = 0;
            let content_range = response.content_range_list.get(content_range_index).unwrap();
            self.headers.push(Header {
                name: Header::_CONTENT_TYPE.to_string(),
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
            response.headers.push(Header {
                name: Header::_CONTENT_RANGE.to_string(),
                value: content_range_header_value.to_string()
            });

            response.headers.push(Header {
                name: Header::_CONTENT_LENGTH.to_string(),
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
            response.headers.push(Header {
                name: Header::_CONTENT_TYPE.to_string(),
                value: content_range_header_value,
            });
        }

        let response_clone = response.clone();
        let body = Response::generate_body(response_clone.content_range_list);

        let mut headers_str = SYMBOL.new_line_carriage_return.to_string();

        let header_list = response.headers.clone();
        for header in header_list {
            let mut header_string = SYMBOL.empty_string.to_string();
            header_string.push_str(&header.name);
            header_string.push_str(Header::NAME_VALUE_SEPARATOR);
            header_string.push_str(&header.value);
            header_string.push_str(SYMBOL.new_line_carriage_return);
            headers_str.push_str(&header_string);
        }

        let response_clone = response.clone();
        let status = [response_clone.http_version, response.status_code.to_string(), response_clone.reason_phrase].join(SYMBOL.whitespace);
        let response_without_body = format!(
            "{}{}{}",
            status,
            headers_str,
            SYMBOL.new_line_carriage_return,
        );


        [response_without_body.into_bytes(), body].concat()
    }

    pub fn get_header(&self, name: String) -> Option<&Header> {
        self._get_header(name)
    }

    pub fn parse(response_vec_u8: &[u8]) -> Result<Response, String> {
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

        let boxed_parse = Response::parse_raw_response_via_cursor(&mut cursor, iteration_number, &mut response, content_length);
        if boxed_parse.is_err() {
            let message = boxed_parse.err().unwrap();
            return Err(message);
        }

        return Ok(response);
    }

    pub fn parse_raw_response_via_cursor(
        cursor: &mut Cursor<&[u8]>,
        mut iteration_number: usize,
        response: &mut Response,
        mut content_length: usize) -> Result<(), String> {

        let mut buffer = vec![];
        let boxed_read = cursor.read_until(b'\n', &mut buffer);
        if boxed_read.is_err() {
            let message = format!("unable to parse raw response via cursor {}", boxed_read.err().unwrap());
            return Err(message);
        }
        let bytes_offset = boxed_read.unwrap();
        let mut buffer_as_u8_array: &[u8] = &buffer;
        let boxed_string = String::from_utf8(Vec::from(buffer_as_u8_array));
        if boxed_string.is_err() {
            let message = boxed_string.err().unwrap().to_string();
            return Err(message);
        }
        let string = boxed_string.unwrap();

        let is_first_iteration = iteration_number == 0;
        let new_line_char_found = bytes_offset != 0;
        let current_string_is_empty = string.trim().len() == 0;

        if is_first_iteration {
            let boxed_http_version_status_code_reason_phrase = Response::_parse_http_version_status_code_reason_phrase_string(&string);
            if boxed_http_version_status_code_reason_phrase.is_err() {
                let message = boxed_http_version_status_code_reason_phrase.err().unwrap();
                return Err(message);
            }

            let (http_version, status_code, reason_phrase) = boxed_http_version_status_code_reason_phrase.unwrap();

            response.http_version = http_version;
            response.status_code = status_code;
            response.reason_phrase = reason_phrase;
        }

        if current_string_is_empty {
            let mut is_multipart = false;
            // if response does not contain Content-Type, it will be defaulted to APPLICATION_OCTET_STREAM
            let mut content_type = MimeType::APPLICATION_OCTET_STREAM;

            let boxed_content_type = response.get_header(Header::_CONTENT_TYPE.to_string());
            if boxed_content_type.is_some() {
                let content_type_header = response.get_header(Header::_CONTENT_TYPE.to_string()).unwrap();
                content_type = content_type_header.value.as_str();
                is_multipart = Response::_is_multipart_byteranges_content_type(&content_type_header);
            }


            if is_multipart {
                let content_range_list : Vec<ContentRange> = vec![];

                let mut buf = vec![];
                let boxed_read = cursor.read_until(b'\n', &mut buf);
                if boxed_read.is_err() {
                    let message = boxed_read.err().unwrap().to_string();
                    return Err(message);
                }
                boxed_read.unwrap();
                let boxed_content_range_list = Range::parse_multipart_body(cursor, content_range_list);
                if boxed_content_range_list.is_err() {
                    let message = boxed_content_range_list.err().unwrap();
                    return Err(message);
                }

                response.content_range_list = boxed_content_range_list.unwrap();
            } else {
                buffer = vec![];
                let boxed_read = cursor.read_to_end(&mut buffer);
                if boxed_read.is_err() {
                    let message = boxed_read.err().unwrap().to_string();
                    return Err(message);
                }

                buffer_as_u8_array = &buffer;

                let content_range = ContentRange {
                    unit: Range::BYTES.to_string(),
                    range: Range {
                        start: 0,
                        end: buffer_as_u8_array.len() as u64
                    },
                    size: buffer_as_u8_array.len().to_string(),
                    body: Vec::from(buffer_as_u8_array),
                    content_type: content_type.to_string()
                };
                response.content_range_list = vec![content_range];


            }

            return Ok(());
        }

        if new_line_char_found && !current_string_is_empty {
            if !is_first_iteration {
                let boxed_header = Response::parse_http_response_header_string(&string);
                if boxed_header.is_err() {
                    let message = boxed_header.err().unwrap();
                    return Err(message);
                }
                let header = boxed_header.unwrap();
                if header.name == Header::_CONTENT_LENGTH {
                    content_length = header.value.parse().unwrap();
                }
                response.headers.push(header);
            }

            iteration_number += 1;
            return Response::parse_raw_response_via_cursor(cursor, iteration_number, response, content_length);
        } else {
            return Err("unable to parse".to_string());
        }
    }

    pub fn parse_http_response_header_string(header_string: &str) -> Result<Header, String> {
        let header_parts: Option<(&str, &str)> = header_string.split_once(Header::NAME_VALUE_SEPARATOR);
        if header_parts.is_none() {
            let message = format!("unable to parse header: {}", header_string);
            return Err(message);
        }
        let (header_name, raw_header_value) = header_parts.unwrap();
        let header_value = StringExt::truncate_new_line_carriage_return(&raw_header_value);

        Ok(Header {
            name: header_name.to_string(),
            value: header_value.to_string()
        })
    }
}