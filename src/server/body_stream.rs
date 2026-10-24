//! Streaming response bodies (`Response::stream_file` / `stream_pipe`) for the
//! HTTP/2 and HTTP/3 writers. Those carry raw bytes in DATA frames, so a file
//! is read in chunks and a proxied stream that arrived in HTTP/1.1 chunked
//! framing is decoded first (the HTTP/1.1 writer passes that framing through).
//! Reading happens on tokio's blocking pool; chunks cross a bounded channel,
//! so memory stays flat however large the body is.

use std::io::Read;

use crate::response::Response;

/// Bytes per chunk read from a file or upstream stream.
const CHUNK: usize = 64 * 1024;

/// Take the response's streaming body, if any, and start producing it.
/// Returns `None` for an ordinary (`content_range_list`) response.
pub(crate) fn spawn_producer(
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

/// `Read` adapter that strips HTTP/1.1 chunked transfer framing
/// (`<hex size>[;ext]\r\n<data>\r\n ... 0\r\n<trailers>\r\n`).
pub(crate) struct ChunkedDecoder<R: Read> {
    inner: R,
    remaining_in_chunk: u64,
    done: bool,
}

impl<R: Read> ChunkedDecoder<R> {
    pub(crate) fn new(inner: R) -> Self {
        ChunkedDecoder { inner, remaining_in_chunk: 0, done: false }
    }

    fn read_line(&mut self) -> std::io::Result<String> {
        let mut line = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            if self.inner.read(&mut byte)? == 0 {
                return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "truncated chunked body"));
            }
            if byte[0] == b'\n' {
                break;
            }
            if line.len() > 4096 {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "chunk-size line too long"));
            }
            line.push(byte[0]);
        }
        Ok(String::from_utf8_lossy(&line).trim().to_string())
    }
}

impl<R: Read> Read for ChunkedDecoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.done || buf.is_empty() {
            return Ok(0);
        }
        if self.remaining_in_chunk == 0 {
            let line = self.read_line()?;
            let size_hex = line.split(';').next().unwrap_or("").trim();
            let size = u64::from_str_radix(size_hex, 16).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, format!("bad chunk size {:?}", size_hex))
            })?;
            if size == 0 {
                // Last chunk: consume trailers up to the blank line.
                while !self.read_line()?.is_empty() {}
                self.done = true;
                return Ok(0);
            }
            self.remaining_in_chunk = size;
        }
        let want = buf.len().min(self.remaining_in_chunk.min(usize::MAX as u64) as usize);
        let n = self.inner.read(&mut buf[..want])?;
        if n == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "truncated chunk"));
        }
        self.remaining_in_chunk -= n as u64;
        if self.remaining_in_chunk == 0 {
            self.read_line()?; // the CRLF after the chunk data
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::ChunkedDecoder;
    use std::io::Read;

    fn decode(raw: &[u8]) -> std::io::Result<Vec<u8>> {
        let mut out = Vec::new();
        ChunkedDecoder::new(raw).read_to_end(&mut out)?;
        Ok(out)
    }

    #[test]
    fn decodes_chunks_extensions_and_trailers() {
        let raw = b"5;ext=1\r\nhello\r\n7\r\n, world\r\n0\r\nX-Trailer: v\r\n\r\n";
        assert_eq!(b"hello, world".to_vec(), decode(raw).unwrap());
    }

    #[test]
    fn rejects_malformed_or_truncated_framing() {
        assert!(decode(b"zz\r\nhello\r\n0\r\n\r\n").is_err(), "non-hex size");
        assert!(decode(b"10\r\nshort").is_err(), "truncated chunk");
        assert!(decode(b"5\r\nhello\r\n").is_err(), "no terminal chunk");
    }
}
