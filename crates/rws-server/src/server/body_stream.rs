//! Streaming response bodies (`Response::stream_file` / `stream_pipe`) for the
//! HTTP/2 and HTTP/3 writers. Those carry raw bytes in DATA frames, so a file
//! is read in chunks and a proxied stream that arrived in HTTP/1.1 chunked
//! framing is decoded first (the HTTP/1.1 writer passes that framing through).
//! Reading happens on tokio's blocking pool; chunks cross a bounded channel,
//! so memory stays flat however large the body is.

use std::io::Read;

use crate::response::Response;

/// A blocking `Read` over chunks a task sends through a tokio channel: how a
/// body produced asynchronously (an HTTP/2 upstream, a tower service) becomes
/// a `Response::stream_pipe`. An `Err` chunk surfaces as a read error.
#[doc(hidden)]
pub struct ChannelReader {
    rx: tokio::sync::mpsc::Receiver<std::io::Result<Vec<u8>>>,
    chunk: Vec<u8>,
    pos: usize,
}

impl ChannelReader {
    #[doc(hidden)]
    pub fn new(rx: tokio::sync::mpsc::Receiver<std::io::Result<Vec<u8>>>) -> Self {
        ChannelReader { rx, chunk: Vec::new(), pos: 0 }
    }
}

impl Read for ChannelReader {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        while self.pos >= self.chunk.len() {
            match self.rx.blocking_recv() {
                Some(Ok(next)) => {
                    self.chunk = next;
                    self.pos = 0;
                }
                Some(Err(e)) => return Err(e),
                None => return Ok(0),
            }
        }
        let n = out.len().min(self.chunk.len() - self.pos);
        out[..n].copy_from_slice(&self.chunk[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// Bytes per chunk read from a file or upstream stream.
const CHUNK: usize = 64 * 1024;

/// Take the response's streaming body, if any, and start producing it.
/// Returns `None` for an ordinary (`content_range_list`) response.
#[doc(hidden)]
pub fn spawn_producer(
    response: &mut Response,
) -> Option<tokio::sync::mpsc::Receiver<Result<Vec<u8>, String>>> {
    let reader: Box<dyn Read + Send> = if let Some(pipe) = response.stream_pipe.take() {
        let chunked = response.headers.iter().any(|h| {
            h.name.eq_ignore_ascii_case("transfer-encoding") && h.value.to_ascii_lowercase().contains("chunked")
        });
        if chunked {
            // The length is unknown until the stream ends.
            response.headers.retain(|h| !h.name.eq_ignore_ascii_case("content-length"));
            Box::new(ChunkedDecoder::new(pipe))
        } else {
            pipe
        }
    } else if let Some(path) = response.stream_file.take() {
        match std::fs::File::open(&path) {
            Ok(file) => Box::new(file),
            Err(e) => {
                let (tx, rx) = tokio::sync::mpsc::channel(1);
                let _ = tx.try_send(Err(format!("unable to open {}: {}", path, e)));
                return Some(rx);
            }
        }
    } else {
        return None;
    };

    let (tx, rx) = tokio::sync::mpsc::channel(4);
    tokio::task::spawn_blocking(move || {
        let mut reader = reader;
        let mut buf = vec![0u8; CHUNK];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    // A closed receiver means the client went away: stop reading.
                    if tx.blocking_send(Ok(buf[..n].to_vec())).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let _ = tx.blocking_send(Err(e.to_string()));
                    break;
                }
            }
        }
    });
    Some(rx)
}

#[doc(hidden)]
pub use super::chunked_decoder::ChunkedDecoder;
