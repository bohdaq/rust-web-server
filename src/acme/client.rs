/// Minimal async HTTPS client for the ACME protocol.
///
/// Creates one TCP+TLS connection per request — ACME interactions are
/// infrequent, so connection reuse is not worth the complexity.

use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::rustls::{self, RootCertStore};
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::TlsConnector;

/// RFC 8555 §6.1: "ACME clients MUST send a User-Agent header field".
/// Pebble rejects requests without one; Let's Encrypt uses it for diagnostics.
pub(crate) const USER_AGENT: &str = concat!(
    "rws-acme/", env!("CARGO_PKG_VERSION"), " (+https://github.com/bohdaq/rust-web-server)"
);

pub struct AcmeHttpClient {
    connector: TlsConnector,
}

pub struct HttpResp {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl HttpResp {
    pub fn header(&self, name: &str) -> Option<&str> {
        let lower = name.to_ascii_lowercase();
        self.headers.iter()
            .find(|(k, _)| k == &lower)
            .map(|(_, v)| v.as_str())
    }

    pub fn nonce(&self) -> Option<String> {
        self.header("replay-nonce").map(|s| s.to_string())
    }

    pub fn location(&self) -> Option<String> {
        self.header("location").map(|s| s.to_string())
    }
}

/// Trust every certificate in the PEM file at `path` in addition to the
/// public roots — for a private ACME CA (`RWS_CONFIG_ACME_CA_FILE`).
pub(crate) fn add_extra_roots(store: &mut RootCertStore, path: &str) -> Result<(), String> {
    use tokio_rustls::rustls::pki_types::{pem::PemObject, CertificateDer};
    let bytes = std::fs::read(path)
        .map_err(|e| format!("failed to read ACME CA file '{path}': {e}"))?;
    let certs = CertificateDer::pem_slice_iter(&bytes)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("failed to parse ACME CA file '{path}': {e}"))?;
    if certs.is_empty() {
        return Err(format!("no certificates found in ACME CA file '{path}'"));
    }
    for cert in certs {
        store.add(cert).map_err(|e| format!("invalid CA certificate in '{path}': {e}"))?;
    }
    Ok(())
}

impl AcmeHttpClient {
    #[cfg(test)]
    pub(crate) fn with_roots(root_store: RootCertStore) -> Self {
        let config = crate::tls_client_builder().with_root_certificates(root_store).with_no_client_auth();
        AcmeHttpClient { connector: TlsConnector::from(Arc::new(config)) }
    }

    pub fn new() -> Result<Self, String> {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let mut root_store = RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        };
        if let Some(ca_file) = std::env::var(crate::entry_point::Config::RWS_CONFIG_ACME_CA_FILE)
            .ok()
            .filter(|v| !v.trim().is_empty())
        {
            add_extra_roots(&mut root_store, ca_file.trim())?;
        }
        let config = crate::tls_client_builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        Ok(AcmeHttpClient {
            connector: TlsConnector::from(Arc::new(config)),
        })
    }

    async fn connect(&self, host: &str, port: u16) -> Result<tokio_rustls::client::TlsStream<TcpStream>, String> {
        let tcp = TcpStream::connect(format!("{host}:{port}"))
            .await
            .map_err(|e| format!("TCP connect to {host}:{port} failed: {e}"))?;
        let server_name = ServerName::try_from(host)
            .map_err(|e| format!("invalid DNS name '{host}': {e}"))?
            .to_owned();
        self.connector.connect(server_name, tcp)
            .await
            .map_err(|e| format!("TLS handshake with {host} failed: {e}"))
    }

    pub async fn head(&self, url: &str) -> Result<HttpResp, String> {
        let (host, port, path) = parse_https_url(url)?;
        let authority = host_header(&host, port);
        let mut stream = self.connect(&host, port).await?;
        let req = format!(
            "HEAD {path} HTTP/1.1\r\nHost: {authority}\r\nUser-Agent: {USER_AGENT}\r\nConnection: close\r\n\r\n"
        );
        stream.write_all(req.as_bytes()).await.map_err(|e| e.to_string())?;
        // No shutdown() before reading: Let's Encrypt's front end answers
        // nothing to a HEAD whose client has already sent close_notify and a
        // TCP FIN, so every newNonce failed against the real CA (Pebble
        // tolerates it). `Connection: close` and the head-only read are enough.
        read_response_head_only(&mut stream).await
    }

    pub async fn get(&self, url: &str) -> Result<HttpResp, String> {
        let (host, port, path) = parse_https_url(url)?;
        let authority = host_header(&host, port);
        let mut stream = self.connect(&host, port).await?;
        let req = format!(
            "GET {path} HTTP/1.1\r\nHost: {authority}\r\nUser-Agent: {USER_AGENT}\r\nConnection: close\r\n\r\n"
        );
        stream.write_all(req.as_bytes()).await.map_err(|e| e.to_string())?;
        read_response(&mut stream).await
    }

    pub async fn post_jws(&self, url: &str, body: &str) -> Result<HttpResp, String> {
        let (host, port, path) = parse_https_url(url)?;
        let authority = host_header(&host, port);
        let mut stream = self.connect(&host, port).await?;
        let req = format!(
            "POST {path} HTTP/1.1\r\n\
             Host: {authority}\r\n\
             User-Agent: {USER_AGENT}\r\n\
             Content-Type: application/jose+json\r\n\
             Content-Length: {len}\r\n\
             Connection: close\r\n\r\n\
             {body}",
            len = body.len(),
        );
        stream.write_all(req.as_bytes()).await.map_err(|e| e.to_string())?;
        read_response(&mut stream).await
    }
}

// ── URL parsing ───────────────────────────────────────────────────────────────

/// Split `https://host[:port]/path` → `(host, port, "/path")`.
/// `Host` header value: RFC 9110 §7.2 requires the port whenever it isn't
/// the scheme default — ACME servers (Pebble, step-ca on :9000) build the
/// URLs in their responses from it.
pub(crate) fn host_header(host: &str, port: u16) -> String {
    if port == 443 { host.to_string() } else { format!("{host}:{port}") }
}

fn parse_https_url(url: &str) -> Result<(String, u16, String), String> {
    let stripped = url.trim_start_matches("https://");
    let (hostport, path_rest) = stripped.split_once('/').unwrap_or((stripped, ""));
    let path = format!("/{}", path_rest);
    let (host, port) = if let Some(colon) = hostport.rfind(':') {
        let p = hostport[colon + 1..].parse::<u16>().unwrap_or(443);
        (hostport[..colon].to_string(), p)
    } else {
        (hostport.to_string(), 443u16)
    };
    if host.is_empty() {
        return Err(format!("empty host in URL: {url}"));
    }
    Ok((host, port, path))
}

// ── response reading ──────────────────────────────────────────────────────────

async fn read_response<S: AsyncReadExt + Unpin>(stream: &mut S) -> Result<HttpResp, String> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 8192];
    loop {
        match stream.read(&mut tmp).await {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(_) => break,
        }
    }
    parse_http_response(&buf)
}

async fn read_response_head_only<S: AsyncReadExt + Unpin>(stream: &mut S) -> Result<HttpResp, String> {
    // For HEAD we only need headers; stop after the blank line.
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        match stream.read(&mut tmp).await {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") { break; }
            }
            Err(_) => break,
        }
    }
    parse_http_response(&buf)
}

fn parse_http_response(buf: &[u8]) -> Result<HttpResp, String> {
    let raw = String::from_utf8_lossy(buf);
    let sep = raw.find("\r\n\r\n").unwrap_or(raw.len());
    let header_part = &raw[..sep];
    let body = if sep + 4 < raw.len() {
        // strip chunked encoding if present — just take everything after the blank line
        let body_raw = &raw[sep + 4..];
        // If first line looks like a hex chunk size, strip the framing
        if let Some(nl) = body_raw.find("\r\n") {
            let first = body_raw[..nl].trim();
            if first.chars().all(|c| c.is_ascii_hexdigit()) && !first.is_empty() {
                // chunked: collect all chunks
                let mut out = String::new();
                let mut rest = &body_raw[nl + 2..];
                loop {
                    let nl2 = rest.find("\r\n").unwrap_or(rest.len());
                    let size_s = rest[..nl2].trim();
                    let size = usize::from_str_radix(size_s, 16).unwrap_or(0);
                    if size == 0 { break; }
                    let data_start = nl2 + 2;
                    if data_start + size <= rest.len() {
                        out.push_str(&rest[data_start..data_start + size]);
                        rest = &rest[data_start + size + 2..]; // skip trailing \r\n
                    } else { break; }
                }
                out
            } else {
                body_raw.to_string()
            }
        } else {
            body_raw.to_string()
        }
    } else {
        String::new()
    };

    let mut lines = header_part.lines();
    let status_line = lines.next().unwrap_or("");
    let status = status_line.split_whitespace().nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0u16);
    let headers: Vec<(String, String)> = lines.filter_map(|l| {
        let colon = l.find(':')?;
        Some((l[..colon].trim().to_ascii_lowercase(), l[colon + 1..].trim().to_string()))
    }).collect();

    Ok(HttpResp { status, headers, body })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

    // A TLS server that behaves like Let's Encrypt's front end: it reads the
    // request head, and if the client then closes its side (close_notify /
    // FIN) before the reply, it drops the request without answering.
    async fn nonce_server() -> (u16, RootCertStore) {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let der = CertificateDer::from(cert.cert.der().to_vec());
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der()));
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let config = rustls::ServerConfig::builder().with_no_client_auth().with_single_cert(vec![der.clone()], key).unwrap();
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut tls = acceptor.accept(tcp).await.unwrap();
            let mut buf = Vec::new();
            let mut tmp = [0u8; 1024];
            while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = tls.read(&mut tmp).await.unwrap();
                if n == 0 { return; }
                buf.extend_from_slice(&tmp[..n]);
            }
            // Did the client hang up already?
            match tokio::time::timeout(std::time::Duration::from_millis(200), tls.read(&mut tmp)).await {
                Ok(Ok(0)) | Ok(Err(_)) => return,
                _ => {}
            }
            let _ = tls.write_all(b"HTTP/1.1 200 OK\r\nReplay-Nonce: test-nonce\r\nConnection: close\r\n\r\n").await;
            let _ = tls.shutdown().await;
        });
        let mut roots = RootCertStore::empty();
        roots.add(der).unwrap();
        (port, roots)
    }

    #[tokio::test]
    async fn head_reads_the_reply_before_closing() {
        let (port, roots) = nonce_server().await;
        let client = AcmeHttpClient::with_roots(roots);
        let resp = client.head(&format!("https://localhost:{port}/acme/new-nonce")).await.unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.nonce().as_deref(), Some("test-nonce"));
    }

    #[tokio::test]
    async fn head_to_a_closed_port_is_an_error() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let client = AcmeHttpClient::with_roots(RootCertStore::empty());
        assert!(client.head(&format!("https://localhost:{port}/acme/new-nonce")).await.is_err());
    }
}
