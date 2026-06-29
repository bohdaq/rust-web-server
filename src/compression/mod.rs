#[cfg(test)]
mod tests;

use flate2::Compression;
use flate2::write::GzEncoder;
use std::io::Write;
use crate::header::Header;
use crate::request::Request;
use crate::response::Response;

/// MIME types whose responses are worth compressing.
const COMPRESSIBLE: &[&str] = &[
    "text/html",
    "text/css",
    "text/javascript",
    "text/plain",
    "text/xml",
    "application/json",
    "application/javascript",
    "application/xml",
    "application/xhtml+xml",
    "image/svg+xml",
];

/// If the client accepts gzip and the response body is compressible text,
/// compress every content range in-place and add `Content-Encoding: gzip`.
/// Also appends `Accept-Encoding` to the `Vary` header.
pub fn apply_gzip(request: &Request, response: &mut Response) {
    if response.content_range_list.is_empty() {
        return;
    }

    let accepts_gzip = request
        .get_header(Header::_ACCEPT_ENCODING.to_string())
        .map(|h| h.value.to_lowercase().contains("gzip"))
        .unwrap_or(false);

    if !accepts_gzip {
        return;
    }

    let content_type = response
        .content_range_list
        .first()
        .map(|cr| cr.content_type.to_lowercase())
        .unwrap_or_default();

    let is_compressible = COMPRESSIBLE.iter().any(|mime| content_type.starts_with(mime));
    if !is_compressible {
        return;
    }

    let mut compressed_ok = true;
    for cr in &mut response.content_range_list {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        if encoder.write_all(&cr.body).is_err() {
            compressed_ok = false;
            break;
        }
        match encoder.finish() {
            Ok(compressed) => {
                let new_len = compressed.len() as u64;
                cr.body = compressed;
                cr.range.end = new_len;
                cr.size = new_len.to_string();
            }
            Err(_) => {
                compressed_ok = false;
                break;
            }
        }
    }

    if !compressed_ok {
        return;
    }

    response.headers.push(Header {
        name: Header::_CONTENT_ENCODING.to_string(),
        value: "gzip".to_string(),
    });

    // append Accept-Encoding to Vary, or add it
    let vary_pos = response.headers.iter().position(|h| h.name == Header::_VARY);
    if let Some(i) = vary_pos {
        let current = response.headers[i].value.clone();
        if !current.to_lowercase().contains("accept-encoding") {
            response.headers[i].value = format!("{}, Accept-Encoding", current);
        }
    } else {
        response.headers.push(Header {
            name: Header::_VARY.to_string(),
            value: "Accept-Encoding".to_string(),
        });
    }
}
