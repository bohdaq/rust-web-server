//! Dictionary compression (RFC 9842, 18.12.0, #38): a response compressed
//! against a resource the client already has — usually the previous
//! version of the same script or stylesheet — so only the difference
//! crosses the wire.
//!
//! The client lists the dictionary it holds in `Available-Dictionary`
//! (`:<base64 SHA-256>:`) and accepts `dcz` (zstd, the dictionary as a raw
//! prefix) or `dcb` (brotli, the dictionary as a custom dictionary). The
//! encoded body starts with a fixed magic and the dictionary's hash, then
//! the compressed stream.
//!
//! ```rust
//! use rust_web_server::compression::dictionary::{decode, encode, DictionaryEncoding};
//! use rust_web_server::compression::Level;
//!
//! let v1 = b"function greet(name) { return 'Hello, ' + name; }".repeat(20);
//! let v2 = b"function greet(name) { return 'Hello there, ' + name; }".repeat(20);
//! # if DictionaryEncoding::Dcz.available() {
//! let delta = encode(DictionaryEncoding::Dcz, &v2, &v1, Level::Static).unwrap();
//! assert!(delta.len() < 100);
//! assert_eq!(decode(DictionaryEncoding::Dcz, &delta, &v1, 1 << 20).unwrap(), v2);
//! # }
//! ```
//!
//! Static files use this automatically when `compression_dictionaries` is
//! set; see the Compression docs.

use crate::header::Header;
use crate::request::Request;
use crate::response::Response;

use super::Level;

/// The first bytes of a `dcz` body (a zstd skippable frame), before the hash.
pub const DCZ_MAGIC: [u8; 8] = [0x5e, 0x2a, 0x4d, 0x18, 0x20, 0x00, 0x00, 0x00];
/// The first bytes of a `dcb` body, before the hash.
pub const DCB_MAGIC: [u8; 4] = [0xff, 0x44, 0x43, 0x42];
/// The largest dictionary or body encoded here (1 MiB each). Together they
/// fit the 8 MiB window browsers support.
pub const MAX_SIZE: usize = 1024 * 1024;

/// A dictionary's SHA-256, as `Available-Dictionary` names it.
pub type DictionaryHash = [u8; 32];

/// A dictionary-compressed content coding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DictionaryEncoding {
    /// zstd with the dictionary as a raw prefix (needs the `zstd` feature).
    Dcz,
    /// Brotli with the dictionary as a custom dictionary (needs `brotli`).
    Dcb,
}

impl DictionaryEncoding {
    /// The `Content-Encoding` / `Accept-Encoding` token.
    pub fn token(self) -> &'static str {
        match self {
            DictionaryEncoding::Dcz => "dcz",
            DictionaryEncoding::Dcb => "dcb",
        }
    }

    /// Whether this build can produce it.
    pub fn available(self) -> bool {
        match self {
            DictionaryEncoding::Dcz => cfg!(feature = "zstd"),
            DictionaryEncoding::Dcb => cfg!(feature = "brotli"),
        }
    }

    fn magic(self) -> &'static [u8] {
        match self {
            DictionaryEncoding::Dcz => &DCZ_MAGIC,
            DictionaryEncoding::Dcb => &DCB_MAGIC,
        }
    }

    /// Bytes before the compressed stream: magic and hash.
    pub fn header_len(self) -> usize {
        self.magic().len() + 32
    }
}

/// On equal weight: zstd (less CPU, and a dictionary of any size), then
/// brotli.
pub const PREFERENCE: &[DictionaryEncoding] = &[DictionaryEncoding::Dcz, DictionaryEncoding::Dcb];

/// The hash that names `dictionary`.
pub fn hash(dictionary: &[u8]) -> DictionaryHash {
    crate::core::sha256::sha256(dictionary)
}

/// An `Available-Dictionary` value: a structured-field byte sequence
/// `:<base64>:` (parameters after it ignored) holding a 32-byte hash.
pub fn parse_available_dictionary(value: &str) -> Option<DictionaryHash> {
    let rest = value.trim().strip_prefix(':')?;
    let end = rest.find(':')?;
    let (encoded, after) = (&rest[..end], &rest[end + 1..]);
    if !(after.is_empty() || after.starts_with(';')) || encoded.len() != 44 {
        return None;
    }
    if !encoded.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=')) {
        return None;
    }
    let bytes = crate::core::base64::Base64::decode(encoded.to_string()).ok()?;
    bytes.try_into().ok()
}

/// The hash in this request's `Available-Dictionary`, if any.
pub fn available_dictionary(request: &Request) -> Option<DictionaryHash> {
    parse_available_dictionary(&request.header("Available-Dictionary")?.value)
}

/// The dictionary coding to use: the one in [`PREFERENCE`] with the highest
/// non-zero weight in `Accept-Encoding` (a `*` doesn't count — a client
/// asks for these by name), compiled in.
pub fn negotiate(request: &Request) -> Option<DictionaryEncoding> {
    let header = request.header(Header::_ACCEPT_ENCODING)?;
    let mut best: Option<(DictionaryEncoding, f32)> = None;
    for &encoding in PREFERENCE.iter().filter(|e| e.available()) {
        let listed = header.value.split(',').any(|c| c.split(';').next().unwrap_or("").trim().eq_ignore_ascii_case(encoding.token()));
        let q = if listed { super::accepted_weight(&header.value, encoding.token()) } else { 0.0 };
        if q > 0.0 && best.is_none_or(|(_, best_q)| q > best_q) {
            best = Some((encoding, q));
        }
    }
    best.map(|(encoding, _)| encoding)
}

/// The smallest power-of-two window, as a log, holding `bytes`.
fn window_log(bytes: usize) -> u32 {
    (usize::BITS - bytes.max(1).saturating_sub(1).leading_zeros()).max(10)
}

/// `body` compressed against `dictionary`, with the magic and hash in
/// front. `None` when the coding isn't compiled in, either input is over
/// [`MAX_SIZE`], or the encoder fails.
pub fn encode(encoding: DictionaryEncoding, body: &[u8], dictionary: &[u8], level: Level) -> Option<Vec<u8>> {
    if body.len() > MAX_SIZE || dictionary.len() > MAX_SIZE || !encoding.available() {
        return None;
    }
    let mut out = Vec::with_capacity(encoding.header_len() + body.len() / 4 + 64);
    out.extend_from_slice(encoding.magic());
    out.extend_from_slice(&hash(dictionary));
    // The window covers dictionary and body, so the whole dictionary can be
    // referenced (at most 2 MiB + a little, under the 8 MiB browsers allow).
    let window = window_log(dictionary.len() + body.len() + 16);
    match encoding {
        #[cfg(feature = "zstd")]
        DictionaryEncoding::Dcz => {
            let mut compressor = zstd::bulk::Compressor::new(super::ZSTD_LEVEL[level.index()]).ok()?;
            compressor.set_parameter(zstd::zstd_safe::CParameter::WindowLog(window)).ok()?;
            compressor.context_mut().ref_prefix(dictionary).ok()?;
            out.extend_from_slice(&compressor.compress(body).ok()?);
        }
        #[cfg(feature = "brotli")]
        DictionaryEncoding::Dcb => {
            let params = brotli::enc::BrotliEncoderParams {
                quality: super::BROTLI_QUALITY[level.index()] as i32,
                lgwin: window.min(24) as i32,
                ..Default::default()
            };
            let mut nop = |_: &mut brotli::interface::PredictionModeContextMap<brotli::InputReferenceMut>,
                           _: &mut [brotli::interface::StaticCommand],
                           _: brotli::InputPair,
                           _: &mut brotli::enc::StandardAlloc| ();
            brotli::BrotliCompressCustomIoCustomDict(
                &mut brotli::enc::IoReaderWrapper(&mut &body[..]),
                &mut brotli::enc::IoWriterWrapper(&mut out),
                &mut [0u8; 4096],
                &mut [0u8; 4096],
                &params,
                brotli::enc::StandardAlloc::default(),
                &mut nop,
                dictionary,
                std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "unexpected end of input"),
            )
            .ok()?;
        }
        #[allow(unreachable_patterns)]
        _ => {
            let _ = (level, window);
            return None;
        }
    }
    Some(out)
}

/// The original of an [`encode`]d body: checks the magic and that the hash
/// is `dictionary`'s, and stops past `max_len` decoded bytes. `None` on any
/// mismatch or error.
pub fn decode(encoding: DictionaryEncoding, data: &[u8], dictionary: &[u8], max_len: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    let header = encoding.header_len();
    if data.len() < header || !data.starts_with(encoding.magic()) || data[encoding.magic().len()..header] != hash(dictionary) {
        return None;
    }
    let stream = &data[header..];
    let mut out = Vec::new();
    match encoding {
        #[cfg(feature = "zstd")]
        DictionaryEncoding::Dcz => {
            let mut decoder = zstd::stream::read::Decoder::with_ref_prefix(stream, dictionary).ok()?;
            decoder.window_log_max(23).ok()?;
            decoder.take(max_len as u64 + 1).read_to_end(&mut out).ok()?;
        }
        #[cfg(feature = "brotli")]
        DictionaryEncoding::Dcb => {
            use brotli::{Allocator, SliceWrapperMut};
            let mut alloc = brotli::enc::StandardAlloc::default();
            let mut dict = <brotli::enc::StandardAlloc as Allocator<u8>>::alloc_cell(&mut alloc, dictionary.len());
            dict.slice_mut().copy_from_slice(dictionary);
            let mut writer = LimitedWriter { out: &mut out, max: max_len + 1 };
            brotli::BrotliDecompressCustomIoCustomDict(
                &mut brotli::enc::IoReaderWrapper(&mut &stream[..]),
                &mut brotli::enc::IoWriterWrapper(&mut writer),
                &mut [0u8; 4096],
                &mut [0u8; 4096],
                brotli::enc::StandardAlloc::default(),
                brotli::enc::StandardAlloc::default(),
                brotli::enc::StandardAlloc::default(),
                dict,
                std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "unexpected end of input"),
            )
            .ok()?;
        }
        #[allow(unreachable_patterns)]
        _ => return None,
    }
    (out.len() <= max_len).then_some(out)
}

/// A writer that refuses to grow past `max` bytes.
#[cfg(feature = "brotli")]
struct LimitedWriter<'a> {
    out: &'a mut Vec<u8>,
    max: usize,
}

#[cfg(feature = "brotli")]
impl std::io::Write for LimitedWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.out.len() + buf.len() > self.max {
            return Err(std::io::Error::other("decoded body too large"));
        }
        self.out.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The headers of a body in `encoding`: `Content-Encoding`, and `Vary` on
/// `Accept-Encoding` and `Available-Dictionary`.
pub fn mark_encoded(response: &mut Response, encoding: DictionaryEncoding) {
    response.headers.push(Header { name: Header::_CONTENT_ENCODING.into(), value: encoding.token().into() });
    add_vary(response, "Accept-Encoding");
    add_vary(response, "Available-Dictionary");
}

/// Append `name` to `Vary` (or add one) unless it's already listed.
pub(crate) fn add_vary(response: &mut Response, name: &'static str) {
    if let Some(vary) = response.headers.iter_mut().find(|h| h.name.eq_ignore_ascii_case("vary")) {
        if !vary.value.split(',').any(|v| v.trim().eq_ignore_ascii_case(name) || v.trim() == "*") {
            vary.value = format!("{}, {name}", vary.value).into();
        }
    } else {
        response.headers.push(Header { name: "Vary".into(), value: name.into() });
    }
}

#[cfg(test)]
mod tests;
