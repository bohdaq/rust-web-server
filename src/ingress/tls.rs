//! In-cluster TLS client for the Kubernetes API server.
//!
//! Trusts only the service-account CA bundle mounted at
//! `/var/run/secrets/kubernetes.io/serviceaccount/ca.crt` — not the public
//! webpki root store `crate::http_client` uses, since the in-cluster API
//! server's certificate is signed by the cluster's own private CA, not a
//! publicly trusted one. This is why [`super::KubernetesIngressWatcher::from_service_account`]
//! can't just reuse `crate::http_client::Client`.
//!
//! Gated behind `any(feature = "http-client", feature = "http2")` — both
//! already pull in `rustls`; this module adds no new dependency. PEM
//! parsing is hand-rolled rather than using `rustls-pemfile` (only
//! available under the `http2` feature) so this also works under
//! `http-client` alone.

#![cfg(any(feature = "http-client", feature = "http2"))]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};

pub(crate) const SERVICE_ACCOUNT_DIR: &str = "/var/run/secrets/kubernetes.io/serviceaccount";

/// Everything needed to reach the in-cluster API server over TLS.
pub(crate) struct InClusterConfig {
    pub host: String,
    pub port: u16,
    pub token: String,
    pub namespace: String,
    pub client_config: Arc<ClientConfig>,
}

/// Load the token, namespace, and CA bundle from the mounted service
/// account directory, and the API server host/port from the
/// `KUBERNETES_SERVICE_HOST`/`KUBERNETES_SERVICE_PORT` environment
/// variables every pod has injected automatically — the same mechanism
/// every other Kubernetes client library uses to find the API server
/// from inside a pod, so no further configuration should be needed.
pub(crate) fn load() -> Result<InClusterConfig, String> {
    let host = std::env::var("KUBERNETES_SERVICE_HOST")
        .map_err(|_| "KUBERNETES_SERVICE_HOST is not set (not running inside a pod?)".to_string())?;
    let port: u16 = std::env::var("KUBERNETES_SERVICE_PORT")
        .unwrap_or_else(|_| "443".to_string())
        .parse()
        .map_err(|_| "KUBERNETES_SERVICE_PORT is not a valid port number".to_string())?;

    let token = std::fs::read_to_string(format!("{SERVICE_ACCOUNT_DIR}/token"))
        .map_err(|e| format!("failed to read service account token: {e}"))?
        .trim()
        .to_string();
    let namespace = std::fs::read_to_string(format!("{SERVICE_ACCOUNT_DIR}/namespace"))
        .unwrap_or_else(|_| "default".to_string())
        .trim()
        .to_string();
    let ca_pem = std::fs::read_to_string(format!("{SERVICE_ACCOUNT_DIR}/ca.crt"))
        .map_err(|e| format!("failed to read service account CA certificate: {e}"))?;

    let client_config = build_client_config(&ca_pem)?;
    Ok(InClusterConfig { host, port, token, namespace, client_config })
}

/// Build a `rustls::ClientConfig` trusting exactly the certificates found
/// in `ca_pem` — no other CA, public or otherwise, is trusted.
pub(crate) fn build_client_config(ca_pem: &str) -> Result<Arc<ClientConfig>, String> {
    // Idempotent: errors only if a *different* provider was already
    // installed, which we don't care about — some other TLS-using module
    // (e.g. `crate::tls`) may have already installed the same one.
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let certs = parse_pem_certificates(ca_pem)?;
    if certs.is_empty() {
        return Err("no certificates found in CA bundle".to_string());
    }
    let mut store = RootCertStore::empty();
    for cert in certs {
        store.add(cert).map_err(|e| format!("invalid CA certificate: {e}"))?;
    }
    Ok(Arc::new(
        ClientConfig::builder()
            .with_root_certificates(store)
            .with_no_client_auth(),
    ))
}

/// Extract every `-----BEGIN CERTIFICATE-----` block's base64 body and
/// decode it to DER, in encounter order. Ignores any other PEM block type
/// (e.g. a private key accidentally concatenated into the same file).
pub(crate) fn parse_pem_certificates(pem: &str) -> Result<Vec<CertificateDer<'static>>, String> {
    let mut certs = Vec::new();
    let mut lines = pem.lines();
    while let Some(line) = lines.next() {
        if line.trim() != "-----BEGIN CERTIFICATE-----" {
            continue;
        }
        let mut b64 = String::new();
        let mut closed = false;
        for l in lines.by_ref() {
            if l.trim() == "-----END CERTIFICATE-----" {
                closed = true;
                break;
            }
            b64.push_str(l.trim());
        }
        if !closed {
            return Err("unterminated CERTIFICATE block in PEM input".to_string());
        }
        certs.push(CertificateDer::from(base64_decode_standard(&b64)?));
    }
    Ok(certs)
}

fn base64_decode_standard(s: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4 + 1);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for ch in s.chars() {
        if ch == '=' {
            break;
        }
        let v: u32 = match ch {
            'A'..='Z' => ch as u32 - 'A' as u32,
            'a'..='z' => ch as u32 - 'a' as u32 + 26,
            '0'..='9' => ch as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => return Err(format!("invalid base64 character in PEM body: '{ch}'")),
        };
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}

/// Issue one GET request over a fresh TLS connection to `host:port`,
/// verifying the peer certificate against `client_config`'s trusted CA and
/// expecting it to be valid for `server_name`, and return the response
/// body as a string.
///
/// Fully parameterized (host/port/server_name are never hard-coded here)
/// so this is directly unit-testable against a local TLS listener — see
/// `tests.rs`. The real `kubernetes.default.svc` server name and
/// production host/port/CA come from [`load`].
pub(crate) fn https_get(
    host: &str,
    port: u16,
    server_name: &str,
    client_config: Arc<ClientConfig>,
    token: &str,
    path: &str,
    read_timeout: Duration,
) -> Result<String, String> {
    let addr = format!("{host}:{port}");
    let tcp = TcpStream::connect(&addr)
        .map_err(|e| format!("ingress watcher: connect to {addr} failed: {e}"))?;
    tcp.set_read_timeout(Some(read_timeout)).map_err(|e| e.to_string())?;
    tcp.set_write_timeout(Some(Duration::from_secs(5))).map_err(|e| e.to_string())?;

    let name = ServerName::try_from(server_name.to_string())
        .map_err(|e| format!("invalid server name '{server_name}': {e}"))?;
    let conn = ClientConnection::new(client_config, name)
        .map_err(|e| format!("TLS setup failed: {e}"))?;
    let mut stream = StreamOwned::new(conn, tcp);

    let auth_header = if token.is_empty() {
        String::new()
    } else {
        format!("Authorization: Bearer {token}\r\n")
    };
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {server_name}\r\n{auth_header}Accept: application/json\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("ingress watcher: TLS write failed: {e}"))?;

    let mut buf = Vec::with_capacity(8192);
    let mut tmp = [0u8; 4096];
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            // rustls treats the peer closing the TCP connection without a
            // TLS `close_notify` alert as an error rather than a clean EOF
            // (it can indicate response truncation by an active attacker),
            // surfaced as `io::ErrorKind::UnexpectedEof` — see
            // https://docs.rs/rustls/latest/rustls/manual/_03_howto/index.html#unexpected-eof,
            // which explicitly documents this as safe to tolerate for a
            // response like ours that's already self-delimited by
            // `Content-Length`/`Connection: close` at the HTTP layer, not
            // relying on the TLS-level close to know where the body ends.
            // Some load balancers/proxies (and, empirically, this crate's
            // own bare-bones test TLS server) don't bother sending it.
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(format!("ingress watcher: TLS read failed: {e}")),
        }
    }

    super::parse_http1_response(&buf)
}

/// Open a TLS connection to `host:port` and return the raw stream —
/// exposed separately from [`https_get`] so the watch loop
/// (`super::watch`) can issue a request and then keep reading a
/// long-lived response from the same connection.
pub(crate) fn tls_connect(
    host: &str,
    port: u16,
    server_name: &str,
    client_config: Arc<ClientConfig>,
    read_timeout: Duration,
) -> Result<StreamOwned<ClientConnection, TcpStream>, String> {
    let addr = format!("{host}:{port}");
    let tcp = TcpStream::connect(&addr)
        .map_err(|e| format!("ingress watcher: connect to {addr} failed: {e}"))?;
    tcp.set_read_timeout(Some(read_timeout)).map_err(|e| e.to_string())?;
    tcp.set_write_timeout(Some(Duration::from_secs(5))).map_err(|e| e.to_string())?;

    let name = ServerName::try_from(server_name.to_string())
        .map_err(|e| format!("invalid server name '{server_name}': {e}"))?;
    let conn = ClientConnection::new(client_config, name)
        .map_err(|e| format!("TLS setup failed: {e}"))?;
    Ok(StreamOwned::new(conn, tcp))
}
