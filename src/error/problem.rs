//! RFC 9457 problem details for the errors rws itself produces (18.17.0):
//! `application/problem+json` bodies such as
//! `{"type":"about:blank","title":"Bad Gateway","status":502,"detail":"…"}`.
//! `RWS_CONFIG_ERROR_FORMAT=text` keeps the plain-text bodies of earlier
//! releases (`502 Bad Gateway`, `413 Payload Too Large: …`).

use crate::response::{Response, StatusCodeReasonPhrase};

/// `Content-Type` of a problem details body.
pub const PROBLEM_JSON: &str = "application/problem+json";

/// Whether rws-made errors use plain text (`error_format = "text"`).
pub fn text_errors() -> bool {
    crate::server_config::ServerConfig::current().error_format_text
}

/// An rws-made error: problem details, or with `error_format = "text"` the
/// text `"<code> <reason>[: <detail>]"`.
pub fn problem(status: &StatusCodeReasonPhrase, detail: &str) -> Response {
    let text = if detail.is_empty() {
        format!("{} {}", status.status_code, status.reason_phrase)
    } else {
        format!("{} {}: {detail}", status.status_code, status.reason_phrase)
    };
    problem_with_text(status, detail, &text)
}

/// [`problem`] whose text form is `text` rather than the status line.
pub fn problem_with_text(status: &StatusCodeReasonPhrase, detail: &str, text: &str) -> Response {
    with_headers(status, detail, text, None)
}

pub(crate) fn with_headers(status: &StatusCodeReasonPhrase, detail: &str, text: &str, headers: Option<Vec<crate::header::Header>>) -> Response {
    let as_text = text_errors();
    if as_text && text.is_empty() {
        return Response::get_response(status, headers, None);
    }
    let (body, content_type) = body(status, detail, text, as_text);
    let content_range = crate::range::Range::get_content_range(body, content_type.to_string());
    Response::get_response(status, headers, Some(vec![content_range]))
}

/// The body and its content type, for writers that build the response
/// head themselves.
pub(crate) fn body(status: &StatusCodeReasonPhrase, detail: &str, text: &str, as_text: bool) -> (Vec<u8>, &'static str) {
    if as_text {
        return (text.as_bytes().to_vec(), crate::mime_type::MimeType::TEXT_PLAIN);
    }
    let mut json = format!(r#"{{"type":"about:blank","title":{},"status":{}"#, quote(status.reason_phrase), status.status_code);
    if !detail.is_empty() {
        json.push_str(r#","detail":"#);
        json.push_str(&quote(detail));
    }
    json.push('}');
    (json.into_bytes(), PROBLEM_JSON)
}

/// The body for an error whose text form is `text` (`"413 Payload Too
/// Large: …"`): the text, or problem details with the part after the status
/// line as `detail`.
pub(crate) fn from_text(status: &StatusCodeReasonPhrase, text: &str) -> (Vec<u8>, &'static str) {
    let line = format!("{} {}", status.status_code, status.reason_phrase);
    let detail = text.strip_prefix(&line).map(|rest| rest.trim_start_matches(':').trim()).unwrap_or(text);
    body(status, detail, text, text_errors())
}

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
