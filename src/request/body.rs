//! The request body: bytes already read, or a stream the handler reads (#31).
//!
//! The server reads a request's body into memory before the handler runs,
//! as it always has — a [`Body::Bytes`]. An application that wants to read a
//! large upload as it arrives opts in per request
//! ([`Application::streams_body`](crate::application::Application::streams_body))
//! and gets a [`Body::Stream`] instead: the handler runs while the server is
//! still receiving, and memory stays at a few chunks however big the body is.
//!
//! `Body` dereferences to `&[u8]`, so code that reads `request.body` as bytes
//! keeps working; a streamed body derefs to an empty slice until
//! [`Body::body_bytes`] reads it in.

use std::fmt;
use std::io::{self, Read};
use std::sync::{Arc, Mutex};

/// A request body.
pub enum Body {
    /// The whole body, already read.
    Bytes(Vec<u8>),
    /// A body still arriving; read it with [`Read`] (`(&mut body).read(..)`,
    /// `std::io::copy`), [`Body::into_reader`] or [`Body::body_bytes`].
    /// Clones share the one stream: what one reads, the others don't see.
    Stream(Arc<Mutex<Box<dyn Read + Send>>>),
}

impl Body {
    /// An empty body.
    pub fn empty() -> Self {
        Body::Bytes(Vec::new())
    }

    /// A body streamed from `reader`.
    pub fn stream(reader: impl Read + Send + 'static) -> Self {
        Body::Stream(Arc::new(Mutex::new(Box::new(reader))))
    }

    /// `true` for a body still arriving.
    pub fn is_stream(&self) -> bool {
        matches!(self, Body::Stream(_))
    }

    /// The bytes read so far: the whole body, or empty for a stream.
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Body::Bytes(bytes) => bytes,
            Body::Stream(_) => &[],
        }
    }

    /// The whole body: a stream is read to the end first (and stays read —
    /// the body is `Bytes` afterwards). Fails if reading the stream fails,
    /// e.g. the client went away or the body passed the size limit.
    pub fn body_bytes(&mut self) -> io::Result<&[u8]> {
        if let Body::Stream(stream) = self {
            let mut bytes = Vec::new();
            stream.lock().map_err(|_| io::Error::other("body stream poisoned"))?.read_to_end(&mut bytes)?;
            *self = Body::Bytes(bytes);
        }
        Ok(self.as_bytes())
    }

    /// The body as one reader, whatever it is.
    pub fn into_reader(self) -> Box<dyn Read + Send> {
        match self {
            Body::Bytes(bytes) => Box::new(io::Cursor::new(bytes)),
            Body::Stream(stream) => Box::new(SharedReader(stream)),
        }
    }

    /// The whole body as a `Vec<u8>` (a stream is read to the end).
    pub fn into_vec(mut self) -> io::Result<Vec<u8>> {
        self.body_bytes()?;
        match self {
            Body::Bytes(bytes) => Ok(bytes),
            Body::Stream(_) => unreachable!("body_bytes turns a stream into bytes"),
        }
    }

    /// Mutable access to the bytes (a stream is read in first).
    pub fn bytes_mut(&mut self) -> io::Result<&mut Vec<u8>> {
        self.body_bytes()?;
        match self {
            Body::Bytes(bytes) => Ok(bytes),
            Body::Stream(_) => unreachable!("body_bytes turns a stream into bytes"),
        }
    }
}

struct SharedReader(Arc<Mutex<Box<dyn Read + Send>>>);

impl Read for SharedReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.lock().map_err(|_| io::Error::other("body stream poisoned"))?.read(buf)
    }
}

/// Reading a `Bytes` body yields nothing (its bytes are in the slice);
/// reading a stream yields the next bytes received.
impl Read for &Body {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Body::Bytes(_) => Ok(0),
            Body::Stream(stream) => stream.lock().map_err(|_| io::Error::other("body stream poisoned"))?.read(buf),
        }
    }
}

impl Default for Body {
    fn default() -> Self {
        Body::empty()
    }
}

impl std::ops::Deref for Body {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl AsRef<[u8]> for Body {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl Clone for Body {
    fn clone(&self) -> Self {
        match self {
            Body::Bytes(bytes) => Body::Bytes(bytes.clone()),
            Body::Stream(stream) => Body::Stream(Arc::clone(stream)),
        }
    }
}

/// Bytes compare by content; a stream equals only its own clones.
impl PartialEq for Body {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Body::Bytes(a), Body::Bytes(b)) => a == b,
            (Body::Stream(a), Body::Stream(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl Eq for Body {}

impl PartialEq<Vec<u8>> for Body {
    fn eq(&self, other: &Vec<u8>) -> bool {
        matches!(self, Body::Bytes(bytes) if bytes == other)
    }
}

impl PartialEq<[u8]> for Body {
    fn eq(&self, other: &[u8]) -> bool {
        matches!(self, Body::Bytes(bytes) if bytes.as_slice() == other)
    }
}

impl PartialEq<Body> for Vec<u8> {
    fn eq(&self, other: &Body) -> bool {
        other == self
    }
}

impl PartialEq<Body> for &[u8] {
    fn eq(&self, other: &Body) -> bool {
        other == *self
    }
}

impl PartialEq<Body> for [u8] {
    fn eq(&self, other: &Body) -> bool {
        other == self
    }
}

impl fmt::Debug for Body {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Body::Bytes(bytes) => f.debug_tuple("Bytes").field(&bytes.len()).finish(),
            Body::Stream(_) => f.write_str("Stream"),
        }
    }
}

impl From<Vec<u8>> for Body {
    fn from(bytes: Vec<u8>) -> Self {
        Body::Bytes(bytes)
    }
}

impl From<&[u8]> for Body {
    fn from(bytes: &[u8]) -> Self {
        Body::Bytes(bytes.to_vec())
    }
}

impl<const N: usize> From<&[u8; N]> for Body {
    fn from(bytes: &[u8; N]) -> Self {
        Body::Bytes(bytes.to_vec())
    }
}

impl From<String> for Body {
    fn from(text: String) -> Self {
        Body::Bytes(text.into_bytes())
    }
}

impl From<&str> for Body {
    fn from(text: &str) -> Self {
        Body::Bytes(text.as_bytes().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::Body;
    use std::io::Read;

    #[test]
    fn bytes_deref_compare_and_convert() {
        let body: Body = b"abc".to_vec().into();
        assert_eq!(3, body.len());
        assert_eq!(b"abc", &body[..]);
        assert_eq!(b"abc".to_vec(), body);
        assert!(body == b"abc".to_vec());
        assert_eq!(Body::from("abc"), Body::from(&b"abc"[..]));
        assert!(!body.is_stream());
        assert_eq!(b"abc".to_vec(), body.clone().into_vec().unwrap());
        assert!(Body::default().is_empty());
    }

    #[test]
    fn a_stream_reads_once_and_becomes_bytes() {
        let mut body = Body::stream(std::io::Cursor::new(b"streamed".to_vec()));
        assert!(body.is_stream());
        assert!(body.is_empty(), "a stream derefs to nothing until read");
        assert_eq!(b"streamed", body.body_bytes().unwrap());
        assert!(!body.is_stream());
        assert_eq!(b"streamed", &body[..]);
    }

    #[test]
    fn clones_of_a_stream_share_it() {
        let body = Body::stream(std::io::Cursor::new(b"0123456789".to_vec()));
        let other = body.clone();
        assert_eq!(body, other);
        let mut first = [0u8; 4];
        (&body).read_exact(&mut first).unwrap();
        let mut rest = Vec::new();
        other.into_reader().read_to_end(&mut rest).unwrap();
        assert_eq!(b"0123", &first);
        assert_eq!(b"456789".to_vec(), rest);
        assert_ne!(Body::stream(std::io::empty()), Body::stream(std::io::empty()));
    }

    #[test]
    fn a_failing_stream_is_an_error() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("client went away"))
            }
        }
        let mut body = Body::stream(Broken);
        assert!(body.body_bytes().is_err());
    }
}
