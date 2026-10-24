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

/// Send `len` bytes of `file` from offset 0 to `socket`, waiting for the
/// socket to become writable between calls. Fails if the file is shorter
/// than `len` (it shrank after `Content-Length` was sent).
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) async fn send_file(socket: &tokio::net::TcpStream, file: &File, len: u64) -> io::Result<()> {
    use std::os::unix::io::AsRawFd;
    let (file_fd, socket_fd) = (file.as_raw_fd(), socket.as_raw_fd());
    let mut offset: u64 = 0;
    while offset < len {
        socket.writable().await?;
        let remaining = len - offset;
        match socket.try_io(tokio::io::Interest::WRITABLE, || sendfile_once(file_fd, socket_fd, offset, remaining)) {
            Ok(0) => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "file shorter than its Content-Length")),
            Ok(sent) => offset += sent as u64,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(crate) async fn send_file(_socket: &tokio::net::TcpStream, _file: &File, _len: u64) -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "sendfile is not available on this platform"))
}

/// Largest single `sendfile` call; the loop sends the rest.
#[cfg(any(target_os = "linux", target_os = "macos"))]
const MAX_CHUNK: u64 = 1 << 30;

/// One `sendfile` call: bytes sent, or `WouldBlock` when the socket buffer
/// is full and nothing went out.
#[cfg(target_os = "linux")]
fn sendfile_once(file_fd: i32, socket_fd: i32, offset: u64, count: u64) -> io::Result<usize> {
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

#[cfg(target_os = "macos")]
fn sendfile_once(file_fd: i32, socket_fd: i32, offset: u64, count: u64) -> io::Result<usize> {
    let mut len = count.min(MAX_CHUNK) as libc::off_t;
    // SAFETY: both descriptors are open for the duration of the call, `len`
    // is a valid pointer to a local, and no header/trailer vector is passed.
    let rc = unsafe { libc::sendfile(file_fd, socket_fd, offset as libc::off_t, &mut len, std::ptr::null_mut(), 0) };
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
        send_file(&server, &file, content.len() as u64).await.unwrap();
        drop(server);
        assert!(reader.await.unwrap() == content, "received bytes differ from the file");
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
        let err = send_file(&server, &file, 100).await.unwrap_err();
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
