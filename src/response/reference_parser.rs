//! The upstream-response head parser as it was before the single-pass
//! rewrite, kept verbatim as the reference for the differential test in
//! `tests.rs`.
#![allow(unused_assignments, unused_mut, unused_variables, clippy::all)]
use std::io::{BufRead, Cursor, Read};
use crate::body::multipart_form_data::FormMultipartData;
use crate::ext::string_ext::StringExt;
use crate::header::Header;
use crate::http::HTTP;
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};
use crate::response::Response;
use crate::symbol::SYMBOL;

pub(super) fn parse(response_vec_u8: &[u8]) -> Result<Response, String> {
    let total_bytes = response_vec_u8.len() as i32;
    let mut cursor = Cursor::new(response_vec_u8);
    let mut response = Response::empty();
    via_cursor(&mut cursor, 0, &mut response, 0, total_bytes, 0)?;
    Ok(response)
}

pub(super) fn status_line(http_version_status_code_reason_phrase: &str) -> Result<(String, i16, String), String> {
    let truncated = StringExt::truncate_new_line_carriage_return(http_version_status_code_reason_phrase);

    let boxed_split = truncated.split_once(SYMBOL.whitespace);
    if boxed_split.is_none() {
        return Err(Response::_ERROR_UNABLE_TO_PARSE_HTTP_VERSION_STATUS_CODE.to_string())
    }

    let (http_version, status_code_reason_phrase) = boxed_split.unwrap();
    if !HTTP::is_supported_version(http_version) {
        return Err(Response::_ERROR_UNABLE_TO_PARSE_HTTP_VERSION_STATUS_CODE.to_string())
    }

    // RFC 9112 §4: status-line = HTTP-version SP status-code SP [ reason-phrase ].
    // The reason phrase is optional free text that clients should ignore
    // (Python's http.server sends "404 File not found"), and any 3-digit
    // code is valid — so neither is checked against the known list.
    let (status_code, reason_phrase) = status_code_reason_phrase
        .split_once(SYMBOL.whitespace)
        .unwrap_or((status_code_reason_phrase, ""));

    let is_three_digits = status_code.len() == 3 && status_code.bytes().all(|b| b.is_ascii_digit());
    let status_code_i16 = match status_code.parse::<i16>() {
        Ok(code) if is_three_digits && code >= 100 => code,
        _ => return Err(Response::_ERROR_UNABLE_TO_PARSE_HTTP_VERSION_STATUS_CODE.to_string()),
    };

    return Ok((http_version.to_string(), status_code_i16, reason_phrase.to_string()))
}

pub(super) fn via_cursor(
    cursor: &mut Cursor<&[u8]>,
    mut iteration_number: usize,
    response: &mut Response,
    _content_length: usize,
    total_bytes: i32,
    mut bytes_read: i32) -> Result<(), String> {
    // A loop, not tail recursion (which Rust doesn't guarantee to
    // eliminate): one stack frame per header line let a backend sending
    // thousands of header lines overflow the stack and abort the process.
    loop {

        let mut buffer = vec![];
        let boxed_read = cursor.read_until(b'\n', &mut buffer);
        if boxed_read.is_err() {
            let message = format!("unable to parse raw response via cursor {}", boxed_read.err().unwrap());
            return Err(message);
        }
        let bytes_offset = boxed_read.unwrap();
        bytes_read = bytes_read + bytes_offset as i32;
        if bytes_read == total_bytes {
            // end of stream
        }
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
            let boxed_http_version_status_code_reason_phrase = status_line(&string);
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

            let boxed_content_type = response.header(Header::_CONTENT_TYPE);
            if boxed_content_type.is_some() {
                let content_type_header = response.header(Header::_CONTENT_TYPE).unwrap();
                content_type = content_type_header.value.as_str();
                is_multipart = Response::_is_multipart_byteranges_content_type(&content_type_header);
            }


            if is_multipart {
                let content_range_list : Vec<ContentRange> = vec![];
                let boxed_content_type = response.header(Header::_CONTENT_TYPE);
                if boxed_content_type.is_none() {
                    return Err("Content-Type is missing".to_string());
                }
                let content_type = boxed_content_type.unwrap();
                let boxed_boundary = FormMultipartData::extract_boundary(content_type.value.as_str());
                if boxed_boundary.is_err() {
                    return Err("unable to extract boundary from Content-Type".to_string());
                }
                let boundary = boxed_boundary.unwrap();

                let is_opening_boundary_read = false;
                let boxed_content_range_list =
                    Range::parse_multipart_body_with_boundary(
                        cursor,
                        content_range_list,
                        boundary,
                        total_bytes,
                        bytes_read,
                        is_opening_boundary_read);
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
                let bytes_offset = boxed_read.unwrap();
                bytes_read = bytes_read + bytes_offset as i32;
                if bytes_read == total_bytes {
                    // end of stream
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
                let boxed_header = header_line(&string);
                if boxed_header.is_err() {
                    let message = boxed_header.err().unwrap();
                    return Err(message);
                }
                let header = boxed_header.unwrap();
                if header.name.eq_ignore_ascii_case(Header::_CONTENT_LENGTH) {
                    // An upstream-supplied value: reject garbage instead of panicking.
                    let _: usize = header.value.trim().parse()
                        .map_err(|_| format!("invalid Content-Length: {:?}", header.value))?;
                }
                response.headers.push(header);
            }

        iteration_number += 1;
    } else {
        return Err("unable to parse".to_string());
    }
    }
}

pub(super) fn header_line(header_string: &str) -> Result<Header, String> {
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

