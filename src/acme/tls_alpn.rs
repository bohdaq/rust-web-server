//! TLS-ALPN-01 (RFC 8737): the CA opens a TLS connection to the domain's
//! port 443 offering only the ALPN protocol `acme-tls/1`, and rws answers
//! with a self-signed certificate for the domain carrying the critical
//! `id-pe-acmeIdentifier` extension: SHA-256 of the key authorization. No
//! port 80, no DNS access.
//!
//! The certificates live in a process-wide store. Once the TLS listener is
//! up (renewals) its resolver ([`crate::tls`]) serves them to `acme-tls/1`
//! handshakes; before that (the first certificate) a temporary acceptor runs
//! on the same socket, which `main` hands over with [`set_startup_listener`].
//! `acme-tls/1` connections are closed after the handshake; ordinary
//! handshakes never see these certificates.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;

/// The ALPN protocol of TLS-ALPN-01 challenges.
pub const ALPN: &[u8] = b"acme-tls/1";

fn store() -> &'static RwLock<HashMap<String, Arc<CertifiedKey>>> {
    static STORE: OnceLock<RwLock<HashMap<String, Arc<CertifiedKey>>>> = OnceLock::new();
    STORE.get_or_init(Default::default)
}

/// Whether rws answers TLS-ALPN-01 challenges (`[acme] challenge =
/// "tls-alpn-01"` with domains): the TLS listener then offers `acme-tls/1`.
pub fn enabled() -> bool {
    super::ChallengeType::configured() == super::ChallengeType::TlsAlpn01
        && super::non_empty_env(crate::entry_point::Config::RWS_CONFIG_ACME_DOMAINS).is_some()
}

/// The challenge certificate for `server_name`, if a challenge for it is in
/// progress.
pub fn certificate(server_name: &str) -> Option<Arc<CertifiedKey>> {
    store().read().unwrap_or_else(|e| e.into_inner()).get(&server_name.to_ascii_lowercase()).cloned()
}

/// Whether a ClientHello is a TLS-ALPN-01 validation.
pub fn is_challenge(hello: &ClientHello) -> bool {
    hello.alpn().is_some_and(|mut protocols| protocols.any(|p| p == ALPN))
}

/// Serve `key_authorization` for `domain` until [`remove`].
pub(crate) fn add(domain: &str, key_authorization: &str) -> Result<(), String> {
    let certificate = challenge_certificate(domain, key_authorization)?;
    store().write().unwrap_or_else(|e| e.into_inner()).insert(domain.to_ascii_lowercase(), Arc::new(certificate));
    Ok(())
}

pub(crate) fn remove(domain: &str) {
    store().write().unwrap_or_else(|e| e.into_inner()).remove(&domain.to_ascii_lowercase());
}

/// The RFC 8737 §3 certificate: SAN = `domain`, critical acmeIdentifier =
/// SHA-256(key authorization), a fresh P-256 key.
pub(crate) fn challenge_certificate(domain: &str, key_authorization: &str) -> Result<CertifiedKey, String> {
    let digest = aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, key_authorization.as_bytes());
    let mut params = rcgen::CertificateParams::new(vec![domain.to_string()]).map_err(|e| format!("TLS-ALPN-01 certificate for {domain}: {e}"))?;
    params.custom_extensions = vec![rcgen::CustomExtension::new_acme_identifier(digest.as_ref())];
    let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).map_err(|e| e.to_string())?;
    let cert = params.self_signed(&key).map_err(|e| format!("TLS-ALPN-01 certificate for {domain}: {e}"))?;
    let key_der = rustls::pki_types::PrivateKeyDer::Pkcs8(key.serialize_der().into());
    let signing_key = rustls::crypto::aws_lc_rs::sign::any_supported_type(&key_der).map_err(|e| e.to_string())?;
    Ok(CertifiedKey::new(vec![cert.der().clone()], signing_key))
}

/// Set once the TLS listener serves (it then answers challenges itself).
static TLS_LISTENER_UP: AtomicBool = AtomicBool::new(false);

pub(crate) fn note_tls_listener() {
    TLS_LISTENER_UP.store(true, Ordering::SeqCst);
    // Release the startup socket's clone: per-core runtimes bind the port
    // anew (SO_REUSEPORT), which an open plain socket on it would block.
    startup_listener().lock().unwrap_or_else(|e| e.into_inner()).take();
}

pub(crate) fn tls_listener_up() -> bool {
    TLS_LISTENER_UP.load(Ordering::SeqCst)
}

fn startup_listener() -> &'static Mutex<Option<std::net::TcpListener>> {
    static LISTENER: OnceLock<Mutex<Option<std::net::TcpListener>>> = OnceLock::new();
    LISTENER.get_or_init(Default::default)
}

/// Hand ACME the bound HTTPS socket (a clone of it) before the server runs,
/// for answering the first TLS-ALPN-01 challenges: the port is taken, and
/// that is where the CA connects.
pub fn set_startup_listener(listener: std::net::TcpListener) {
    *startup_listener().lock().unwrap_or_else(|e| e.into_inner()) = Some(listener);
}

/// Serves only challenge certificates.
#[derive(Debug)]
struct ChallengesOnly;

impl ResolvesServerCert for ChallengesOnly {
    fn resolve(&self, hello: ClientHello) -> Option<Arc<CertifiedKey>> {
        if is_challenge(&hello) {
            hello.server_name().and_then(certificate)
        } else {
            None
        }
    }
}

/// A temporary listener answering TLS-ALPN-01 handshakes until `shutdown`:
/// on the startup socket when there is one, else bound to `addr`.
pub(crate) async fn serve(addr: String, shutdown: tokio::sync::oneshot::Receiver<()>) -> Result<(), String> {
    let std_listener = match startup_listener().lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        Some(listener) => Some(listener.try_clone().map_err(|e| e.to_string())?),
        None => None,
    };
    let listener = match std_listener {
        Some(listener) => {
            listener.set_nonblocking(true).map_err(|e| e.to_string())?;
            tokio::net::TcpListener::from_std(listener).map_err(|e| e.to_string())?
        }
        None => tokio::net::TcpListener::bind(&addr).await.map_err(|e| format!("TLS-ALPN-01 listener on {addr}: {e}"))?,
    };
    let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::aws_lc_rs::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_no_client_auth()
        .with_cert_resolver(Arc::new(ChallengesOnly));
    config.alpn_protocols = vec![ALPN.to_vec()];
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = &mut shutdown => return Ok(()),
            accepted = listener.accept() => {
                let Ok((tcp, _)) = accepted else { continue };
                let acceptor = acceptor.clone();
                tokio::spawn(async move {
                    // The handshake is the whole exchange; then close.
                    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), acceptor.accept(tcp)).await;
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_challenge_certificate_carries_the_critical_acme_identifier() {
        let certified = challenge_certificate("example.test", "token.thumbprint").unwrap();
        let der = certified.cert[0].as_ref();
        let digest = aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, b"token.thumbprint");
        // id-pe-acmeIdentifier 1.3.6.1.5.5.7.1.31, critical, OCTET STRING of
        // the 32-byte digest.
        let oid = [0x06, 0x08, 0x2B, 0x06, 0x01, 0x05, 0x05, 0x07, 0x01, 0x1F];
        let at = der.windows(oid.len()).position(|w| w == oid).expect("acmeIdentifier extension");
        let rest = &der[at + oid.len()..];
        assert_eq!(&[0x01, 0x01, 0xFF], &rest[..3], "critical");
        assert_eq!(&[0x04, 0x22, 0x04, 0x20], &rest[3..7], "extnValue: an OCTET STRING holding an OCTET STRING of 32 bytes");
        assert_eq!(digest.as_ref(), &rest[7..39]);
        assert!(der.windows(b"example.test".len()).any(|w| w == b"example.test"), "SAN");
    }

    #[test]
    fn the_store_is_keyed_by_lower_case_name() {
        add("Mixed.Example.Test", "a.b").unwrap();
        assert!(certificate("mixed.example.test").is_some());
        remove("MIXED.example.test");
        assert!(certificate("mixed.example.test").is_none());
    }
}
