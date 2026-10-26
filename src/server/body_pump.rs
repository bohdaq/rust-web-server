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
    (tx, Body::stream(ChannelBody { rx: Rx::Bounded(rx), chunk: Vec::new(), pos: 0, failed: None, on_read: None }))
}

/// The sending side of [`flow_controlled_body_channel`].
pub(crate) type UnboundedBodySender = mpsc::UnboundedSender<io::Result<Vec<u8>>>;

/// For HTTP/2, where flow control bounds what the client may send: an
/// unbounded channel, and `on_read(n)` after every read so the connection
/// can give the client `n` bytes of window back — the client is never more
/// than one window ahead of the handler.
pub(crate) fn flow_controlled_body_channel(on_read: impl Fn(usize) + Send + 'static) -> (UnboundedBodySender, Body) {
    let (tx, rx) = mpsc::unbounded_channel();
    (tx, Body::stream(ChannelBody { rx: Rx::Unbounded(rx), chunk: Vec::new(), pos: 0, failed: None, on_read: Some(Box::new(on_read)) }))
}

enum Rx {
    Bounded(mpsc::Receiver<io::Result<Vec<u8>>>),
    Unbounded(mpsc::UnboundedReceiver<io::Result<Vec<u8>>>),
}

/// Blocking reader over the channel (handlers run on blocking threads).
struct ChannelBody {
    rx: Rx,
    chunk: Vec<u8>,
    pos: usize,
    /// The error that ended the stream, repeated on every later read.
    failed: Option<(io::ErrorKind, String)>,
    on_read: Option<Box<dyn Fn(usize) + Send>>,
}

impl Read for ChannelBody {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if let Some((kind, message)) = &self.failed {
            return Err(io::Error::new(*kind, message.clone()));
        }
        while self.pos >= self.chunk.len() {
            let next = match &mut self.rx {
                Rx::Bounded(rx) => rx.blocking_recv(),
                Rx::Unbounded(rx) => rx.blocking_recv(),
            };
            match next {
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
        if let Some(on_read) = &self.on_read {
            on_read(n);
        }
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
    fn a_flow_controlled_channel_reports_reads() {
        let reported = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = reported.clone();
        let (tx, body) = flow_controlled_body_channel(move |n| { counter.fetch_add(n, std::sync::atomic::Ordering::SeqCst); });
        tx.send(Ok(b"0123456789".to_vec())).unwrap();
        drop(tx);
        let mut reader = body.into_reader();
        let mut buf = [0u8; 4];
        assert_eq!(4, reader.read(&mut buf).unwrap());
        assert_eq!(4, reported.load(std::sync::atomic::Ordering::SeqCst));
        let mut rest = Vec::new();
        reader.read_to_end(&mut rest).unwrap();
        assert_eq!(10, reported.load(std::sync::atomic::Ordering::SeqCst));
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
