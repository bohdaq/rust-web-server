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

/// Bodies smaller than this are sent uncompressed: gzip's framing overhead
/// and the CPU time outweigh what it saves (Caddy's `encode` uses the same
/// minimum).
pub const MIN_COMPRESS_SIZE: usize = 512;

/// If the client accepts gzip and the response body is compressible text of
/// at least [`MIN_COMPRESS_SIZE`] bytes, compress every content range
/// in-place and add `Content-Encoding: gzip`; also appends `Accept-Encoding`
/// to the `Vary` header. A body that gzip doesn't make smaller (already
/// compressed or random data) is left as it was, and so is a response that
/// already has a `Content-Encoding` (e.g. a backend that compressed it).
pub fn apply_gzip(request: &Request, response: &mut Response) {
    if response.content_range_list.is_empty() {
        return;
    }
    if response.headers.iter().any(|h| h.name.eq_ignore_ascii_case(Header::_CONTENT_ENCODING)) {
        return;
    }
    let original_size: usize = response.content_range_list.iter().map(|cr| cr.body.len()).sum();
    if original_size < MIN_COMPRESS_SIZE {
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

    let mut compressed_bodies = Vec::with_capacity(response.content_range_list.len());
    for cr in &response.content_range_list {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        if encoder.write_all(&cr.body).is_err() {
            return;
        }
        match encoder.finish() {
            Ok(compressed) => compressed_bodies.push(compressed),
            Err(_) => return,
        }
    }
    let compressed_size: usize = compressed_bodies.iter().map(Vec::len).sum();
    if compressed_size >= original_size {
        return;
    }
    for (cr, compressed) in response.content_range_list.iter_mut().zip(compressed_bodies) {
        let new_len = compressed.len() as u64;
        cr.body = compressed;
        cr.range.end = new_len;
        cr.size = new_len.to_string();
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
