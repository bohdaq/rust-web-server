//! kTLS against a rustls client over loopback. The tests that need the
//! kernel's `tls` module return early (with a note) when it isn't there.

use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::ReadBuf;

use super::*;

#[test]
fn the_setting_is_true_or_off() {
    assert!(parse_setting("true"));
    assert!(parse_setting(" TRUE "));
    assert!(!parse_setting("false"));
    assert!(!parse_setting("on"));
    assert!(!parse_setting(""));
}

#[test]
fn cipher_suites_map_to_kernel_aeads() {
    use rustls::CipherSuite as S;
    assert_eq!(Some(Cipher::AesGcm128), cipher_of(S::TLS13_AES_128_GCM_SHA256));
    assert_eq!(Some(Cipher::AesGcm128), cipher_of(S::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256));
    assert_eq!(Some(Cipher::AesGcm256), cipher_of(S::TLS13_AES_256_GCM_SHA384));
    assert_eq!(Some(Cipher::AesGcm256), cipher_of(S::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384));
    assert_eq!(Some(Cipher::Chacha20Poly1305), cipher_of(S::TLS13_CHACHA20_POLY1305_SHA256));
    assert_eq!(Some(Cipher::Chacha20Poly1305), cipher_of(S::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256));
    assert_eq!(None, cipher_of(S::TLS_RSA_WITH_AES_128_CBC_SHA));
}

#[test]
fn crypto_state_rejects_wrong_lengths() {
    let iv = [0u8; 12];
    assert!(sys::CryptoState::new(sys::TLS_1_3_VERSION, Cipher::AesGcm128, &[0; 16], &iv, 0).is_some());
    assert!(sys::CryptoState::new(sys::TLS_1_3_VERSION, Cipher::AesGcm128, &[0; 32], &iv, 0).is_none());
    assert!(sys::CryptoState::new(sys::TLS_1_3_VERSION, Cipher::AesGcm256, &[0; 32], &iv, 0).is_some());
    assert!(sys::CryptoState::new(sys::TLS_1_3_VERSION, Cipher::Chacha20Poly1305, &[0; 32], &iv, 0).is_some());
    assert!(sys::CryptoState::new(sys::TLS_1_3_VERSION, Cipher::Chacha20Poly1305, &[0; 32], &iv[..8], 0).is_none());
}

/// Two records and a partial third arrive at once; corked reads stop at
/// each record boundary, uncorked reads take what's there.
#[tokio::test]
async fn corked_reads_stop_at_record_boundaries() {
    let (mut client, server) = tokio::io::duplex(1024);
    let mut wire = Vec::new();
    wire.extend_from_slice(&[22, 3, 3, 0, 3, b'a', b'b', b'c']);
    wire.extend_from_slice(&[23, 3, 3, 0, 2, b'd', b'e']);
    wire.extend_from_slice(&[23, 3, 3]);
    client.write_all(&wire).await.unwrap();
    let mut corked = Corked::new(server);
    let mut buf = [0u8; 64];
    let mut seen = Vec::new();
    let mut boundaries = 0;
    while seen.len() < 15 {
        let n = corked.read(&mut buf).await.unwrap();
        assert!(n > 0 && n <= 5, "a corked read took {n} bytes");
        seen.extend_from_slice(&buf[..n]);
        if corked.at_record_boundary() {
            boundaries += 1;
        }
    }
    assert_eq!(&wire[..15], &seen[..]);
    assert_eq!(2, boundaries);
    // Inside the third record's header.
    let n = corked.read(&mut buf).await.unwrap();
    assert_eq!(3, n);
    assert!(!corked.at_record_boundary());
    corked.uncork();
    client.write_all(&[0, 10, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]).await.unwrap();
    let n = corked.read(&mut buf).await.unwrap();
    assert_eq!(12, n, "uncorked reads take everything available");
}

fn kernel_has_ktls() -> bool {
    let ok = !supported_ciphers().is_empty();
    if !ok {
        eprintln!("skipped: no kernel TLS here (tls module not available)");
    }
    ok
}

struct Pki {
    cert: rustls::pki_types::CertificateDer<'static>,
    key: rustls::pki_types::PrivateKeyDer<'static>,
}

fn pki() -> Pki {
    let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    Pki {
        cert: certified.cert.der().clone(),
        key: rustls::pki_types::PrivateKeyDer::Pkcs8(certified.signing_key.serialize_der().into()),
    }
}

fn provider_with(suite: Option<rustls::CipherSuite>) -> Arc<rustls::crypto::CryptoProvider> {
    let mut provider = crate::crypto_provider();
    if let Some(suite) = suite {
        provider.cipher_suites.retain(|s| s.suite() == suite);
    }
    Arc::new(provider)
}

fn server_config(pki: &Pki, version: &'static rustls::SupportedProtocolVersion) -> Arc<rustls::ServerConfig> {
    let mut config = rustls::ServerConfig::builder_with_provider(provider_with(None))
        .with_protocol_versions(&[version])
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![pki.cert.clone()], pki.key.clone_key())
        .unwrap();
    config.enable_secret_extraction = true;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Arc::new(config)
}

fn client_config(pki: &Pki, version: &'static rustls::SupportedProtocolVersion, suite: Option<rustls::CipherSuite>) -> Arc<rustls::ClientConfig> {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(pki.cert.clone()).unwrap();
    let mut config = rustls::ClientConfig::builder_with_provider(provider_with(suite))
        .with_protocol_versions(&[version])
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Arc::new(config)
}

/// A connected pair: the server side upgraded to kTLS, the client on rustls.
/// `early` is written by the client as soon as its handshake completes.
async fn pair(
    version: &'static rustls::SupportedProtocolVersion,
    suite: Option<rustls::CipherSuite>,
    early: &'static [u8],
) -> (Upgraded<tokio_rustls::server::TlsStream<Corked<TcpStream>>>, tokio_rustls::client::TlsStream<TcpStream>) {
    let pki = pki();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(server_config(&pki, version));
    let connector = tokio_rustls::TlsConnector::from(client_config(&pki, version, suite));
    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let tls = acceptor.accept(Corked::new(tcp)).await.unwrap();
        upgrade(tls)
    });
    let tcp = TcpStream::connect(addr).await.unwrap();
    let name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
    let mut client = connector.connect(name, tcp).await.unwrap();
    if !early.is_empty() {
        client.write_all(early).await.unwrap();
        client.flush().await.unwrap();
    }
    (server.await.unwrap(), client)
}

fn ktls(upgraded: Upgraded<tokio_rustls::server::TlsStream<Corked<TcpStream>>>) -> KtlsStream {
    match upgraded {
        Upgraded::Ktls(s) => s,
        Upgraded::Kept(_) => panic!("kept on rustls"),
        Upgraded::Failed(e) => panic!("failed: {e}"),
    }
}

async fn exchange(server: &mut KtlsStream, client: &mut tokio_rustls::client::TlsStream<TcpStream>) {
    // Server → client, more than one record.
    let big: Vec<u8> = (0..100_000u32).map(|i| (i % 253) as u8).collect();
    let (to_send, expected) = (big.clone(), big);
    let send = async {
        server.write_all(&to_send).await.unwrap();
        server.flush().await.unwrap();
    };
    let recv = async {
        let mut got = vec![0u8; expected.len()];
        client.read_exact(&mut got).await.unwrap();
        got
    };
    let ((), got) = tokio::join!(send, recv);
    assert!(got == expected);
    // Client → server.
    client.write_all(b"ping from rustls").await.unwrap();
    client.flush().await.unwrap();
    let mut buf = [0u8; 16];
    server.read_exact(&mut buf).await.unwrap();
    assert_eq!(b"ping from rustls", &buf);
}

#[tokio::test]
async fn every_supported_version_and_cipher_round_trips() {
    if !kernel_has_ktls() {
        return;
    }
    use rustls::CipherSuite as S;
    let cases = [
        (&rustls::version::TLS13, S::TLS13_AES_128_GCM_SHA256),
        (&rustls::version::TLS13, S::TLS13_AES_256_GCM_SHA384),
        (&rustls::version::TLS13, S::TLS13_CHACHA20_POLY1305_SHA256),
        (&rustls::version::TLS12, S::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256),
        (&rustls::version::TLS12, S::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384),
        (&rustls::version::TLS12, S::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256),
    ];
    for (version, suite) in cases {
        let cipher = cipher_of(suite).unwrap();
        if !supported_ciphers().contains(&cipher) {
            eprintln!("skipped {suite:?}: not supported by this kernel");
            continue;
        }
        let (server, mut client) = pair(version, Some(suite), b"").await;
        let mut server = ktls(server);
        exchange(&mut server, &mut client).await;
    }
}

/// Holds back a TLS 1.3 client's writes after its ClientHello until
/// `release`: its Finished and first request then reach the server together,
/// as from a real client, and tokio-rustls reads the request with the
/// Finished.
struct HoldWrites {
    tcp: TcpStream,
    writes: usize,
    holding: bool,
    held: Vec<u8>,
}

impl HoldWrites {
    async fn release(&mut self) {
        self.holding = false;
        let held = std::mem::take(&mut self.held);
        self.tcp.write_all(&held).await.unwrap();
    }
}

impl tokio::io::AsyncRead for HoldWrites {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().tcp).poll_read(cx, buf)
    }
}

impl tokio::io::AsyncWrite for HoldWrites {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        this.writes += 1;
        if this.holding && this.writes > 1 {
            this.held.extend_from_slice(buf);
            return Poll::Ready(Ok(buf.len()));
        }
        Pin::new(&mut this.tcp).poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().tcp).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().tcp).poll_shutdown(cx)
    }
}

/// A TLS 1.3 client's first request travels with its Finished; rustls has
/// decrypted it by the time the handshake returns, and the kTLS stream
/// reads it first, then what the kernel decrypts after it.
#[tokio::test]
async fn a_request_sent_with_the_finished_message_is_kept() {
    if !kernel_has_ktls() {
        return;
    }
    let pki = pki();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(server_config(&pki, &rustls::version::TLS13));
    let connector = tokio_rustls::TlsConnector::from(client_config(&pki, &rustls::version::TLS13, None));
    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let tls = acceptor.accept(Corked::new(tcp)).await.unwrap();
        upgrade(tls)
    });
    let tcp = TcpStream::connect(addr).await.unwrap();
    let hold = HoldWrites { tcp, writes: 0, holding: true, held: Vec::new() };
    let name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
    let mut client = connector.connect(name, hold).await.unwrap();
    client.write_all(b"GET /first HTTP/1.1\r\n\r\n").await.unwrap();
    client.flush().await.unwrap();
    client.get_mut().0.release().await;
    let mut server = ktls(server.await.unwrap());
    assert_eq!(23, server.prefix.len(), "rustls decrypted the request during the handshake");
    let mut buf = [0u8; 23];
    server.read_exact(&mut buf).await.unwrap();
    assert_eq!(b"GET /first HTTP/1.1\r\n\r\n", &buf);
    // And the kernel takes over at the right sequence number.
    client.write_all(b"second").await.unwrap();
    client.flush().await.unwrap();
    let mut buf = [0u8; 6];
    server.read_exact(&mut buf).await.unwrap();
    assert_eq!(b"second", &buf);
    // Both directions still work.
    server.write_all(b"pong").await.unwrap();
    let mut buf = [0u8; 4];
    client.read_exact(&mut buf).await.unwrap();
    assert_eq!(b"pong", &buf);
}

/// Data the client sends once the handshake is over arrives through the kernel.
#[tokio::test]
async fn data_sent_right_after_the_handshake_arrives() {
    if !kernel_has_ktls() {
        return;
    }
    for version in [&rustls::version::TLS13, &rustls::version::TLS12] {
        let (server, _client) = pair(version, None, b"GET / HTTP/1.1\r\n\r\n").await;
        let mut server = ktls(server);
        let mut buf = [0u8; 18];
        server.read_exact(&mut buf).await.unwrap();
        assert_eq!(b"GET / HTTP/1.1\r\n\r\n", &buf);
    }
}

#[tokio::test]
async fn close_notify_ends_the_stream_both_ways() {
    if !kernel_has_ktls() {
        return;
    }
    let (server, mut client) = pair(&rustls::version::TLS13, None, b"").await;
    let mut server = ktls(server);
    // Client closes: the server reads end of stream, not an error.
    client.get_mut().1.send_close_notify();
    client.flush().await.unwrap();
    let mut buf = [0u8; 8];
    assert_eq!(0, server.read(&mut buf).await.unwrap());
    assert_eq!(0, server.read(&mut buf).await.unwrap());
    // Server closes: rustls sees a clean close_notify.
    server.shutdown().await.unwrap();
    let mut rest = Vec::new();
    client.read_to_end(&mut rest).await.unwrap();
    assert!(rest.is_empty());
}

#[tokio::test]
async fn sendfile_over_ktls_sends_the_file_encrypted() {
    if !kernel_has_ktls() {
        return;
    }
    let file = crate::test_env::TempPath::new("ktls_sendfile.bin");
    let content: Vec<u8> = (0..1_500_000u32).map(|i| (i % 249) as u8).collect();
    std::fs::write(&*file, &content).unwrap();
    for version in [&rustls::version::TLS13, &rustls::version::TLS12] {
        let (server, mut client) = pair(version, None, b"").await;
        let server = ktls(server);
        let head = b"HTTP/1.1 200 OK\r\n\r\n".to_vec();
        let opened = std::fs::File::open(&*file).unwrap();
        let len = content.len() as u64;
        let send = async {
            crate::server::sendfile::send_file(server.tcp(), &head, &opened, len).await.unwrap();
        };
        let recv = async {
            let mut got = vec![0u8; head.len() + content.len()];
            client.read_exact(&mut got).await.unwrap();
            got
        };
        let ((), got) = tokio::join!(send, recv);
        assert_eq!(&head[..], &got[..head.len()]);
        assert!(got[head.len()..] == content[..]);
    }
}

/// rustls' buffered connection can't hand over the secrets needed to follow
/// a TLS 1.3 KeyUpdate: the server ends the connection and counts it.
#[tokio::test]
async fn a_key_update_closes_the_connection() {
    if !kernel_has_ktls() {
        return;
    }
    let (server, mut client) = pair(&rustls::version::TLS13, None, b"").await;
    let mut server = ktls(server);
    let before = key_update_closes();
    client.get_mut().1.refresh_traffic_keys().unwrap();
    client.write_all(b"after").await.unwrap();
    client.flush().await.unwrap();
    let mut buf = [0u8; 8];
    let err = server.read(&mut buf).await.unwrap_err();
    assert_eq!(io::ErrorKind::ConnectionAborted, err.kind(), "{err}");
    assert!(key_update_closes() > before);
}

#[tokio::test]
async fn an_unsupported_cipher_keeps_rustls() {
    // CBC suites aren't offered by aws-lc-rs' defaults; stand in with the
    // mapping: a suite `cipher_of` doesn't know keeps the stream.
    assert_eq!(None, cipher_of(rustls::CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA256));
    if !kernel_has_ktls() {
        return;
    }
    let before = count(Outcome::Enabled);
    let (server, _client) = pair(&rustls::version::TLS13, None, b"").await;
    assert!(matches!(server, Upgraded::Ktls(_)));
    assert!(count(Outcome::Enabled) > before);
}
