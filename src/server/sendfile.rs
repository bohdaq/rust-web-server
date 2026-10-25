//! Zero-copy file responses over plain TCP: the kernel copies file pages
//! straight to the socket (`sendfile(2)`), as nginx does. The general path —
//! read 64 KiB on the blocking pool, copy it into a channel, write it from
//! the event loop — copies every byte twice in user space.
//!
//! Used by `http1_async::serve` for `Response::stream_file` bodies with a
//! known `Content-Length` on Linux and macOS. TLS connections can't use it
//! (the bytes must be encrypted in user space) and keep the general path.

use std::fs::File;
use std::io;

/// The connection types `http1_async::serve` handles: plain TCP exposes its
/// socket for `sendfile`, everything else (TLS, test streams) doesn't.
pub(crate) trait SendfileSocket {
    fn sendfile_socket(&self) -> Option<&tokio::net::TcpStream> {
        None
    }
}

impl SendfileSocket for tokio::net::TcpStream {
    fn sendfile_socket(&self) -> Option<&tokio::net::TcpStream> {
        Some(self)
    }
}

impl SendfileSocket for tokio_rustls::server::TlsStream<tokio::net::TcpStream> {}

impl SendfileSocket for tokio::io::DuplexStream {}

/// `true` where [`send_file`] is implemented.
pub(crate) const SUPPORTED: bool = cfg!(any(target_os = "linux", target_os = "macos"));

/// Send `head` followed by `len` bytes of `file` from offset 0 to `socket`,
/// waiting for the socket to become writable between calls. The head shares
/// a TCP segment with the start of the file rather than going out alone:
/// on Linux with the head sent `MSG_MORE` (like `TCP_CORK`, nginx's `tcp_nopush`), on macOS as
/// `sendfile`'s header vector. Fails if the file is shorter than `len` (it
/// shrank after `Content-Length` was sent).
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) async fn send_file(socket: &tokio::net::TcpStream, head: &[u8], file: &File, len: u64) -> io::Result<()> {
    use std::os::unix::io::AsRawFd;
    let (file_fd, socket_fd) = (file.as_raw_fd(), socket.as_raw_fd());

    // The head goes out with MSG_MORE: the kernel holds it until the file
    // bytes join it (what TCP_CORK did, without two setsockopt calls per
    // response). An empty body sends the head as is.
    #[cfg(target_os = "linux")]
    let mut head_left: &[u8] = {
        let more = len > 0;
        let mut rest = head;
        while !rest.is_empty() {
            socket.writable().await?;
            match socket.try_io(tokio::io::Interest::WRITABLE, || send_head(socket_fd, rest, more)) {
                Ok(n) => rest = &rest[n..],
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e),
            }
        }
        rest
    };
    #[cfg(target_os = "macos")]
    let mut head_left: &[u8] = head;

    let mut offset: u64 = 0;
    while offset < len || !head_left.is_empty() {
        socket.writable().await?;
        let remaining = len - offset;
        let header = head_left;
        match socket.try_io(tokio::io::Interest::WRITABLE, || sendfile_once(file_fd, socket_fd, offset, remaining, header)) {
            Ok(0) if header.is_empty() => {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "file shorter than its Content-Length"))
            }
            Ok(sent) => {
                // Header bytes count first (macOS reports them in the total).
                let from_header = sent.min(head_left.len());
                head_left = &head_left[from_header..];
                offset += (sent - from_header) as u64;
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(crate) async fn send_file(_socket: &tokio::net::TcpStream, _head: &[u8], _file: &File, _len: u64) -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "sendfile is not available on this platform"))
}

/// `send(2)` of (part of) the response head, with `MSG_MORE` when file
/// bytes follow.
#[cfg(target_os = "linux")]
fn send_head(fd: i32, head: &[u8], more: bool) -> io::Result<usize> {
    let flags = libc::MSG_NOSIGNAL | if more { libc::MSG_MORE } else { 0 };
    // SAFETY: a valid socket descriptor and a live buffer of `head.len()` bytes.
    let sent = unsafe { libc::send(fd, head.as_ptr() as *const libc::c_void, head.len(), flags) };
    if sent < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(sent as usize)
    }
}

/// Largest single `sendfile` call; the loop sends the rest.
#[cfg(any(target_os = "linux", target_os = "macos"))]
const MAX_CHUNK: u64 = 1 << 30;

/// One `sendfile` call: bytes sent, or `WouldBlock` when the socket buffer
/// is full and nothing went out.
#[cfg(target_os = "linux")]
fn sendfile_once(file_fd: i32, socket_fd: i32, offset: u64, count: u64, _header: &[u8]) -> io::Result<usize> {
    if count == 0 {
        return Ok(0);
    }
    let mut off = offset as libc::off_t;
    // SAFETY: both descriptors are open for the duration of the call, and
    // `off` is a valid pointer to a local.
    let sent = unsafe { libc::sendfile(socket_fd, file_fd, &mut off, count.min(MAX_CHUNK) as usize) };
    if sent < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(sent as usize)
    }
}

/// With a non-empty `header`, it is sent first, in the same call; the
/// returned count includes the header bytes that went out.
#[cfg(target_os = "macos")]
fn sendfile_once(file_fd: i32, socket_fd: i32, offset: u64, count: u64, header: &[u8]) -> io::Result<usize> {
    let mut len = count.min(MAX_CHUNK) as libc::off_t;
    let mut iov = libc::iovec { iov_base: header.as_ptr() as *mut libc::c_void, iov_len: header.len() };
    let mut hdtr = libc::sf_hdtr { headers: &mut iov, hdr_cnt: 1, trailers: std::ptr::null_mut(), trl_cnt: 0 };
    let hdtr_ptr: *mut libc::sf_hdtr = if header.is_empty() { std::ptr::null_mut() } else { &mut hdtr };
    // SAFETY: both descriptors are open for the duration of the call; `len`,
    // `iov` and `hdtr` are locals that outlive it, and `iov` points at
    // `header`, which the caller keeps alive. sendfile only reads the header.
    let rc = unsafe { libc::sendfile(file_fd, socket_fd, offset as libc::off_t, &mut len, hdtr_ptr, 0) };
    if rc == -1 {
        let err = io::Error::last_os_error();
        // A partial send on a non-blocking socket reports EAGAIN with `len`
        // set to what did go out.
        if len > 0 && matches!(err.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) {
            return Ok(len as usize);
        }
        return Err(err);
    }
    Ok(len as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn sends_a_whole_file_over_tcp() {
        if !SUPPORTED {
            return;
        }
        let path = crate::test_env::TempPath::new("sendfile_src.bin");
        let content: Vec<u8> = (0..3 * 1024 * 1024 + 17).map(|i| (i % 253) as u8).collect();
        std::fs::write(path.as_str(), &content).unwrap();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let reader = tokio::spawn(async move {
            let mut client = tokio::net::TcpStream::connect(addr).await.unwrap();
            let mut received = Vec::new();
            client.read_to_end(&mut received).await.unwrap();
            received
        });
        let (server, _) = listener.accept().await.unwrap();
        let file = File::open(path.as_str()).unwrap();
        send_file(&server, b"HEAD\r\n\r\n", &file, content.len() as u64).await.unwrap();
        drop(server);
        let received = reader.await.unwrap();
        assert!(received.starts_with(b"HEAD\r\n\r\n"), "head first");
        assert!(received[8..] == content[..], "received bytes differ from the file");
    }

    /// An empty body: the head must leave at once, not wait for file bytes
    /// that never come (MSG_MORE would hold it for the kernel's 200 ms).
    #[tokio::test]
    async fn an_empty_file_sends_its_head_at_once() {
        if !SUPPORTED {
            return;
        }
        let path = crate::test_env::TempPath::new("sendfile_empty.bin");
        std::fs::write(path.as_str(), b"").unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut client = tokio::net::TcpStream::connect(addr).await.unwrap();
        let (server, _) = listener.accept().await.unwrap();
        let file = File::open(path.as_str()).unwrap();
        send_file(&server, b"HEAD\r\n\r\n", &file, 0).await.unwrap();
        // The connection stays open: only an immediate send arrives in time.
        let mut head = [0u8; 8];
        tokio::time::timeout(std::time::Duration::from_millis(100), client.read_exact(&mut head)).await.unwrap().unwrap();
        assert_eq!(b"HEAD\r\n\r\n", &head);
        drop(server);
    }

    #[tokio::test]
    async fn a_file_shorter_than_promised_is_an_error() {
        if !SUPPORTED {
            return;
        }
        let path = crate::test_env::TempPath::new("sendfile_short.bin");
        std::fs::write(path.as_str(), b"short").unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let _client = tokio::net::TcpStream::connect(addr).await.unwrap();
        let (server, _) = listener.accept().await.unwrap();
        let file = File::open(path.as_str()).unwrap();
        let err = send_file(&server, b"", &file, 100).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn only_plain_tcp_offers_its_socket() {
        fn offers<S: SendfileSocket>(stream: &S) -> bool {
            stream.sendfile_socket().is_some()
        }
        let (a, _b) = tokio::io::duplex(8);
        assert!(!offers(&a));
    }
}
