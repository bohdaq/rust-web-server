//! HTTP responses: [`Response`], its writers and the upstream-response parser.
//!
//! A response has a status, headers and a body as a list of
//! [`ContentRange`](crate::range::ContentRange)s (one, or several for a
//! `multipart/byteranges` range response), or a `stream_file` / `stream_pipe`
//! for bodies too large to buffer. `Response::json` / `Response::text` build the
//! common cases. The writers (`generate_response`, `generate_response_parts`,
//! …) add `Content-Type`, `Content-Length` and `Content-Range`;
//! [`Response::parse`] reads a raw upstream response (used by the proxy).
//! Examples: `src/response/example/`.

#[cfg(test)]
mod tests;
#[cfg(test)]
mod example;
#[cfg(test)]
mod reference_parser;

use crate::ext::decimal::Decimal;
use std::io;
use std::io::{BufRead, Cursor, Read};
use crate::body::multipart_form_data::FormMultipartData;
use crate::core::New;
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

/// An HTTP response. Build one inside [`Controller::process`] and return it.
///
/// Set `status_code` and `reason_phrase` from [`STATUS_CODE_REASON_PHRASE`],
/// push body content via [`Range::get_content_range`] into `content_range_list`,
/// and add any extra headers to `headers`.
///
/// For large files, set `stream_file` to the absolute file path instead of loading
/// the body into `content_range_list`. The server will stream it with
/// `Transfer-Encoding: chunked` so memory usage stays constant regardless of file size.
///
/// For proxy passthrough (SSE, AI token streams, large downloads), set `stream_pipe`
/// to a boxed `Read` source instead. The server streams its output with
/// `Transfer-Encoding: chunked` immediately after sending the response headers.
pub struct Response {
    /// HTTP version string, e.g. `"HTTP/1.1"`.
    pub http_version: String,
    /// Numeric status code, e.g. `200`. Set from [`STATUS_CODE_REASON_PHRASE`].
    pub status_code: i16,
    /// Reason phrase, e.g. `"OK"`. Set from [`STATUS_CODE_REASON_PHRASE`].
    pub reason_phrase: String,
    /// Response headers. Pre-populated with standard headers by [`Header::get_header_list`].
    pub headers: Vec<Header>,
    /// Response body as a list of content ranges (supports multipart and range responses).
    pub content_range_list: Vec<ContentRange>,
    /// If set, the server streams this file with `Transfer-Encoding: chunked`
    /// instead of using `content_range_list`. Use for files larger than ~8 MB.
    pub stream_file: Option<String>,
    /// If set, the server pipes bytes from this reader with `Transfer-Encoding: chunked`
    /// immediately after sending the response headers. Takes precedence over
    /// `content_range_list` and `stream_file`. The reader is consumed and not cloned.
    pub stream_pipe: Option<Box<dyn std::io::Read + Send>>,
    /// Header fields sent after the body (HTTP/2 and HTTP/3; HTTP/1.1 drops
    /// them) — gRPC's `grpc-status` and `grpc-message`. A streamed body's
    /// producer fills the slot before it ends the stream. Since 18.3.0.
    pub trailers: Option<Trailers>,
}

/// Trailer fields shared between whoever produces a body and the connection
/// writer that sends them after it (see [`Response::trailers`]). Clones share
/// the one slot.
#[derive(Clone, Default, Debug)]
pub struct Trailers(std::sync::Arc<std::sync::Mutex<Vec<Header>>>);

impl Trailers {
    /// An empty slot.
    pub fn new() -> Self {
        Trailers::default()
    }

    /// Trailers already known, e.g. for a buffered body.
    pub fn with(fields: Vec<Header>) -> Self {
        let trailers = Trailers::new();
        trailers.set(fields);
        trailers
    }

    /// Replace the fields; call before the body's last byte is produced.
    pub fn set(&self, fields: Vec<Header>) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = fields;
    }

    /// The fields set so far.
    pub fn get(&self) -> Vec<Header> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl Clone for Response {
    fn clone(&self) -> Self {
        Response {
            http_version: self.http_version.clone(),
            status_code: self.status_code,
            reason_phrase: self.reason_phrase.clone(),
            headers: self.headers.clone(),
            content_range_list: self.content_range_list.clone(),
            stream_file: self.stream_file.clone(),
            stream_pipe: None,
            trailers: self.trailers.clone(),
        }
    }
}

impl PartialEq for Response {
    fn eq(&self, other: &Self) -> bool {
        self.http_version == other.http_version
            && self.status_code == other.status_code
            && self.reason_phrase == other.reason_phrase
            && self.headers == other.headers
            && self.content_range_list == other.content_range_list
            && self.stream_file == other.stream_file
    }
}

impl Eq for Response {}

impl std::fmt::Debug for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Response")
            .field("http_version", &self.http_version)
            .field("status_code", &self.status_code)
            .field("reason_phrase", &self.reason_phrase)
            .field("headers", &self.headers)
            .field("content_range_list", &self.content_range_list)
            .field("stream_file", &self.stream_file)
            .field("stream_pipe", &self.stream_pipe.as_ref().map(|_| "<stream>"))
            .finish()
    }
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

/// The `Content-Range` value for a single-part body, or `None` when the
/// response shouldn't carry one. RFC 9110 §14.4 gives `Content-Range` meaning
/// only on `206 Partial Content` (and `416`, which rws answers with a plain
/// error body), so full `200` responses get no such header. `range.end` is
/// inclusive.
pub fn content_range_header_value(status_code: i16, content_range: &ContentRange) -> Option<String> {
    if status_code != *STATUS_CODE_REASON_PHRASE.n206_partial_content.status_code {
        return None;
    }
    Some(format!(
        "{} {}-{}/{}",
        Range::BYTES, content_range.range.start, content_range.range.end, content_range.size
    ))
}

impl Response {

    /// A response with nothing in it and no allocations — a placeholder to
    /// `std::mem::replace` a real one with.
    pub(crate) fn empty() -> Response {
        Response {
            http_version: String::new(),
            status_code: 0,
            reason_phrase: String::new(),
            headers: Vec::new(),
            content_range_list: Vec::new(),
            stream_file: None,
            stream_pipe: None,
            trailers: None,
        }
    }

    /// Set the status code and reason phrase, reusing the phrase's buffer.
    pub(crate) fn set_status(&mut self, status: &StatusCodeReasonPhrase) {
        self.status_code = *status.status_code;
        self.reason_phrase.clear();
        self.reason_phrase.push_str(status.reason_phrase);
    }

    pub fn build(status: StatusCodeReasonPhrase, header_list : Vec<Header>, body: Vec<ContentRange>) -> Response {
        Response {
            http_version: VERSION.http_1_1.to_string(),
            status_code: *status.status_code,
            reason_phrase: status.reason_phrase.to_string(),
            headers: header_list,
            content_range_list: body,
            stream_file: None,
            stream_pipe: None,
            trailers: None,
        }
    }

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
        // Header names are case-insensitive (RFC 9110 §5.1), as in Request::get_header.
        let header =  self.headers.iter().find(|x| x.name.eq_ignore_ascii_case(&name));
        header
    }

    /// `true` for a status that never has content: `1xx`, `204`, `304`
    /// (RFC 9110 §6.4.1, §15.3.5, §15.4.5).
    pub fn status_has_no_content(status_code: i16) -> bool {
        (100..200).contains(&status_code) || status_code == 204 || status_code == 304
    }

    /// For a status without content ([`Response::status_has_no_content`]),
    /// drop the body — buffered, file or pipe — and any `Content-Length` or
    /// `Transfer-Encoding`, which those responses must not carry (#54).
    /// Every connection writer calls it before framing the response.
    pub fn drop_content_for_status(&mut self) {
        if !Response::status_has_no_content(self.status_code) {
            return;
        }
        self.content_range_list.clear();
        self.stream_file = None;
        self.stream_pipe = None;
        self.headers.retain(|h| {
            !h.name.eq_ignore_ascii_case(Header::_CONTENT_LENGTH)
                && !h.name.eq_ignore_ascii_case(Header::_TRANSFER_ENCODING)
        });
    }

    pub fn generate_body(content_range_list: Vec<ContentRange>) -> Vec<u8> {
        let mut body = vec![];
        let one = 1;

        if content_range_list.len() == one {
            // Move the body out rather than copy it: for a large file that
            // copy was a second full-size buffer per in-flight response.
            let mut content_range_list = content_range_list;
            return content_range_list.swap_remove(0).body;
        }

        if content_range_list.len() > one {
            // RFC 2046 §5.1.1: each part opens with "--boundary", the body
            // closes with "--boundary--" (#53).
            let boundary = Range::multipart_boundary(&content_range_list);
            for (i, content_range) in content_range_list.iter().enumerate() {
                let mut body_str = SYMBOL.empty_string.to_string();
                if i != 0 {
                    body_str.push_str(SYMBOL.new_line_carriage_return);
                }
                body_str.push_str(SYMBOL.hyphen);
                body_str.push_str(SYMBOL.hyphen);
                body_str.push_str(&boundary);
                body_str.push_str(SYMBOL.new_line_carriage_return);
                let content_type = [Header::_CONTENT_TYPE, Header::NAME_VALUE_SEPARATOR, SYMBOL.whitespace, &Header::single_line(&content_range.content_type)].join("");
                body_str.push_str(content_type.as_str());
                body_str.push_str(SYMBOL.new_line_carriage_return);
                let content_range_header = [Header::_CONTENT_RANGE, Header::NAME_VALUE_SEPARATOR, SYMBOL.whitespace, Range::BYTES, SYMBOL.whitespace, &content_range.range.start.to_string(), SYMBOL.hyphen, &content_range.range.end.to_string(), SYMBOL.slash, &Header::single_line(&content_range.size)].join("");
                body_str.push_str(content_range_header.as_str());
                body_str.push_str(SYMBOL.new_line_carriage_return);
                body_str.push_str(SYMBOL.new_line_carriage_return);

                body.extend_from_slice(body_str.as_bytes());
                body.extend_from_slice(&content_range.body);
            }
            body.extend_from_slice(b"\r\n--");
            body.extend_from_slice(boundary.as_bytes());
            body.extend_from_slice(b"--\r\n");
        }

        body
    }

    /// The serialized response: status line + headers, then the body.
    pub fn generate_response(response: Response, request: Request) -> Vec<u8> {
        Response::generate_response_coalesced(response, request, usize::MAX).0
    }

    /// Like [`generate_response`](Self::generate_response), but returns the
    /// head (status line + headers + blank line) and the body separately, so
    /// a connection writer can send them back to back without first copying
    /// a large body into one combined buffer. The body is empty for HEAD and
    /// for a status without content.
    pub fn generate_response_parts(response: Response, request: Request) -> (Vec<u8>, Vec<u8>) {
        Response::generate_response_coalesced(response, request, 0)
    }

    /// [`generate_response_parts`](Self::generate_response_parts), except
    /// that a body of at most `coalesce_limit` bytes is appended to the head
    /// (sized for both up front) and the returned body is then empty — one
    /// buffer for one `write`, without growing the head afterwards.
    pub(crate) fn generate_response_coalesced(mut response: Response, request: Request, coalesce_limit: usize) -> (Vec<u8>, Vec<u8>) {
        response.drop_content_for_status();
        let Response { http_version, status_code, reason_phrase, headers, mut content_range_list, .. } = response;

        // Content-Type, Content-Range and Content-Length follow the other
        // headers; they're written straight into the head rather than pushed
        // onto `headers` first.
        let mut content_type: Option<String> = None;
        let mut content_range: Option<String> = None;
        let mut content_length: Option<Decimal> = None;
        if content_range_list.len() == 1 {
            let range = &mut content_range_list[0];
            content_range = content_range_header_value(status_code, range);
            content_length = Some(Decimal::new(range.body.len() as u128));
            // A single range's body is moved out by generate_body, which
            // doesn't look at its type.
            content_type = Some(std::mem::take(&mut range.content_type));
        }
        let multipart = content_range_list.len() > 1;
        if multipart {
            content_type = Some(Range::multipart_content_type(&content_range_list));
        }

        // HEAD: the head of the GET response, no body. OPTIONS gets its body
        // like any other method: dropping it under a Content-Length that
        // announced it left the client waiting (#54).
        let is_head = request.method == METHOD.head;
        let body = if is_head && !multipart {
            Vec::new()
        } else {
            Response::generate_body(content_range_list)
        };
        if multipart {
            // Without it a keep-alive client can't tell where the body ends
            // (RFC 9112 §6.3; #53). A HEAD gets the GET's length.
            content_length = Some(Decimal::new(body.len() as u128));
        }
        let body = if is_head { Vec::new() } else { body };
        let coalesce = body.len() <= coalesce_limit;

        // One buffer, sized up front, written once: this used to build a
        // String per header, grow a second one, join the status line and
        // copy it all again with format! — the biggest allocation cost of a
        // small response.
        let status = u16::try_from(status_code).map(|code| Decimal::new(code as u128));
        let status_text = match &status {
            Ok(decimal) => std::borrow::Cow::Borrowed(decimal.as_str()),
            Err(_) => std::borrow::Cow::Owned(status_code.to_string()),
        };
        let extra = [
            (Header::_CONTENT_TYPE, content_type.as_deref()),
            (Header::_CONTENT_RANGE, content_range.as_deref()),
            (Header::_CONTENT_LENGTH, content_length.as_ref().map(Decimal::as_str)),
        ];
        let capacity = http_version.len() + status_text.len() + reason_phrase.len() + 4
            + headers.iter().map(|h| h.name.len() + h.value.len() + 4).sum::<usize>()
            + extra.iter().map(|(name, value)| value.map_or(0, |v| name.len() + v.len() + 4)).sum::<usize>()
            + 2
            + if coalesce { body.len() } else { 0 };
        let mut head = Vec::with_capacity(capacity);
        head.extend_from_slice(http_version.as_bytes());
        head.push(b' ');
        head.extend_from_slice(status_text.as_bytes());
        head.push(b' ');
        head.extend_from_slice(Header::single_line(&reason_phrase).as_bytes());
        head.extend_from_slice(b"\r\n");
        let header_lines = headers.iter().map(|h| (h.name.as_ref(), h.value.as_ref()))
            .chain(extra.iter().filter_map(|(name, value)| value.map(|v| (*name, v))));
        for (name, value) in header_lines {
            head.extend_from_slice(Header::single_line(name).as_bytes());
            head.extend_from_slice(Header::NAME_VALUE_SEPARATOR.as_bytes());
            head.extend_from_slice(Header::single_line(value).as_bytes());
            head.extend_from_slice(b"\r\n");
        }
        head.extend_from_slice(b"\r\n");

        if coalesce {
            head.extend_from_slice(&body);
            (head, Vec::new())
        } else {
            (head, body)
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
            stream_file: None,
            stream_pipe: None,
            trailers: None,
        };

        let content_length: usize = 0;
        let iteration_number : usize = 0;

        Response::_parse_raw_response_via_cursor(&mut cursor, iteration_number, &mut response, content_length);

        return response;
    }

    pub fn _parse_http_version_status_code_reason_phrase_string(http_version_status_code_reason_phrase: &str) -> Result<(String, i16, String), String> {
        let line = without_cr_lf(http_version_status_code_reason_phrase);
        let (http_version, status_code, reason_phrase) = Response::status_line_parts(&line)?;
        Ok((http_version.to_string(), status_code, reason_phrase.to_string()))
    }

    /// `HTTP-version SP status-code [SP reason-phrase]`, CR/LF already
    /// removed, borrowed from `line`.
    fn status_line_parts(line: &str) -> Result<(&str, i16, &str), String> {
        let boxed_split = line.split_once(SYMBOL.whitespace);
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

        Ok((http_version, status_code_i16, reason_phrase))
    }

    pub fn _parse_http_response_header_string(header_string: &str) -> Header {
        // A line without ": " (malformed upstream input) is a name with an
        // empty value, as in Request::parse_http_request_header_string —
        // not an index-out-of-bounds panic.
        let (header_name, raw_header_value) = header_string
            .split_once(Header::NAME_VALUE_SEPARATOR)
            .unwrap_or((header_string, ""));
        let header_value = StringExt::truncate_new_line_carriage_return(raw_header_value);


        Header {
            name: header_name.to_string().into(),
            value: header_value.to_string().into()
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
            // No Content-Type (a 204, a 304): an empty, non-multipart body.
            let content_type = response._get_header(Header::_CONTENT_TYPE.to_string()).cloned()
                .unwrap_or(Header { name: Header::_CONTENT_TYPE.into(), value: "".into() });
            let is_multipart = Response::_is_multipart_byteranges_content_type(&content_type);

            if is_multipart {
                let content_range_list : Vec<ContentRange> = vec![];

                let boundary = content_type.value.split(';')
                    .filter_map(|param| param.trim().strip_prefix("boundary="))
                    .next()
                    .map(|b| b.trim_matches('"').to_string())
                    .unwrap_or_else(|| Range::STRING_SEPARATOR.to_string());
                let mut buf = vec![];
                let _ = cursor.read_until(b'\n', &mut buf);
                let boxed_value = Range::_parse_multipart_body_with_boundary(cursor, content_range_list, &boundary);
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
            let mut header = Header { name: "".into(), value: "".into() };
            if !is_first_iteration {
                header = Response::_parse_http_response_header_string(&string);
                if header.name.eq_ignore_ascii_case(Header::_CONTENT_LENGTH) {
                    // No error channel here: treat an invalid value as absent.
                    content_length = header.value.trim().parse().unwrap_or(0);
                }
            }

            response.headers.push(header);
            iteration_number += 1;
            Response::_parse_raw_response_via_cursor(cursor, iteration_number, response, content_length);
        }
    }

    pub fn _is_multipart_byteranges_content_type(content_type: &Header) -> bool {
        // Asked of every parsed (e.g. every proxied) response: no join.
        content_type.value.strip_prefix(Range::MULTIPART)
            .and_then(|rest| rest.strip_prefix(SYMBOL.slash))
            .is_some_and(|rest| rest.starts_with(Range::BYTERANGES))
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
            content_range_list,
            stream_file: None,
            stream_pipe: None,
            trailers: None,
        };

        response
    }

    /// Build a `Content-Type: application/json` response from already-serialized bytes.
    ///
    /// Takes raw bytes rather than a `Serialize` value so it carries no dependency on the
    /// `serde` feature — pair it with `serde_json::to_vec` (or hand-rolled JSON) to build `body`.
    /// For a one-line serialize-and-respond helper on a typed value, see [`crate::json::Json`]
    /// (`serde` feature).
    ///
    /// ```rust,no_run
    /// use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
    ///
    /// let response = Response::json(STATUS_CODE_REASON_PHRASE.n200_ok, br#"{"ok":true}"#.to_vec());
    /// ```
    pub fn json(status_code_reason_phrase: &StatusCodeReasonPhrase, body: Vec<u8>) -> Response {
        let content_range = Range::get_content_range(body, MimeType::APPLICATION_JSON.to_string());
        Response::get_response(status_code_reason_phrase, None, Some(vec![content_range]))
    }

    /// Build a `Content-Type: text/plain` response from a body string.
    ///
    /// ```rust,no_run
    /// use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
    ///
    /// let response = Response::text(STATUS_CODE_REASON_PHRASE.n400_bad_request, "invalid input");
    /// ```
    pub fn text(status_code_reason_phrase: &StatusCodeReasonPhrase, body: &str) -> Response {
        let content_range = Range::get_content_range(body.as_bytes().to_vec(), MimeType::TEXT_PLAIN.to_string());
        Response::get_response(status_code_reason_phrase, None, Some(vec![content_range]))
    }

    pub fn generate(&mut self) -> Vec<u8> {
        let response = &mut self.clone();
        response.drop_content_for_status();

        if response.content_range_list.len() == 1 {
            let content_range_index = 0;
            let content_range = response.content_range_list.get(content_range_index).unwrap();
            response.headers.push(Header {
                name: Header::_CONTENT_TYPE.into(),
                value: content_range.content_type.to_string().into()
            });

            if let Some(value) = content_range_header_value(response.status_code, content_range) {
                response.headers.push(Header {
                    name: Header::_CONTENT_RANGE.into(),
                    value: value.into(),
                });
            }

            response.headers.push(Header {
                name: Header::_CONTENT_LENGTH.into(),
                value: content_range.body.len().to_string().into()
            });
        }

        if response.content_range_list.len() > 1 {
            response.headers.push(Header {
                name: Header::_CONTENT_TYPE.into(),
                value: Range::multipart_content_type(&response.content_range_list).into(),
            });
        }

        let response_clone = response.clone();
        let body = Response::generate_body(response_clone.content_range_list);
        if response.content_range_list.len() > 1 {
            response.headers.push(Header {
                name: Header::_CONTENT_LENGTH.into(),
                value: body.len().to_string().into(),
            });
        }

        let mut headers_str = SYMBOL.new_line_carriage_return.to_string();

        let header_list = response.headers.clone();
        for header in header_list {
            let mut header_string = SYMBOL.empty_string.to_string();
            header_string.push_str(&Header::single_line(&header.name));
            header_string.push_str(Header::NAME_VALUE_SEPARATOR);
            header_string.push_str(&Header::single_line(&header.value));
            header_string.push_str(SYMBOL.new_line_carriage_return);
            headers_str.push_str(&header_string);
        }

        let response_clone = response.clone();
        let status = [response_clone.http_version, response.status_code.to_string(), Header::single_line(&response_clone.reason_phrase).into_owned()].join(SYMBOL.whitespace);
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

    /// The first header named `name` (case-insensitive), without allocating.
    pub fn header(&self, name: &str) -> Option<&Header> {
        self.headers.iter().find(|h| h.name.eq_ignore_ascii_case(name))
    }

    pub fn parse(response_vec_u8: &[u8]) -> Result<Response, String> {
        let total_bytes : i32 = response_vec_u8.len() as i32;
        let bytes_read = 0;
        let mut cursor = io::Cursor::new(response_vec_u8);

        let mut response = Response {
            http_version: "".to_string(),
            status_code: 0,
            reason_phrase: "".to_string(),
            // Typical upstream heads have 6–16 headers: grown 1→2→4→8→16
            // this was four allocations per proxied response.
            headers: Vec::with_capacity(16),
            content_range_list: vec![],
            stream_file: None,
            stream_pipe: None,
            trailers: None,
        };

        let content_length: usize = 0;
        let iteration_number : usize = 0;

        let boxed_parse = Response::parse_raw_response_via_cursor(
            &mut cursor, iteration_number,
            &mut response,
            content_length,
            total_bytes,
            bytes_read
        );
        if boxed_parse.is_err() {
            let message = boxed_parse.err().unwrap();
            return Err(message);
        }

        return Ok(response);
    }

    /// [`Response::parse`] and `proxy::normalize_upstream_response` in one
    /// pass, for a buffered upstream response: the status line is re-emitted
    /// as HTTP/1.1, headers named in `dropped` (compared ignoring ASCII case)
    /// are skipped before their value is copied, and every `Content-Type`
    /// moves onto the body (the first one is its type). A
    /// `multipart/byteranges` response goes the general way. The result is
    /// what the two-step path builds (`response_tests::parse_upstream_*`) —
    /// minus a copy per dropped header, a second scan for `Content-Type`
    /// and the `retain` that removed them.
    pub(crate) fn parse_upstream(input: &[u8], dropped: &[&str]) -> Result<Response, String> {
        let mut response = Response {
            http_version: crate::http::VERSION.http_1_1.to_string(),
            status_code: 0,
            reason_phrase: String::new(),
            headers: Vec::with_capacity(16),
            content_range_list: vec![],
            stream_file: None,
            stream_pipe: None,
            trailers: None,
        };
        let mut content_type: Option<&str> = None;
        let mut pos = 0usize;
        let mut first = true;
        loop {
            let rest = &input[pos.min(input.len())..];
            let line_len = rest.iter().position(|&b| b == b'\n').map_or(rest.len(), |i| i + 1);
            pos += line_len;
            let line = std::str::from_utf8(&rest[..line_len]).map_err(|e| e.to_string())?;
            if first {
                let status_line = without_cr_lf(line);
                let (_, status_code, reason_phrase) = Response::status_line_parts(&status_line)?;
                response.status_code = status_code;
                response.reason_phrase = reason_phrase.to_string();
            }
            if line.trim().is_empty() {
                if content_type.is_some_and(|ct| ct.starts_with("multipart/byteranges")) {
                    return Response::parse(input).map(normalized_like_proxy(dropped));
                }
                let body = input[pos.min(input.len())..].to_vec();
                response.content_range_list = vec![ContentRange {
                    unit: Range::BYTES.to_string(),
                    range: Range { start: 0, end: body.len() as u64 },
                    size: Decimal::new(body.len() as u128).as_str().to_string(),
                    content_type: content_type.unwrap_or(MimeType::APPLICATION_OCTET_STREAM).to_string(),
                    body,
                }];
                return Ok(response);
            }
            if line_len == 0 {
                return Err("unable to parse".to_string());
            }
            if !first {
                let (name, raw_value) = split_name_value(line).ok_or_else(|| format!("unable to parse header: {}", line))?;
                if name.eq_ignore_ascii_case(Header::_CONTENT_LENGTH) {
                    let value = without_cr_lf(raw_value);
                    let _: usize = value.trim().parse().map_err(|_| format!("invalid Content-Length: {:?}", value))?;
                }
                if name.eq_ignore_ascii_case(Header::_CONTENT_TYPE) {
                    if content_type.is_none() {
                        // Borrowed from `input`; an embedded CR/LF (rare)
                        // falls back to the general path to stay exact.
                        match without_cr_lf(raw_value) {
                            std::borrow::Cow::Borrowed(v) => content_type = Some(v),
                            std::borrow::Cow::Owned(_) => return Response::parse(input).map(normalized_like_proxy(dropped)),
                        }
                    }
                    continue;
                }
                if !dropped.iter().any(|d| name.eq_ignore_ascii_case(d)) {
                    response.headers.push(Header {
                        name: Header::name_from(name),
                        value: without_cr_lf(raw_value).into_owned().into(),
                    });
                }
            }
            first = false;
        }
    }

    pub fn parse_raw_response_via_cursor(
        cursor: &mut Cursor<&[u8]>,
        mut iteration_number: usize,
        response: &mut Response,
        _content_length: usize,
        total_bytes: i32,
        mut bytes_read: i32) -> Result<(), String> {
        // One pass over the input: each line is a slice of it (this used to
        // copy every line into a Vec and then a String, and strip CR/LF with
        // two `str::replace` allocations per header), and only the values
        // kept are allocated. A loop, not recursion: one stack frame per
        // header line let a backend overflow the stack.
        let input: &[u8] = cursor.get_ref();
        loop {
            let start = (cursor.position() as usize).min(input.len());
            let rest = &input[start..];
            let line_len = rest.iter().position(|&b| b == b'\n').map_or(rest.len(), |i| i + 1);
            cursor.set_position((start + line_len) as u64);
            bytes_read += line_len as i32;
            let line = std::str::from_utf8(&rest[..line_len]).map_err(|e| e.to_string())?;

            let is_first_iteration = iteration_number == 0;
            let new_line_char_found = line_len != 0;
            let current_string_is_empty = line.trim().is_empty();

            if is_first_iteration {
                let status_line = without_cr_lf(line);
                let (http_version, status_code, reason_phrase) = Response::status_line_parts(&status_line)?;
                response.http_version = http_version.to_string();
                response.status_code = status_code;
                response.reason_phrase = reason_phrase.to_string();
            }

            if current_string_is_empty {
                // if response does not contain Content-Type, it will be defaulted to APPLICATION_OCTET_STREAM
                let content_type_header = response.header(Header::_CONTENT_TYPE);
                let is_multipart = content_type_header.map_or(false, Response::_is_multipart_byteranges_content_type);

                if is_multipart {
                    let content_type = response.header(Header::_CONTENT_TYPE).ok_or("Content-Type is missing")?;
                    let boundary = FormMultipartData::extract_boundary(content_type.value.as_ref())
                        .map_err(|_| "unable to extract boundary from Content-Type".to_string())?;
                    let is_opening_boundary_read = false;
                    response.content_range_list = Range::parse_multipart_body_with_boundary(
                        cursor,
                        vec![],
                        boundary,
                        total_bytes,
                        bytes_read,
                        is_opening_boundary_read)?;
                } else {
                    let content_type = content_type_header.map_or(MimeType::APPLICATION_OCTET_STREAM, |h| h.value.as_ref());
                    let start = (cursor.position() as usize).min(input.len());
                    let body = input[start..].to_vec();
                    cursor.set_position(input.len() as u64);
                    let content_range = ContentRange {
                        unit: Range::BYTES.to_string(),
                        range: Range {
                            start: 0,
                            end: body.len() as u64
                        },
                        size: Decimal::new(body.len() as u128).as_str().to_string(),
                        content_type: content_type.to_string(),
                        body,
                    };
                    response.content_range_list = vec![content_range];
                }

                return Ok(());
            }

            if new_line_char_found {
                if !is_first_iteration {
                    let header = Response::parse_http_response_header_string(line)?;
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

    pub fn parse_http_response_header_string(header_string: &str) -> Result<Header, String> {
        match split_name_value(header_string) {
            Some((header_name, raw_header_value)) => Ok(Header {
                name: Header::name_from(header_name),
                value: without_cr_lf(raw_header_value).into_owned().into(),
            }),
            None => Err(format!("unable to parse header: {}", header_string)),
        }
    }
}

impl New for Response {
    fn new() -> Self {
        Response {
            http_version: VERSION.http_1_1.to_string(),
            status_code: *STATUS_CODE_REASON_PHRASE.n501_not_implemented.status_code,
            reason_phrase: STATUS_CODE_REASON_PHRASE.n501_not_implemented.reason_phrase.to_string(),
            headers: vec![],
            content_range_list: vec![],
            stream_file: None,
            stream_pipe: None,
            trailers: None,
        }
    }
}
/// The general path of [`Response::parse_upstream`]: what
/// `proxy::normalize_upstream_response` does to a parsed response.
fn normalized_like_proxy<'a>(dropped: &'a [&'a str]) -> impl FnOnce(Response) -> Response + 'a {
    move |mut response: Response| {
        response.http_version.clear();
        response.http_version.push_str(crate::http::VERSION.http_1_1);
        let has_body = !response.content_range_list.is_empty();
        if let [range] = response.content_range_list.as_mut_slice() {
            if let Some(h) = response.headers.iter().find(|h| h.name.eq_ignore_ascii_case(Header::_CONTENT_TYPE)) {
                range.content_type.clear();
                range.content_type.push_str(&h.value);
            }
        }
        response.headers.retain(|h| {
            if h.name.eq_ignore_ascii_case(Header::_CONTENT_TYPE) {
                return !has_body;
            }
            !dropped.iter().any(|d| h.name.eq_ignore_ascii_case(d))
        });
        response
    }
}

/// `s` with every CR and LF removed — borrowed when they only end the line.
fn without_cr_lf(s: &str) -> std::borrow::Cow<'_, str> {
    let trimmed = s.strip_suffix('\n').unwrap_or(s);
    let trimmed = trimmed.strip_suffix('\r').unwrap_or(trimmed);
    if trimmed.contains(['\r', '\n']) {
        std::borrow::Cow::Owned(StringExt::truncate_new_line_carriage_return(s))
    } else {
        std::borrow::Cow::Borrowed(trimmed)
    }
}

/// `line` split at its first `": "` — what `split_once(": ")` returns,
/// without a substring searcher.
fn split_name_value(line: &str) -> Option<(&str, &str)> {
    let bytes = line.as_bytes();
    let mut from = 0;
    while let Some(offset) = bytes[from..].iter().position(|&b| b == b':') {
        let colon = from + offset;
        if bytes.get(colon + 1) == Some(&b' ') {
            return Some((&line[..colon], &line[colon + 2..]));
        }
        from = colon + 1;
    }
    None
}
