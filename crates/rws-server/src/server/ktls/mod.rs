//! Kernel TLS (kTLS, Linux): after rustls finishes the handshake, the session
//! keys go to the kernel (`TCP_ULP "tls"`) and the kernel encrypts and
//! decrypts the records. The connection then reads and writes plaintext on
//! the TCP socket, so static files go out with `sendfile(2)` over HTTPS as
//! they do over plain HTTP, without passing through user space.
//!
//! Opt-in with `tls_ktls = true` (`RWS_CONFIG_TLS_KTLS`); off by default and
//! never in `fips` builds (the kernel's AEAD isn't the validated module).
//! Whether it saves CPU depends on the kernel and CPU: on Linux 6.8 with an
//! AES-NI/AVX2 CPU (no VAES) it doesn't (`bench/ktls.sh`, `spec/KTLS.md`).
//! NIC TLS offload, where the kernel hands records to the card, is what
//! kTLS makes possible beyond that.
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

/// A finished tokio-rustls handshake over [`Corked`]: server or client.
pub trait Handshaken: Sized {
    type Data;
    type Conn: std::ops::DerefMut<Target = rustls::ConnectionCommon<Self::Data>> + ExtractSecrets;
    /// The client side (it receives TLS 1.3 session tickets after the
    /// handshake).
    const CLIENT: bool;
    fn parts(&mut self) -> (&mut Corked<TcpStream>, &mut Self::Conn);
    fn into_parts(self) -> (Corked<TcpStream>, Self::Conn);
}

impl Handshaken for tokio_rustls::server::TlsStream<Corked<TcpStream>> {
    type Data = rustls::server::ServerConnectionData;
    type Conn = rustls::ServerConnection;
    const CLIENT: bool = false;
    fn parts(&mut self) -> (&mut Corked<TcpStream>, &mut Self::Conn) {
        self.get_mut()
    }
    fn into_parts(self) -> (Corked<TcpStream>, Self::Conn) {
        self.into_inner()
    }
}

impl Handshaken for tokio_rustls::client::TlsStream<Corked<TcpStream>> {
    type Data = rustls::client::ClientConnectionData;
    type Conn = rustls::ClientConnection;
    const CLIENT: bool = true;
    fn parts(&mut self) -> (&mut Corked<TcpStream>, &mut Self::Conn) {
        self.get_mut()
    }
    fn into_parts(self) -> (Corked<TcpStream>, Self::Conn) {
        self.into_inner()
    }
}

/// Hand a finished handshake (server or client) to the kernel, or keep it
/// on rustls.
pub fn upgrade<T: Handshaken>(mut tls: T) -> Upgraded<T> {
    tls.parts().0.uncork();
    let (corked, conn) = tls.parts();
    let (version, cipher) = match (conn.protocol_version(), conn.negotiated_cipher_suite()) {
        (Some(rustls::ProtocolVersion::TLSv1_3), Some(s)) => (sys::TLS_1_3_VERSION, cipher_of(s.suite())),
        (Some(rustls::ProtocolVersion::TLSv1_2), Some(s)) => (sys::TLS_1_2_VERSION, cipher_of(s.suite())),
        _ => (0, None),
    };
    let Some(cipher) = cipher.filter(|c| supported_ciphers().contains(c)) else {
        record(Outcome::Cipher);
        return Upgraded::Kept(tls);
    };
    let record_limit = match conn.negotiated_cipher_suite() {
        Some(rustls::SupportedCipherSuite::Tls13(s)) => s.common.confidentiality_limit,
        Some(rustls::SupportedCipherSuite::Tls12(s)) => s.common.confidentiality_limit,
        None => 0,
    };
    // Nothing may be left half-done in rustls: records still to send
    // (session tickets) or part of a record read. `Corked` keeps reads on
    // record boundaries; whole records rustls already read are processed
    // here and their plaintext handed over below.
    if conn.process_new_packets().is_err() || conn.wants_write() || !corked.at_record_boundary() {
        record(Outcome::Buffered);
        return Upgraded::Kept(tls);
    }
    if sys::set_ulp(corked.tcp().as_raw_fd()).is_err() {
        record(Outcome::Ulp);
        return Upgraded::Kept(tls);
    }
    // From here the socket has the `tls` layer but no keys yet, so it still
    // passes bytes through: a fallback below keeps working on rustls.
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
            Err(e) => {
                // Some plaintext may be gone already: don't carry on.
                record(Outcome::Failed);
                return Upgraded::Failed(e);
            }
        }
    }
    // Past this point rustls' state is consumed: failures close the connection.
    let (corked, conn) = tls.into_parts();
    let tcp = corked.into_tcp();
    match install(&tcp, version, cipher, conn) {
        Ok(tx_seq) => {
            record(Outcome::Enabled);
            let mut stream = KtlsStream::new(tcp);
            stream.reader.client = T::CLIENT;
            stream.records_left = AtomicU64::new(record_limit.saturating_sub(tx_seq).saturating_sub(RECORD_LIMIT_MARGIN));
            stream.reader.closed = peer_closed && prefix.is_empty();
            stream.reader.prefix = prefix;
            stream.reader.closed_after_prefix = peer_closed;
            Upgraded::Ktls(stream)
        }
        Err(e) => {
            record(Outcome::Failed);
            Upgraded::Failed(e)
        }
    }
}

/// Extract the traffic secrets from `conn` and install them on `tcp`
/// (which already has the `tls` ULP). Returns the send sequence number the
/// kernel starts at.
fn install(tcp: &TcpStream, version: u16, cipher: Cipher, conn: impl ExtractSecrets) -> io::Result<u64> {
    let secrets = conn.extract().map_err(|e| io::Error::other(format!("kTLS: secret extraction failed: {e}")))?;
    let tx_seq = secrets.tx.0;
    let fd = tcp.as_raw_fd();
    for (direction, (seq, secret)) in [(sys::TLS_TX, secrets.tx), (sys::TLS_RX, secrets.rx)] {
        let tx_tls12_gcm = direction == sys::TLS_TX && version == sys::TLS_1_2_VERSION && cipher != Cipher::Chacha20Poly1305;
        let state = state_for(version, cipher, seq, &secret, tx_tls12_gcm)
            .ok_or_else(|| io::Error::other("kTLS: negotiated cipher and extracted secret don't match"))?;
        sys::set_crypto(fd, direction, &state)?;
    }
    Ok(tx_seq)
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

/// One direction's kernel state from rustls' extracted secret.
///
/// `tx_tls12_gcm`: TLS 1.2 AES-GCM sends an explicit 8-byte nonce per
/// record, and the two sides derive it differently — rustls as the IV's
/// last 8 bytes XOR the sequence number, the kernel by incrementing from the
/// value it's given. Handing the kernel rustls' IV unchanged makes its first
/// record reuse the nonce of rustls' Finished under the same key (AES-GCM
/// nonce reuse). The explicit nonce only has to be unique — the receiver
/// reads it off the wire — so the kernel starts half the 64-bit space away:
/// rustls used `iv ^ seq` for a handful of small `seq`, all within 2^16 of
/// `iv`, and the kernel's `iv + 2^63 + n` can't come back within 2^16 of
/// `iv` before n ≈ 2^63, far past AES-GCM's record limit.
fn state_for(version: u16, cipher: Cipher, seq: u64, secret: &rustls::ConnectionTrafficSecrets, tx_tls12_gcm: bool) -> Option<sys::CryptoState> {
    use rustls::ConnectionTrafficSecrets as T;
    let (key, iv) = match (cipher, secret) {
        (Cipher::AesGcm128, T::Aes128Gcm { key, iv })
        | (Cipher::AesGcm256, T::Aes256Gcm { key, iv })
        | (Cipher::Chacha20Poly1305, T::Chacha20Poly1305 { key, iv }) => (key, iv),
        _ => return None,
    };
    let mut iv: [u8; 12] = iv.as_ref().try_into().ok()?;
    if tx_tls12_gcm {
        let start = tls12_explicit_nonce_start(&iv);
        iv[4..].copy_from_slice(&start.to_be_bytes());
    }
    sys::CryptoState::new(version, cipher, key.as_ref(), &iv, seq)
}

/// Where the kernel's TLS 1.2 AES-GCM explicit nonces start: rustls' base
/// plus 2^63 (see [`state_for`]).
fn tls12_explicit_nonce_start(iv: &[u8; 12]) -> u64 {
    u64::from_be_bytes(iv[4..].try_into().expect("8 bytes")).wrapping_add(1 << 63)
}

// ---------------------------------------------------------------- KtlsStream

/// Largest TLS record plaintext: a write of `n` bytes is at most
/// `ceil(n / MAX_RECORD)` records (the kernel may merge, never split finer).
const MAX_RECORD: u64 = 16_384;

/// Records kept in reserve below the cipher's confidentiality limit (rustls
/// stops at the limit itself; this leaves room for the close_notify).
const RECORD_LIMIT_MARGIN: u64 = 16;

/// Records needed for `bytes` of plaintext, at most.
fn records_for(bytes: u64) -> u64 {
    bytes.div_ceil(MAX_RECORD).max(1)
}

/// The read side of a kTLS connection, shared by [`KtlsStream`] and
/// [`BlockingKtlsReader`]: plaintext rustls decrypted before the hand-over,
/// then records from the kernel, with control records acted on.
struct RecordReader {
    /// Plaintext rustls decrypted before the hand-over, read first.
    prefix: Vec<u8>,
    prefix_read: usize,
    /// The peer's close_notify arrived before the hand-over: end of stream
    /// once `prefix` is read.
    closed_after_prefix: bool,
    /// `close_notify` received or EOF seen: further reads return 0.
    closed: bool,
    /// A control message (alert, post-handshake message) read in part.
    control: Vec<u8>,
    control_type: u8,
    /// Client side: TLS 1.3 session tickets the server sends after the
    /// handshake are skipped (rustls can't take them once the keys are out,
    /// so the next connection does a full handshake).
    client: bool,
}

/// What one `recvmsg` amounted to.
enum Got {
    /// `n` bytes of application data in the caller's buffer (0 = EOF).
    Data(usize),
    /// A control record (or part of one) was handled: read again.
    Again,
}

impl RecordReader {
    fn new() -> Self {
        RecordReader { prefix: Vec::new(), prefix_read: 0, closed_after_prefix: false, closed: false, control: Vec::new(), control_type: 0, client: false }
    }

    /// Copy prefix plaintext into `out`; `None` once there is none.
    fn take_prefix(&mut self, out: &mut [u8]) -> Option<usize> {
        if self.prefix_read >= self.prefix.len() || out.is_empty() {
            return None;
        }
        let n = (self.prefix.len() - self.prefix_read).min(out.len());
        out[..n].copy_from_slice(&self.prefix[self.prefix_read..self.prefix_read + n]);
        self.prefix_read += n;
        if self.prefix_read == self.prefix.len() {
            self.prefix = Vec::new();
            self.prefix_read = 0;
            self.closed |= self.closed_after_prefix;
        }
        Some(n)
    }

    /// Account for a record (`n` bytes of `record_type` in `buf`).
    fn got(&mut self, buf: &[u8], n: usize, record_type: u8) -> io::Result<Got> {
        if record_type == sys::RECORD_APPLICATION_DATA {
            if !self.control.is_empty() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "kTLS: application data inside a TLS control message"));
            }
            if n == 0 {
                self.closed = true;
            }
            return Ok(Got::Data(n));
        }
        self.control_record(record_type, &buf[..n])?;
        Ok(if self.closed { Got::Data(0) } else { Got::Again })
    }

    /// Bytes of a non-data record (`record_type`). The kernel returns a
    /// control record in pieces when the reader's buffer is smaller, so they
    /// collect in `control` and each complete message is acted on: alerts
    /// are 2 bytes, handshake messages carry their own length.
    fn control_record(&mut self, record_type: u8, bytes: &[u8]) -> io::Result<()> {
        if !self.control.is_empty() && self.control_type != record_type {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "kTLS: TLS record types interleaved inside a message"));
        }
        self.control_type = record_type;
        self.control.extend_from_slice(bytes);
        loop {
            let used = match record_type {
                sys::RECORD_ALERT if self.control.len() >= 2 => {
                    if self.control[1] != 0 {
                        return Err(io::Error::new(io::ErrorKind::ConnectionAborted, format!("kTLS: peer sent TLS alert {}", self.control[1])));
                    }
                    // close_notify: the peer's clean end of stream.
                    self.closed = true;
                    2
                }
                sys::RECORD_HANDSHAKE if self.control.len() >= 4 => {
                    let len = u32::from_be_bytes([0, self.control[1], self.control[2], self.control[3]]) as usize;
                    if self.control.len() < 4 + len {
                        return Ok(()); // the rest of the message is still to come
                    }
                    match self.control[0] {
                        // TLS 1.3 NewSessionTicket to a client: skipped.
                        4 if self.client => {}
                        24 => {
                            KEY_UPDATES.fetch_add(1, Ordering::Relaxed);
                            return Err(io::Error::new(
                                io::ErrorKind::ConnectionAborted,
                                "kTLS: peer sent a TLS 1.3 KeyUpdate; closing the connection",
                            ));
                        }
                        other => {
                            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("kTLS: unexpected post-handshake message type {other}")));
                        }
                    }
                    4 + len
                }
                sys::RECORD_ALERT | sys::RECORD_HANDSHAKE => return Ok(()), // incomplete
                other => {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, format!("kTLS: unexpected TLS record type {other}")));
                }
            };
            self.control.drain(..used);
            if self.control.is_empty() || self.closed {
                return Ok(());
            }
        }
    }
}

/// A TCP stream whose TLS record layer runs in the kernel. Reads and writes
/// are plaintext; [`crate::server::sendfile`] can use its socket directly.
pub struct KtlsStream {
    tcp: TcpStream,
    reader: RecordReader,
    /// Records we may still send under these keys: AES-GCM has a
    /// confidentiality limit (about 2^24.5 records) and a kTLS connection
    /// can't rekey, so it ends before the limit. Atomic because `sendfile`
    /// reserves through `&self`.
    records_left: AtomicU64,
    /// Our `close_notify` is sent (by `poll_shutdown`).
    close_notify_sent: bool,
}

impl KtlsStream {
    fn new(tcp: TcpStream) -> Self {
        KtlsStream { tcp, reader: RecordReader::new(), records_left: AtomicU64::new(u64::MAX), close_notify_sent: false }
    }

    pub fn tcp(&self) -> &TcpStream {
        &self.tcp
    }

    /// Whether `bytes` more plaintext stay within the cipher's limit.
    fn has_room(&self, bytes: u64) -> bool {
        records_for(bytes) <= self.records_left.load(Ordering::Relaxed)
    }

    /// Reserve the records `bytes` of plaintext take; `false` (nothing
    /// reserved) when that would pass the cipher's limit — the connection
    /// must end.
    pub fn reserve_records(&self, bytes: u64) -> bool {
        let need = records_for(bytes);
        self.records_left.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |left| left.checked_sub(need)).is_ok()
    }

    fn limit_reached() -> io::Error {
        io::Error::other("kTLS: the cipher's record limit is reached; closing the connection")
    }

    /// The rest of the stream as a blocking reader on the same socket (for
    /// code that reads a body on a blocking thread), with `read_timeout`.
    pub fn into_blocking_reader(self, read_timeout: std::time::Duration) -> io::Result<BlockingKtlsReader> {
        let socket = self.tcp.into_std()?;
        socket.set_nonblocking(false)?;
        socket.set_read_timeout(Some(read_timeout))?;
        Ok(BlockingKtlsReader { socket, reader: self.reader })
    }
}

impl AsyncRead for KtlsStream {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        // SAFETY: only written into (by a copy or recvmsg); we mark as
        // initialised just what was written.
        let unfilled = unsafe { &mut *(buf.unfilled_mut() as *mut [std::mem::MaybeUninit<u8>] as *mut [u8]) };
        if let Some(n) = this.reader.take_prefix(unfilled) {
            // SAFETY: n bytes were copied in.
            unsafe { buf.assume_init(n) };
            buf.advance(n);
            return Poll::Ready(Ok(()));
        }
        if this.reader.closed || unfilled.is_empty() {
            return Poll::Ready(Ok(()));
        }
        loop {
            ready!(this.tcp.poll_read_ready(cx))?;
            let fd = this.tcp.as_raw_fd();
            match this.tcp.try_io(Interest::READABLE, || sys::recv_record(fd, unfilled)) {
                Ok((n, record_type)) => match this.reader.got(unfilled, n, record_type)? {
                    Got::Data(n) => {
                        // SAFETY: the kernel wrote `n` bytes.
                        unsafe { buf.assume_init(n) };
                        buf.advance(n);
                        return Poll::Ready(Ok(()));
                    }
                    Got::Again => continue,
                },
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => return Poll::Ready(Err(e)),
            }
        }
    }
}

/// A kTLS connection's read side on a blocking socket
/// ([`KtlsStream::into_blocking_reader`]).
pub struct BlockingKtlsReader {
    socket: std::net::TcpStream,
    reader: RecordReader,
}

impl std::io::Read for BlockingKtlsReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(n) = self.reader.take_prefix(buf) {
            return Ok(n);
        }
        if self.reader.closed || buf.is_empty() {
            return Ok(0);
        }
        loop {
            let (n, record_type) = sys::recv_record(self.socket.as_raw_fd(), buf)?;
            match self.reader.got(buf, n, record_type)? {
                Got::Data(n) => return Ok(n),
                Got::Again => continue,
            }
        }
    }
}

impl AsyncWrite for KtlsStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if !this.has_room(buf.len() as u64) {
            return Poll::Ready(Err(KtlsStream::limit_reached()));
        }
        let n = ready!(Pin::new(&mut this.tcp).poll_write(cx, buf))?;
        this.reserve_records(n as u64);
        Poll::Ready(Ok(n))
    }
    fn poll_write_vectored(self: Pin<&mut Self>, cx: &mut Context<'_>, bufs: &[io::IoSlice<'_>]) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if !this.has_room(bufs.iter().map(|b| b.len() as u64).sum()) {
            return Poll::Ready(Err(KtlsStream::limit_reached()));
        }
        let n = ready!(Pin::new(&mut this.tcp).poll_write_vectored(cx, bufs))?;
        this.reserve_records(n as u64);
        Poll::Ready(Ok(n))
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
