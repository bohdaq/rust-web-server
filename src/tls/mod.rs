use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use rustls::{RootCertStore, ServerConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::{ClientHello, ResolvesServerCert, WebPkiClientVerifier};
use rustls::sign::CertifiedKey;
use tokio_rustls::TlsAcceptor;

use crate::virtual_host::VirtualHostConfig;

// ── SNI certificate resolver ──────────────────────────────────────────────────

#[derive(Debug)]
pub struct SniCertResolver {
    /// Per-domain certificates, keyed by exact SNI hostname.
    certs: HashMap<String, Arc<CertifiedKey>>,
    /// Fallback used when SNI is absent or does not match any domain.
    default: Option<Arc<CertifiedKey>>,
}

impl SniCertResolver {
    fn build(
        vhosts: &[VirtualHostConfig],
        default_cert: &str,
        default_key: &str,
    ) -> Result<Self, String> {
        let mut certs = HashMap::new();

        for vh in vhosts {
            if vh.cert_file.is_empty() || vh.key_file.is_empty() {
                eprintln!("[TLS] virtual host '{}' has no cert/key — skipped", vh.domain);
                continue;
            }
            let ck = load_certified_key(&vh.cert_file, &vh.key_file)?;
            certs.insert(vh.domain.clone(), Arc::new(ck));
        }

        let default = if default_cert.is_empty() || default_key.is_empty() {
            None
        } else {
            Some(Arc::new(load_certified_key(default_cert, default_key)?))
        };

        Ok(SniCertResolver { certs, default })
    }
}

impl ResolvesServerCert for SniCertResolver {
    fn resolve(&self, hello: ClientHello) -> Option<Arc<CertifiedKey>> {
        hello.server_name()
            .and_then(|sni| self.certs.get(sni).cloned())
            .or_else(|| self.default.clone())
    }
}

// ── Public constructors ───────────────────────────────────────────────────────

/// Build a `TlsAcceptor` supporting multiple virtual hosts via SNI.
///
/// The `default_cert`/`default_key` pair is used when the client sends no SNI
/// or when the SNI hostname is not found in `vhosts`.  Pass an empty slice for
/// `vhosts` to behave identically to the single-cert path.
pub fn create_tls_acceptor_from_vhosts(
    vhosts: &[VirtualHostConfig],
    default_cert: &str,
    default_key: &str,
) -> Result<TlsAcceptor, String> {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let resolver = SniCertResolver::build(vhosts, default_cert, default_key)?;

    // mTLS: if RWS_CONFIG_TLS_CLIENT_CA_FILE is set, require a valid client cert.
    let ca_path = std::env::var(crate::entry_point::Config::RWS_CONFIG_TLS_CLIENT_CA_FILE)
        .unwrap_or_default();
    let mut config = if ca_path.is_empty() {
        ServerConfig::builder()
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(resolver))
    } else {
        let verifier = load_client_verifier(&ca_path)?;
        ServerConfig::builder()
            .with_client_cert_verifier(verifier)
            .with_cert_resolver(Arc::new(resolver))
    };

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(TlsAcceptor::from(Arc::new(config)))
}

/// Convenience wrapper for single-certificate deployments (backward compat).
pub fn create_tls_acceptor(cert_path: &str, key_path: &str) -> Result<TlsAcceptor, String> {
    create_tls_acceptor_from_vhosts(&[], cert_path, key_path)
}

#[cfg(feature = "http3")]
pub fn create_quinn_server_config_from_vhosts(
    vhosts: &[VirtualHostConfig],
    default_cert: &str,
    default_key: &str,
) -> Result<quinn::ServerConfig, String> {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let resolver = SniCertResolver::build(vhosts, default_cert, default_key)?;

    let ca_path = std::env::var(crate::entry_point::Config::RWS_CONFIG_TLS_CLIENT_CA_FILE)
        .unwrap_or_default();
    let mut tls_config = if ca_path.is_empty() {
        ServerConfig::builder()
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(resolver))
    } else {
        let verifier = load_client_verifier(&ca_path)?;
        ServerConfig::builder()
            .with_client_cert_verifier(verifier)
            .with_cert_resolver(Arc::new(resolver))
    };

    tls_config.max_early_data_size = u32::MAX;
    tls_config.alpn_protocols = vec![b"h3".to_vec()];

    let quic_config = quinn::crypto::rustls::QuicServerConfig::try_from(tls_config)
        .map_err(|e| format!("QUIC TLS config error: {}", e))?;

    Ok(quinn::ServerConfig::with_crypto(Arc::new(quic_config)))
}

#[cfg(feature = "http3")]
pub fn create_quinn_server_config(cert_path: &str, key_path: &str) -> Result<quinn::ServerConfig, String> {
    create_quinn_server_config_from_vhosts(&[], cert_path, key_path)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn load_certified_key(cert_path: &str, key_path: &str) -> Result<CertifiedKey, String> {
    let certs = load_certs(cert_path)?;
    let key = load_key(key_path)?;
    let signing_key = rustls::crypto::aws_lc_rs::sign::any_supported_type(&key)
        .map_err(|e| format!("unsupported key type in '{}': {}", key_path, e))?;
    Ok(CertifiedKey::new(certs, signing_key))
}

fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("failed to read cert file '{}': {}", path, e))?;
    let mut cursor = Cursor::new(bytes);
    rustls_pemfile::certs(&mut cursor)
        .map(|r| r.map(|c| c.into_owned()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("failed to parse certs from '{}': {}", path, e))
}

fn load_key(path: &str) -> Result<PrivateKeyDer<'static>, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("failed to read key file '{}': {}", path, e))?;
    let mut cursor = Cursor::new(bytes);
    rustls_pemfile::private_key(&mut cursor)
        .map_err(|e| format!("failed to parse key from '{}': {}", path, e))?
        .ok_or_else(|| format!("no private key found in '{}'", path))
        .map(|k| k.clone_key())
}

fn load_client_verifier(ca_path: &str) -> Result<Arc<dyn rustls::server::danger::ClientCertVerifier>, String> {
    let certs = load_certs(ca_path)?;
    let mut root_store = RootCertStore::empty();
    for cert in certs {
        root_store
            .add(cert)
            .map_err(|e| format!("invalid CA cert in '{}': {}", ca_path, e))?;
    }
    WebPkiClientVerifier::builder(Arc::new(root_store))
        .build()
        .map_err(|e| format!("client cert verifier build error: {}", e))
}
