//! `Request::cursor_read` as it was before the single-pass rewrite, kept
//! verbatim as the reference for the differential test in `tests.rs`.
use std::io::{BufRead, Cursor, Read};
use crate::header::Header;
use crate::request::Request;

pub(super) fn parse(request_vec_u8: &[u8]) -> Result<Request, String> {
    let mut cursor = Cursor::new(request_vec_u8);
    let mut request = Request {
        method: "".to_string(),
        request_uri: "".to_string(),
        http_version: "".to_string(),
        headers: vec![],
        body: vec![],
    };
    cursor_read(&mut cursor, 0, &mut request, 0)?;
    Ok(request)
}

pub(super) fn cursor_read(cursor: &mut Cursor<&[u8]>, mut iteration_number: usize, request: &mut Request, content_length: usize) -> Result<bool, String> {
    let _ = content_length;
    loop {
        let mut buf = vec![];
        let bytes_offset = cursor.read_until(b'\n', &mut buf).map_err(|e| e.to_string())?;
        let string = String::from_utf8(buf).map_err(|e| e.to_string())?;

        let is_first_iteration = iteration_number == 0;
        if is_first_iteration {
            let (method, request_uri, http_version) =
                Request::parse_method_and_request_uri_and_http_version_string(&string)?;
            request.method = method;
            request.request_uri = request_uri;
            request.http_version = http_version;
        }

        // Blank line (end of the head) or end of input.
        if bytes_offset == 0 || string.trim().is_empty() {
            break;
        }

        // The request line is not a header. (This used to push an empty
        // `Header { name: "", value: "" }` for it, which the reverse proxy
        // forwarded as a malformed ": " line — nginx answers 400.)
        if !is_first_iteration {
            let header = Request::parse_http_request_header_string(&string);
            if header.name.eq_ignore_ascii_case(Header::_CONTENT_LENGTH) {
                // A client-supplied value: reject garbage (RFC 9112 §6.3) instead of panicking.
                let _: usize = header.value.trim().parse()
                    .map_err(|_| format!("invalid Content-Length: {:?}", header.value))?;
            }
            request.headers.push(header);
        }
        iteration_number += 1;
    }

    // remaining part is request body
    let mut body = vec![];
    cursor.read_to_end(&mut body).map_err(|e| e.to_string())?;
    request.body.append(&mut body);

    Ok(true)
}
