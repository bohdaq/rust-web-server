//! Kernel TLS (kTLS, Linux): after rustls finishes the handshake, the session
//! keys go to the kernel (`TCP_ULP "tls"`) and the kernel encrypts and
//! decrypts the records. The connection then reads and writes plaintext on
//! the TCP socket, so static files go out with `sendfile(2)` over HTTPS as
//! they do over plain HTTP, without passing through user space.
//!
//! Opt-in with `tls_ktls = true` (`RWS_CONFIG_TLS_KTLS`); off by default and
//! never in `fips` builds (the kernel's AEAD isn't the validated module).
//! Whether it saves CPU depends on the kernel's AES-GCM against aws-lc's: on
//! Linux 6.8 with an AES-NI/AVX2 CPU (no VAES) it doesn't — the kernel's
//! encryption costs what the saved copies do (`bench/ktls.sh`,
//! `spec/KTLS.md`). NIC TLS offload, where the kernel hands records to the
//! card, is what kTLS makes possible beyond that.
//!
//! How a connection gets there:
//!
//! 1. The handshake runs over [`Corked`], which never reads past the end of
//!    the TLS record being read. When the handshake ends, rustls therefore
//!    holds only whole records: none half-read that the kernel would need.
//! 2. [`upgrade`] checks the negotiated cipher against what the kernel
//!    supports ([`supported_ciphers`], probed once) and that rustls has
//!    nothing left to send, takes any application data rustls already
//!    decrypted (a TLS 1.3 client's first request travels with its Finished,
//!    and tokio-rustls reads on while rustls wants data), attaches the ULP,
//!    extracts the secrets and installs both directions. rustls' read
//!    sequence number already counts the records it decrypted, so the kernel
//!    continues after them. Anything unsupported keeps the rustls stream
//!    ([`Upgraded::Kept`]); counted in `rws_tls_ktls_connections_total`.
//! 3. [`KtlsStream`] returns that plaintext first, then reads with `recvmsg`
//!    and the record-type control message: application data is returned,
//!    `close_notify` is end of stream, any other alert or a post-handshake
//!    message (a TLS 1.3 KeyUpdate: rustls' buffered connection can't hand
//!    over the traffic secrets needed to follow it) ends the connection.

pub(crate) mod sys;
#[cfg(test)]
mod tests;

use std::io;
use std::os::unix::io::AsRawFd;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::task::{ready, Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, Interest, ReadBuf};
use tokio::net::TcpStream;

pub use sys::Cipher;

/// Whether kTLS is configured (`RWS_CONFIG_TLS_KTLS` = `true`). Read
/// once; a change needs a restart. Always `false` in `fips` builds.
pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        let on = std::env::var("RWS_CONFIG_TLS_KTLS").is_ok_and(|v| parse_setting(&v));
        if on && cfg!(feature = "fips") {
            eprintln!("kTLS: tls_ktls is ignored in FIPS builds (the kernel's AEAD is not the validated module)");
            return false;
        }
        if on && supported_ciphers().is_empty() {
            eprintln!("kTLS: tls_ktls is on but the kernel has no usable TLS support (is the `tls` module available?); serving TLS with rustls");
            return false;
        }
        on
    })
}

/// `true` (any case) turns kTLS on; anything else is off.
pub fn parse_setting(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case("true")
}

/// The AEADs this kernel accepts for both directions, found once by
/// installing dummy keys on loopback sockets.
pub fn supported_ciphers() -> &'static [Cipher] {
    static SUPPORTED: OnceLock<Vec<Cipher>> = OnceLock::new();
    SUPPORTED.get_or_init(|| Cipher::ALL.into_iter().filter(|c| probe(*c).is_ok()).collect())
}

fn probe(cipher: Cipher) -> io::Result<()> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let client = std::net::TcpStream::connect(listener.local_addr()?)?;
    let (_server, _) = listener.accept()?;
    let fd = client.as_raw_fd();
    sys::set_ulp(fd)?;
    let key_len = match cipher {
        Cipher::AesGcm128 => 16,
        _ => 32,
    };
    let key = [0u8; 32];
    let iv = [0u8; 12];
    for direction in [sys::TLS_TX, sys::TLS_RX] {
        let state = sys::CryptoState::new(sys::TLS_1_3_VERSION, cipher, &key[..key_len], &iv, 0)
            .ok_or_else(|| io::Error::other("bad key length"))?;
        sys::set_crypto(fd, direction, &state)?;
    }
    Ok(())
}

/// Map a rustls cipher suite to its kTLS AEAD.
pub fn cipher_of(suite: rustls::CipherSuite) -> Option<Cipher> {
    use rustls::CipherSuite as S;
    match suite {
        S::TLS13_AES_128_GCM_SHA256 | S::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256 | S::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256 => {
            Some(Cipher::AesGcm128)
        }
        S::TLS13_AES_256_GCM_SHA384 | S::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384 | S::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384 => {
            Some(Cipher::AesGcm256)
        }
        S::TLS13_CHACHA20_POLY1305_SHA256
        | S::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256
        | S::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256 => Some(Cipher::Chacha20Poly1305),
        _ => None,
    }
}

// ---------------------------------------------------------------- metrics

/// Outcomes counted in `rws_tls_ktls_connections_total{result}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Both directions run in the kernel.
    Enabled,
    /// The negotiated cipher isn't one the kernel supports.
    Cipher,
    /// rustls still had records to send, or a record was half read.
    Buffered,
    /// The kernel refused the `tls` ULP on this socket.
    Ulp,
    /// Extraction or key installation failed after the point of no return;
    /// the connection was closed.
    Failed,
}

impl Outcome {
    const ALL: [Outcome; 5] = [Outcome::Enabled, Outcome::Cipher, Outcome::Buffered, Outcome::Ulp, Outcome::Failed];

    fn label(self) -> &'static str {
        match self {
            Outcome::Enabled => "enabled",
            Outcome::Cipher => "fallback_cipher",
            Outcome::Buffered => "fallback_buffered",
            Outcome::Ulp => "fallback_ulp",
            Outcome::Failed => "failed",
        }
    }
}

static OUTCOMES: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];
/// Connections ended because the peer sent a TLS 1.3 KeyUpdate.
static KEY_UPDATES: AtomicU64 = AtomicU64::new(0);

fn record(outcome: Outcome) {
    OUTCOMES[outcome as usize].fetch_add(1, Ordering::Relaxed);
}

/// How many connections ended with `outcome` so far.
pub fn count(outcome: Outcome) -> u64 {
    OUTCOMES[outcome as usize].load(Ordering::Relaxed)
}

/// Connections closed on a peer's KeyUpdate so far.
pub fn key_update_closes() -> u64 {
    KEY_UPDATES.load(Ordering::Relaxed)
}

/// The kTLS series for `/metrics` (empty until kTLS is enabled).
pub fn prometheus_text() -> String {
    if !enabled() {
        return String::new();
    }
    let mut out = String::from(
        "# HELP rws_tls_ktls_connections_total TLS connections by whether they were handed to kernel TLS\n# TYPE rws_tls_ktls_connections_total counter\n",
    );
    for o in Outcome::ALL {
        out.push_str(&format!("rws_tls_ktls_connections_total{{result=\"{}\"}} {}\n", o.label(), count(o)));
    }
    out.push_str("# HELP rws_tls_ktls_key_update_closes_total kTLS connections closed because the peer sent a TLS 1.3 KeyUpdate\n");
    out.push_str("# TYPE rws_tls_ktls_key_update_closes_total counter\n");
    out.push_str(&format!("rws_tls_ktls_key_update_closes_total {}\n", key_update_closes()));
    out
}

// ---------------------------------------------------------------- Corked

/// A stream that never reads past the end of the current TLS record while
/// corked: a record's 5-byte header, then exactly its body. The handshake
/// runs over it so rustls can't read ahead into application data. After
/// [`Corked::uncork`] reads pass straight through.
pub struct Corked<IO> {
    io: IO,
    corked: bool,
    header: [u8; 5],
    header_len: usize,
    body_left: usize,
}

impl<IO> Corked<IO> {
    pub fn new(io: IO) -> Self {
        Corked { io, corked: true, header: [0; 5], header_len: 0, body_left: 0 }
    }

    pub fn uncork(&mut self) {
        self.corked = false;
    }

    pub fn get_ref(&self) -> &IO {
        &self.io
    }

    pub fn into_inner(self) -> IO {
        self.io
    }

    /// Whether the last read ended exactly at a record boundary.
    pub fn at_record_boundary(&self) -> bool {
        self.header_len == 0 && self.body_left == 0
    }

    /// Account for `bytes` just read while corked.
    fn consumed(&mut self, mut bytes: &[u8]) {
        while !bytes.is_empty() {
            if self.body_left > 0 {
                let n = self.body_left.min(bytes.len());
                self.body_left -= n;
                bytes = &bytes[n..];
            } else {
                let n = (5 - self.header_len).min(bytes.len());
                self.header[self.header_len..self.header_len + n].copy_from_slice(&bytes[..n]);
                self.header_len += n;
                bytes = &bytes[n..];
                if self.header_len == 5 {
                    self.header_len = 0;
                    self.body_left = u16::from_be_bytes([self.header[3], self.header[4]]) as usize;
                }
            }
        }
    }

    /// How many bytes the next read may take while corked.
    fn limit(&self) -> usize {
        if self.body_left > 0 { self.body_left } else { 5 - self.header_len }
    }
}

impl<IO: AsyncRead + Unpin> AsyncRead for Corked<IO> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if !this.corked {
            return Pin::new(&mut this.io).poll_read(cx, buf);
        }
        let limit = this.limit().min(buf.remaining());
        let mut limited = buf.take(limit);
        ready!(Pin::new(&mut this.io).poll_read(cx, &mut limited))?;
        let n = limited.filled().len();
        let start = buf.filled().len();
        // SAFETY: `limited` was a view of `buf`'s unfilled part and the
        // inner read initialised its first `n` bytes.
        unsafe { buf.assume_init(n) };
        buf.advance(n);
        let read = buf.filled()[start..start + n].to_vec();
        this.consumed(&read);
        Poll::Ready(Ok(()))
    }
}

impl<IO: AsyncWrite + Unpin> AsyncWrite for Corked<IO> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().io).poll_write(cx, buf)
    }
    fn poll_write_vectored(self: Pin<&mut Self>, cx: &mut Context<'_>, bufs: &[io::IoSlice<'_>]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().io).poll_write_vectored(cx, bufs)
    }
    fn is_write_vectored(&self) -> bool {
        self.io.is_write_vectored()
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().io).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().io).poll_shutdown(cx)
    }
}

// ---------------------------------------------------------------- upgrade

/// What [`upgrade`] made of a finished handshake.
pub enum Upgraded<IO> {
    /// The kernel does the record layer now.
    Ktls(KtlsStream),
    /// Kept on rustls (uncorked); the reason was counted.
    Kept(IO),
    /// The connection can't continue (counted as [`Outcome::Failed`]).
    Failed(io::Error),
}

/// A TCP stream a kTLS connection can run on.
pub trait KtlsSocket {
    fn tcp(&self) -> &TcpStream;
    fn into_tcp(self) -> TcpStream;
}

impl KtlsSocket for TcpStream {
    fn tcp(&self) -> &TcpStream {
        self
    }
    fn into_tcp(self) -> TcpStream {
        self
    }
}

impl KtlsSocket for Corked<TcpStream> {
    fn tcp(&self) -> &TcpStream {
        &self.io
    }
    fn into_tcp(self) -> TcpStream {
        self.io
    }
}

/// Hand a finished server handshake to the kernel, or keep it on rustls.
pub fn upgrade(mut tls: tokio_rustls::server::TlsStream<Corked<TcpStream>>) -> Upgraded<tokio_rustls::server::TlsStream<Corked<TcpStream>>> {
    tls.get_mut().0.uncork();
    let (corked, conn) = tls.get_mut();
    let (version, cipher) = match (conn.protocol_version(), conn.negotiated_cipher_suite()) {
        (Some(rustls::ProtocolVersion::TLSv1_3), Some(s)) => (sys::TLS_1_3_VERSION, cipher_of(s.suite())),
        (Some(rustls::ProtocolVersion::TLSv1_2), Some(s)) => (sys::TLS_1_2_VERSION, cipher_of(s.suite())),
        _ => (0, None),
    };
    let Some(cipher) = cipher.filter(|c| supported_ciphers().contains(c)) else {
        record(Outcome::Cipher);
        return Upgraded::Kept(tls);
    };
    // Nothing may be left half-done in rustls: records still to send
    // (session tickets) or part of a record read. `Corked` keeps reads on
    // record boundaries; whole records rustls already read are processed
    // here and their plaintext handed over below.
    if conn.process_new_packets().is_err() || conn.wants_write() || !corked.at_record_boundary() {
        record(Outcome::Buffered);
        return Upgraded::Kept(tls);
    }
    // tokio-rustls keeps reading while rustls wants data, so a TLS 1.3
    // client's first request — sent with its Finished — may already be
    // decrypted. Its records are behind rustls' read sequence number, so
    // the kernel continues after them; the plaintext goes first.
    let mut prefix = Vec::new();
    let mut peer_closed = false;
    loop {
        let mut chunk = [0u8; 4096];
        match std::io::Read::read(&mut conn.reader(), &mut chunk) {
            Ok(0) => {
                peer_closed = true; // close_notify already received
                break;
            }
            Ok(n) => prefix.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(_) => {
                record(Outcome::Buffered);
                return Upgraded::Kept(tls);
            }
        }
    }
    if sys::set_ulp(corked.tcp().as_raw_fd()).is_err() {
        record(Outcome::Ulp);
        return Upgraded::Kept(tls);
    }
    // Past this point rustls' state is consumed: failures close the connection.
    let (corked, conn) = tls.into_inner();
    let tcp = corked.into_tcp();
    match install(&tcp, version, cipher, conn) {
        Ok(()) => {
            record(Outcome::Enabled);
            let mut stream = KtlsStream::new(tcp);
            stream.prefix = prefix;
            stream.read_closed = peer_closed && stream.prefix.is_empty();
            stream.closed_after_prefix = peer_closed;
            Upgraded::Ktls(stream)
        }
        Err(e) => {
            record(Outcome::Failed);
            Upgraded::Failed(e)
        }
    }
}

/// Extract the traffic secrets from `conn` and install them on `tcp`
/// (which already has the `tls` ULP).
fn install(tcp: &TcpStream, version: u16, cipher: Cipher, conn: impl ExtractSecrets) -> io::Result<()> {
    let secrets = conn.extract().map_err(|e| io::Error::other(format!("kTLS: secret extraction failed: {e}")))?;
    let fd = tcp.as_raw_fd();
    for (direction, (seq, secret)) in [(sys::TLS_TX, secrets.tx), (sys::TLS_RX, secrets.rx)] {
        let state = state_for(version, cipher, seq, &secret)
            .ok_or_else(|| io::Error::other("kTLS: negotiated cipher and extracted secret don't match"))?;
        sys::set_crypto(fd, direction, &state)?;
    }
    Ok(())
}

/// rustls connections whose secrets can be extracted (server and client).
pub trait ExtractSecrets {
    fn extract(self) -> Result<rustls::ExtractedSecrets, rustls::Error>;
}

impl ExtractSecrets for rustls::ServerConnection {
    fn extract(self) -> Result<rustls::ExtractedSecrets, rustls::Error> {
        self.dangerous_extract_secrets()
    }
}

impl ExtractSecrets for rustls::ClientConnection {
    fn extract(self) -> Result<rustls::ExtractedSecrets, rustls::Error> {
        self.dangerous_extract_secrets()
    }
}

fn state_for(version: u16, cipher: Cipher, seq: u64, secret: &rustls::ConnectionTrafficSecrets) -> Option<sys::CryptoState> {
    use rustls::ConnectionTrafficSecrets as T;
    let (key, iv) = match (cipher, secret) {
        (Cipher::AesGcm128, T::Aes128Gcm { key, iv })
        | (Cipher::AesGcm256, T::Aes256Gcm { key, iv })
        | (Cipher::Chacha20Poly1305, T::Chacha20Poly1305 { key, iv }) => (key, iv),
        _ => return None,
    };
    sys::CryptoState::new(version, cipher, key.as_ref(), iv.as_ref(), seq)
}

// ---------------------------------------------------------------- KtlsStream

/// A TCP stream whose TLS record layer runs in the kernel. Reads and writes
/// are plaintext; [`crate::server::sendfile`] can use its socket directly.
pub struct KtlsStream {
    tcp: TcpStream,
    /// Plaintext rustls decrypted before the hand-over, read first.
    prefix: Vec<u8>,
    prefix_read: usize,
    /// The peer's close_notify arrived before the hand-over: end of stream
    /// once `prefix` is read.
    closed_after_prefix: bool,
    /// `close_notify` received or EOF seen: further reads return 0.
    read_closed: bool,
    /// Our `close_notify` is sent (by `poll_shutdown`).
    close_notify_sent: bool,
}

impl KtlsStream {
    fn new(tcp: TcpStream) -> Self {
        KtlsStream { tcp, prefix: Vec::new(), prefix_read: 0, closed_after_prefix: false, read_closed: false, close_notify_sent: false }
    }

    pub fn tcp(&self) -> &TcpStream {
        &self.tcp
    }

    /// What to do with a non-data record (`record_type`, its plaintext).
    fn control_record(&mut self, record_type: u8, body: &[u8]) -> io::Result<()> {
        match record_type {
            sys::RECORD_ALERT if body.len() >= 2 && body[1] == 0 => {
                // close_notify: the peer's clean end of stream.
                self.read_closed = true;
                Ok(())
            }
            sys::RECORD_ALERT if body.len() >= 2 => {
                Err(io::Error::new(io::ErrorKind::ConnectionAborted, format!("kTLS: peer sent TLS alert {}", body[1])))
            }
            // A TLS 1.3 post-handshake message: 24 = KeyUpdate.
            sys::RECORD_HANDSHAKE if body.first() == Some(&24) => {
                KEY_UPDATES.fetch_add(1, Ordering::Relaxed);
                Err(io::Error::new(io::ErrorKind::ConnectionAborted, "kTLS: peer sent a TLS 1.3 KeyUpdate; closing the connection"))
            }
            sys::RECORD_HANDSHAKE => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("kTLS: unexpected post-handshake message type {:?}", body.first()),
            )),
            other => Err(io::Error::new(io::ErrorKind::InvalidData, format!("kTLS: unexpected TLS record type {other}"))),
        }
    }
}

impl AsyncRead for KtlsStream {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.prefix_read < this.prefix.len() {
            let n = (this.prefix.len() - this.prefix_read).min(buf.remaining());
            buf.put_slice(&this.prefix[this.prefix_read..this.prefix_read + n]);
            this.prefix_read += n;
            if this.prefix_read == this.prefix.len() {
                this.prefix = Vec::new();
                this.prefix_read = 0;
                this.read_closed |= this.closed_after_prefix;
            }
            return Poll::Ready(Ok(()));
        }
        if this.read_closed || buf.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        loop {
            ready!(this.tcp.poll_read_ready(cx))?;
            let fd = this.tcp.as_raw_fd();
            // SAFETY: recvmsg only writes into the slice; we mark as
            // initialised just what it returned.
            let unfilled = unsafe { &mut *(buf.unfilled_mut() as *mut [std::mem::MaybeUninit<u8>] as *mut [u8]) };
            match this.tcp.try_io(Interest::READABLE, || sys::recv_record(fd, unfilled)) {
                Ok((n, sys::RECORD_APPLICATION_DATA)) => {
                    if n == 0 {
                        this.read_closed = true;
                    }
                    // SAFETY: the kernel wrote `n` bytes.
                    unsafe { buf.assume_init(n) };
                    buf.advance(n);
                    return Poll::Ready(Ok(()));
                }
                Ok((n, record_type)) => {
                    let body = unfilled[..n].to_vec();
                    this.control_record(record_type, &body)?;
                    if this.read_closed {
                        return Poll::Ready(Ok(()));
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => return Poll::Ready(Err(e)),
            }
        }
    }
}

impl AsyncWrite for KtlsStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().tcp).poll_write(cx, buf)
    }
    fn poll_write_vectored(self: Pin<&mut Self>, cx: &mut Context<'_>, bufs: &[io::IoSlice<'_>]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().tcp).poll_write_vectored(cx, bufs)
    }
    fn is_write_vectored(&self) -> bool {
        true
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().tcp).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        while !this.close_notify_sent {
            ready!(this.tcp.poll_write_ready(cx))?;
            let fd = this.tcp.as_raw_fd();
            match this.tcp.try_io(Interest::WRITABLE, || sys::send_record(fd, sys::RECORD_ALERT, &[1, 0])) {
                Ok(_) => this.close_notify_sent = true,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                // The peer is gone; nothing more to say.
                Err(_) => this.close_notify_sent = true,
            }
        }
        Pin::new(&mut this.tcp).poll_shutdown(cx)
    }
}
