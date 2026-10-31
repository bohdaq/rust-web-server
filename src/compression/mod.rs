#[cfg(test)]
mod tests;

pub mod dictionary;

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
    compress_with(request, response, &[Encoding::Gzip]);
}

/// Like [`apply_gzip`], with the content coding negotiated from the request's
/// `Accept-Encoding` among those compiled in ([`DYNAMIC_PREFERENCE`]): `zstd`,
/// `br` or `gzip`. What the HTTP/1.1, HTTP/2 and HTTP/3 writers call (18.1.0).
pub fn apply_compression(request: &Request, response: &mut Response) {
    compress_with(request, response, DYNAMIC_PREFERENCE);
}

fn compress_with(request: &Request, response: &mut Response, preference: &[Encoding]) {
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
    let Some(encoding) = negotiate(request, preference) else { return };
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
        match encode(encoding, &cr.body, Level::Dynamic) {
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
    mark_encoded(response, encoding);
    tag_etag(response, encoding);
}

/// A compressed body is a different representation, so it needs its own
/// ETag: `"x"` becomes `"x-br"` (weak tags too), as nginx and Caddy do.
fn tag_etag(response: &mut Response, encoding: Encoding) {
    for header in response.headers.iter_mut().filter(|h| h.name.eq_ignore_ascii_case(Header::_ETAG)) {
        let value = header.value.trim();
        if let Some(quoted) = value.strip_suffix('"') {
            let mut tagged = String::with_capacity(value.len() + 6);
            tagged.push_str(quoted);
            tagged.push('-');
            tagged.push_str(encoding.token());
            tagged.push('"');
            header.value = tagged.into();
        }
    }
}

/// A content coding rws can produce. `Brotli` and `Zstd` need the `brotli`
/// and `zstd` features (both default); without them they are never chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Encoding {
    Gzip,
    Brotli,
    Zstd,
}

impl Encoding {
    /// The `Content-Encoding` / `Accept-Encoding` token.
    pub fn token(self) -> &'static str {
        match self {
            Encoding::Gzip => "gzip",
            Encoding::Brotli => "br",
            Encoding::Zstd => "zstd",
        }
    }

    /// Whether this build can produce it.
    pub fn available(self) -> bool {
        match self {
            Encoding::Gzip => true,
            Encoding::Brotli => cfg!(feature = "brotli"),
            Encoding::Zstd => cfg!(feature = "zstd"),
        }
    }
}

/// Order of preference for responses compressed per request, when the client
/// accepts several equally: zstd costs the least CPU at a ratio close to
/// brotli's at the levels used here.
pub const DYNAMIC_PREFERENCE: &[Encoding] = &[Encoding::Zstd, Encoding::Brotli, Encoding::Gzip];

/// Order of preference for static files, compressed once per file version:
/// brotli's smaller output wins.
pub const STATIC_PREFERENCE: &[Encoding] = &[Encoding::Brotli, Encoding::Zstd, Encoding::Gzip];

/// How hard to compress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// For a body compressed on every request: fast levels (brotli 4, zstd 3,
    /// gzip 6 — what Caddy and nginx use). Brotli's default quality, 11, is
    /// about a hundred times slower than gzip.
    Dynamic,
    /// For a static file compressed once per version and cached: a notch
    /// higher (brotli 6, zstd 6, gzip 6).
    Static,
}

/// Brotli quality per [`Level`].
pub const BROTLI_QUALITY: [u32; 2] = [4, 6];
/// zstd level per [`Level`].
pub const ZSTD_LEVEL: [i32; 2] = [3, 6];

impl Level {
    fn index(self) -> usize {
        match self {
            Level::Dynamic => 0,
            Level::Static => 1,
        }
    }
}

/// The coding to use for this request among `preference` (in that order of
/// preference on equal weight): the one with the highest `q` in its
/// `Accept-Encoding`, counting `*` for codings not listed; `None` without the
/// header, or when nothing in `preference` is acceptable or compiled in.
pub fn negotiate(request: &Request, preference: &[Encoding]) -> Option<Encoding> {
    let header = request.header(Header::_ACCEPT_ENCODING)?;
    let mut best: Option<(Encoding, f32)> = None;
    for &encoding in preference.iter().filter(|e| e.available()) {
        let q = accepted_weight(&header.value, encoding.token());
        if q > 0.0 && best.is_none_or(|(_, best_q)| q > best_q) {
            best = Some((encoding, q));
        }
    }
    best.map(|(encoding, _)| encoding)
}

/// `token`'s weight in an `Accept-Encoding` value: its own entry's `q`
/// (1 when absent or unparsable), else the `*` entry's, else 0.
fn accepted_weight(value: &str, token: &str) -> f32 {
    let mut wildcard = None;
    for coding in value.split(',') {
        let mut parts = coding.split(';');
        let name = parts.next().unwrap_or("").trim();
        let q = parts
            .find_map(|p| {
                let p = p.trim();
                // Bytes, not a str slice: `p` may start with a multi-byte
                // character, and `p[..2]` would panic (found by fuzzing).
                p.as_bytes().get(..2).is_some_and(|b| b.eq_ignore_ascii_case(b"q=")).then(|| p[2..].trim().parse::<f32>().ok())
            })
            .flatten()
            .filter(|q| (0.0..=1.0).contains(q))
            .unwrap_or(1.0);
        if name.eq_ignore_ascii_case(token) {
            return q;
        }
        if name == "*" {
            wildcard = Some(q);
        }
    }
    wildcard.unwrap_or(0.0)
}

/// `body` in `encoding` at `level`; `None` if this build can't produce it or
/// the encoder fails.
pub fn encode(encoding: Encoding, body: &[u8], level: Level) -> Option<Vec<u8>> {
    match encoding {
        Encoding::Gzip => gzip(body),
        #[cfg(feature = "brotli")]
        Encoding::Brotli => {
            let mut out = Vec::with_capacity(body.len() / 3 + 64);
            {
                let mut writer = brotli::CompressorWriter::new(&mut out, 4096, BROTLI_QUALITY[level.index()], 22);
                writer.write_all(body).ok()?;
                writer.flush().ok()?;
            }
            Some(out)
        }
        #[cfg(feature = "zstd")]
        Encoding::Zstd => zstd::bulk::compress(body, ZSTD_LEVEL[level.index()]).ok(),
        #[allow(unreachable_patterns)]
        _ => {
            let _ = level;
            None
        }
    }
}

/// The headers of a body in `encoding`: `Content-Encoding`, and
/// `Accept-Encoding` appended to `Vary` (or a new `Vary`).
pub fn mark_encoded(response: &mut Response, encoding: Encoding) {
    response.headers.push(Header {
        name: Header::_CONTENT_ENCODING.into(),
        value: encoding.token().into(),
    });
    add_vary_accept_encoding(response);
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

fn add_vary_accept_encoding(response: &mut Response) {
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
