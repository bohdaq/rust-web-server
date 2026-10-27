//! gRPC-web to gRPC: [`GrpcWebLayer`] lets browsers (which can't read HTTP
//! trailers or send HTTP/2 frames of their own) call gRPC services, as
//! Envoy's `grpc_web` filter does.
//!
//! A request with `Content-Type: application/grpc-web[+proto]` or
//! `application/grpc-web-text[+proto]` (base64) is passed on as
//! `application/grpc[+proto]` with `te: trailers` and its body decoded; the
//! response comes back as gRPC-web, the gRPC status and other trailers
//! appended to the body as the final frame (flag `0x80`), base64-encoded for
//! `-text`. Bodies stream both ways. Other requests pass through untouched.

use std::io::Read;
use std::sync::{Arc, Mutex};

use crate::application::Application;
use crate::core::base64::Base64;
use crate::header::Header;
use crate::middleware::Middleware;
use crate::request::{Body, Request};
use crate::response::{Response, Trailers};
use crate::server::ConnectionInfo;

/// Translates gRPC-web requests to gRPC for the layers after it — put it in
/// front of a [`GrpcProxy`](super::GrpcProxy):
///
/// ```rust,no_run
/// use rust_web_server::app::App;
/// use rust_web_server::core::New;
/// use rust_web_server::proxy::{GrpcProxy, GrpcWebLayer};
///
/// let app = App::new()
///     .wrap(GrpcWebLayer::new())
///     .wrap(GrpcProxy::new(["grpc-service:50051"]));
/// ```
#[derive(Default)]
pub struct GrpcWebLayer;

impl GrpcWebLayer {
    pub fn new() -> Self {
        GrpcWebLayer
    }
}

/// What a gRPC-web request asked for: base64 (`-text`) or not, and the
/// message format suffix (`+proto`, `+json` or none).
struct Variant {
    text: bool,
    suffix: String,
}

fn variant(request: &Request) -> Option<Variant> {
    let content_type = request.header(Header::_CONTENT_TYPE)?.value.as_ref().trim().to_ascii_lowercase();
    if let Some(suffix) = content_type.strip_prefix("application/grpc-web-text") {
        return Some(Variant { text: true, suffix: suffix.to_string() });
    }
    content_type.strip_prefix("application/grpc-web").map(|suffix| Variant { text: false, suffix: suffix.to_string() })
}

impl Middleware for GrpcWebLayer {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        let Some(variant) = variant(request) else {
            return next.execute(request, connection);
        };
        let mut grpc = request.clone();
        grpc.headers.retain(|h| {
            !(h.name.eq_ignore_ascii_case("content-type") || h.name.eq_ignore_ascii_case("te")
                || (variant.text && h.name.eq_ignore_ascii_case("content-length")))
        });
        grpc.headers.push(Header::new(Header::_CONTENT_TYPE, format!("application/grpc{}", variant.suffix)));
        grpc.headers.push(Header::new("te", "trailers"));
        if variant.text {
            grpc.body = match std::mem::take(&mut grpc.body) {
                Body::Bytes(bytes) => Body::Bytes(decode_text(&bytes).map_err(|e| format!("grpc-web-text request body: {e}"))?),
                stream => Body::Stream(Arc::new(Mutex::new(Box::new(Base64Decoder::new(stream.into_reader()))))),
            };
        }
        let response = next.execute(&grpc, connection)?;
        Ok(to_grpc_web(response, variant.text))
    }

    /// The body passes through (decoded on the fly for `-text`).
    fn accepts_streamed_body(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }
}

/// The gRPC response as gRPC-web: the content type renamed, the trailers as
/// the body's last frame.
fn to_grpc_web(mut response: Response, text: bool) -> Response {
    let web_type = |grpc: &str| {
        let suffix = grpc.trim().strip_prefix("application/grpc").unwrap_or("");
        format!("application/grpc-web{}{suffix}", if text { "-text" } else { "" })
    };
    let mut is_grpc = false;
    for header in response.headers.iter_mut() {
        if header.name.eq_ignore_ascii_case("content-type") && header.value.starts_with("application/grpc") {
            header.value = web_type(&header.value).into();
            is_grpc = true;
        }
    }
    for range in response.content_range_list.iter_mut() {
        if range.content_type.starts_with("application/grpc") {
            range.content_type = web_type(&range.content_type);
            is_grpc = true;
        }
    }
    if !is_grpc {
        return response; // an error page from rws or the upstream: leave it
    }
    let trailers = response.trailers.take();
    if let Some(body) = response.stream_pipe.take() {
        let framed: Box<dyn Read + Send> = Box::new(TrailerFrame { body, trailers, frame: None });
        response.stream_pipe = Some(if text { Box::new(Base64Encoder::new(framed)) } else { framed });
    } else {
        // Buffered (a small response): one body with the frame appended.
        let mut body = Response::generate_body(std::mem::take(&mut response.content_range_list));
        if let Some(trailers) = &trailers {
            body.extend_from_slice(&trailer_frame(&trailers.get()));
        }
        if text {
            body = Base64::encode(&body).unwrap_or_default().into_bytes();
        }
        let content_type = response
            .headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case("content-type"))
            .map_or_else(|| web_type("application/grpc"), |h| h.value.to_string());
        response.headers.retain(|h| !h.name.eq_ignore_ascii_case("content-type"));
        if !body.is_empty() {
            response.content_range_list = vec![crate::range::Range::get_content_range(body, content_type)];
        } else {
            response.headers.push(Header::new(Header::_CONTENT_TYPE, content_type));
        }
    }
    response
}

/// The gRPC-web trailer frame: flag `0x80`, a 4-byte length, then
/// `name:value\r\n` lines with lower-case names. Empty without trailers.
pub(crate) fn trailer_frame(trailers: &[Header]) -> Vec<u8> {
    if trailers.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    for header in trailers {
        lines.extend(header.name.bytes().map(|b| b.to_ascii_lowercase()));
        lines.push(b':');
        lines.extend_from_slice(header.value.trim().as_bytes());
        lines.extend_from_slice(b"\r\n");
    }
    let mut frame = Vec::with_capacity(5 + lines.len());
    frame.push(0x80);
    frame.extend_from_slice(&(lines.len() as u32).to_be_bytes());
    frame.extend_from_slice(&lines);
    frame
}

/// The response body, then the trailer frame once the body has ended (and
/// with it the trailers are known).
struct TrailerFrame {
    body: Box<dyn Read + Send>,
    trailers: Option<Trailers>,
    frame: Option<std::io::Cursor<Vec<u8>>>,
}

impl Read for TrailerFrame {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.frame.is_none() {
            let n = self.body.read(out)?;
            if n > 0 {
                return Ok(n);
            }
            let fields = self.trailers.as_ref().map(|t| t.get()).unwrap_or_default();
            self.frame = Some(std::io::Cursor::new(trailer_frame(&fields)));
        }
        self.frame.as_mut().map_or(Ok(0), |frame| frame.read(out))
    }
}

/// Base64-encodes a stream: whole 3-byte groups as they arrive, the rest
/// (padded) at the end.
struct Base64Encoder {
    inner: Box<dyn Read + Send>,
    raw: Vec<u8>,
    encoded: std::io::Cursor<Vec<u8>>,
    ended: bool,
}

impl Base64Encoder {
    fn new(inner: Box<dyn Read + Send>) -> Self {
        Base64Encoder { inner, raw: Vec::new(), encoded: std::io::Cursor::new(Vec::new()), ended: false }
    }
}

impl Read for Base64Encoder {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        loop {
            let n = self.encoded.read(out)?;
            if n > 0 || self.ended {
                return Ok(n);
            }
            let mut chunk = vec![0u8; 16 * 1024];
            let read = self.inner.read(&mut chunk)?;
            self.raw.extend_from_slice(&chunk[..read]);
            let take = if read == 0 {
                self.ended = true;
                self.raw.len()
            } else {
                self.raw.len() - self.raw.len() % 3
            };
            let group: Vec<u8> = self.raw.drain(..take).collect();
            let text = Base64::encode(&group).map_err(std::io::Error::other)?;
            self.encoded = std::io::Cursor::new(text.into_bytes());
        }
    }
}

/// Decodes a base64 request body as it arrives. A body may be several
/// padded segments back to back (a client encoding each message on its own).
struct Base64Decoder {
    inner: Box<dyn Read + Send>,
    text: Vec<u8>,
    decoded: std::io::Cursor<Vec<u8>>,
    ended: bool,
}

impl Base64Decoder {
    fn new(inner: Box<dyn Read + Send>) -> Self {
        Base64Decoder { inner, text: Vec::new(), decoded: std::io::Cursor::new(Vec::new()), ended: false }
    }
}

impl Read for Base64Decoder {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        loop {
            let n = self.decoded.read(out)?;
            if n > 0 || self.ended {
                return Ok(n);
            }
            let mut chunk = vec![0u8; 16 * 1024];
            let read = self.inner.read(&mut chunk)?;
            self.text.extend(chunk[..read].iter().filter(|b| !b.is_ascii_whitespace()));
            let take = if read == 0 {
                self.ended = true;
                if self.text.len() % 4 != 0 {
                    return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "grpc-web-text body isn't whole base64"));
                }
                self.text.len()
            } else {
                self.text.len() - self.text.len() % 4
            };
            let groups: Vec<u8> = self.text.drain(..take).collect();
            let bytes = decode_text(&groups).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            self.decoded = std::io::Cursor::new(bytes);
        }
    }
}

/// Decode base64 that may hold several padded segments.
fn decode_text(text: &[u8]) -> Result<Vec<u8>, String> {
    let text: Vec<u8> = text.iter().copied().filter(|b| !b.is_ascii_whitespace()).collect();
    if text.len() % 4 != 0 {
        return Err("not whole base64".to_string());
    }
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut start = 0;
    for (i, group) in text.chunks(4).enumerate() {
        // A padded group ends a segment.
        if group.contains(&b'=') {
            let end = (i + 1) * 4;
            let segment = String::from_utf8(text[start..end].to_vec()).map_err(|e| e.to_string())?;
            out.extend(Base64::decode(segment)?);
            start = end;
        }
    }
    if start < text.len() {
        let segment = String::from_utf8(text[start..].to_vec()).map_err(|e| e.to_string())?;
        out.extend(Base64::decode(segment)?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
