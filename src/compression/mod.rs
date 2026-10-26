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
    // A partial body (206, one range or several) is a slice of the
    // representation: gzipping the slice and rewriting Content-Range to the
    // compressed size describes bytes that don't exist (#52, RFC 9110 §14.4).
    if response.status_code == 206 || response.content_range_list.len() > 1 {
        return;
    }
    if response.headers.iter().any(|h| h.name.eq_ignore_ascii_case(Header::_CONTENT_ENCODING)) {
        return;
    }
    let original_size: usize = response.content_range_list.iter().map(|cr| cr.body.len()).sum();
    if original_size < MIN_COMPRESS_SIZE {
        return;
    }
    if !accepts_gzip(request) {
        return;
    }
    let content_type = response
        .content_range_list
        .first()
        .map(|cr| cr.content_type.as_str())
        .unwrap_or_default();
    if !is_compressible(content_type) {
        return;
    }

    let mut compressed_bodies = Vec::with_capacity(response.content_range_list.len());
    for cr in &response.content_range_list {
        match gzip(&cr.body) {
            Some(compressed) => compressed_bodies.push(compressed),
            None => return,
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
    mark_gzipped(response);
}

/// Whether the request's `Accept-Encoding` allows gzip: `gzip`, optionally
/// with parameters, and not `q=0` ("not acceptable").
pub(crate) fn accepts_gzip(request: &Request) -> bool {
    request
        .header(Header::_ACCEPT_ENCODING)
        .map(|h| h.value.split(',').any(|coding| {
            let mut parts = coding.split(';');
            let name_ok = parts.next().unwrap_or("").trim().eq_ignore_ascii_case("gzip");
            let refused = parts.any(|p| {
                let p = p.trim();
                p.len() >= 2 && p[..2].eq_ignore_ascii_case("q=") && p[2..].trim().parse::<f32>().is_ok_and(|q| q == 0.0)
            });
            name_ok && !refused
        }))
        .unwrap_or(false)
}

/// Whether a body of this `Content-Type` is worth compressing ([`COMPRESSIBLE`]
/// prefixes, case-insensitive).
pub(crate) fn is_compressible(content_type: &str) -> bool {
    COMPRESSIBLE.iter().any(|mime| {
        content_type.len() >= mime.len() && content_type.as_bytes()[..mime.len()].eq_ignore_ascii_case(mime.as_bytes())
    })
}

/// `body` gzip-compressed at the default level, as [`apply_gzip`] sends it.
pub(crate) fn gzip(body: &[u8]) -> Option<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(body).ok()?;
    encoder.finish().ok()
}

/// The headers of a gzip-encoded body: `Content-Encoding: gzip`, and
/// `Accept-Encoding` appended to `Vary` (or a new `Vary`).
pub(crate) fn mark_gzipped(response: &mut Response) {
    response.headers.push(Header {
        name: Header::_CONTENT_ENCODING.into(),
        value: "gzip".into(),
    });

    let vary_pos = response.headers.iter().position(|h| h.name == Header::_VARY);
    if let Some(i) = vary_pos {
        let current = response.headers[i].value.clone();
        if !current.to_lowercase().contains("accept-encoding") {
            response.headers[i].value = format!("{}, Accept-Encoding", current).into();
        }
    } else {
        response.headers.push(Header {
            name: Header::_VARY.into(),
            value: "Accept-Encoding".into(),
        });
    }
}
