//! Streaming a request body to a handler that opted in with
//! [`Application::streams_body`](crate::application::Application::streams_body)
//! (#31): the connection task sends the body's chunks into a bounded channel
//! as they arrive; the handler, on a blocking-pool thread, reads them from
//! the request's [`Body::Stream`](crate::request::Body::Stream).
//!
//! A failure on the connection side (the client went away, stalled past the
//! read timeout, sent more than the size limit or broken chunked framing)
//! reaches the handler as a read error, not a body that merely ends early.

use std::io::{self, Read};

use tokio::sync::mpsc;

use crate::request::Body;

/// Chunks buffered between the connection and the handler: the most a slow
/// handler holds in memory, besides the chunk it is reading.
pub(crate) const BODY_CHANNEL_CHUNKS: usize = 4;

/// The sending side: each item a chunk of body, or the error that ended it.
pub(crate) type BodySender = mpsc::Sender<io::Result<Vec<u8>>>;

/// A channel whose receiving end is a streamed [`Body`].
pub(crate) fn body_channel() -> (BodySender, Body) {
    let (tx, rx) = mpsc::channel(BODY_CHANNEL_CHUNKS);
    (tx, Body::stream(ChannelBody { rx, chunk: Vec::new(), pos: 0, failed: None }))
}

/// Blocking reader over the channel (handlers run on blocking threads).
struct ChannelBody {
    rx: mpsc::Receiver<io::Result<Vec<u8>>>,
    chunk: Vec<u8>,
    pos: usize,
    /// The error that ended the stream, repeated on every later read.
    failed: Option<(io::ErrorKind, String)>,
}

impl Read for ChannelBody {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if let Some((kind, message)) = &self.failed {
            return Err(io::Error::new(*kind, message.clone()));
        }
        while self.pos >= self.chunk.len() {
            match self.rx.blocking_recv() {
                Some(Ok(next)) => {
                    self.chunk = next;
                    self.pos = 0;
                }
                Some(Err(e)) => {
                    self.failed = Some((e.kind(), e.to_string()));
                    return Err(e);
                }
                None => return Ok(0),
            }
        }
        let n = out.len().min(self.chunk.len() - self.pos);
        out[..n].copy_from_slice(&self.chunk[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// How the body pump ended.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PumpOutcome {
    /// Every byte of the body was handed over: the connection can be reused.
    Complete,
    /// The handler stopped reading (dropped the body) before the end.
    Abandoned,
    /// The client side failed; the handler got the error.
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_then_end() {
        let (tx, body) = body_channel();
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        rt.block_on(async {
            tx.send(Ok(b"hello ".to_vec())).await.unwrap();
            tx.send(Ok(b"world".to_vec())).await.unwrap();
        });
        drop(tx);
        let mut out = String::new();
        body.into_reader().read_to_string(&mut out).unwrap();
        assert_eq!("hello world", out);
    }

    #[test]
    fn an_error_reaches_the_reader_and_sticks() {
        let (tx, body) = body_channel();
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        rt.block_on(async {
            tx.send(Ok(b"part".to_vec())).await.unwrap();
            tx.send(Err(io::Error::new(io::ErrorKind::UnexpectedEof, "client went away"))).await.unwrap();
        });
        let mut reader = body.into_reader();
        let mut buf = [0u8; 16];
        assert_eq!(4, reader.read(&mut buf).unwrap());
        assert_eq!(io::ErrorKind::UnexpectedEof, reader.read(&mut buf).unwrap_err().kind());
        assert_eq!(io::ErrorKind::UnexpectedEof, reader.read(&mut buf).unwrap_err().kind());
    }
}
