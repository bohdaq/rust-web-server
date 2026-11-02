use std::collections::HashMap;
use std::sync::Arc;
use rustls::{RootCertStore, ServerConfig};
use rustls::pki_types::pem::{self, PemObject};
use rustls::pki_types::{CertificateDer, CertificateRevocationListDer, PrivateKeyDer};
use rustls::server::{ClientHello, ResolvesServerCert, WebPkiClientVerifier};
use rustls::sign::CertifiedKey;
use tokio_rustls::TlsAcceptor;

use crate::virtual_host::VirtualHostConfig;

pub mod internal;
#[cfg(feature = "pkcs11")]
pub mod pkcs11;

#[cfg(test)]
mod tests;

/// What ACME (`acme`, a layer above this one) plugs into TLS and the
/// listeners: TLS-ALPN-01 and on-demand certificates, HTTP-01 answers on the
/// redirect listener. `acme` sets them the first time it is used; without
/// them none of this happens (no ACME configured, or a build without it).
#[doc(hidden)]
pub struct AcmeHooks {
    pub on_demand_enabled: fn() -> bool,
    pub on_demand_ensure: for<'a> fn(&'a str) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>>,
    pub on_demand_certificate: fn(&str) -> Option<Arc<CertifiedKey>>,
    pub tls_alpn_enabled: fn() -> bool,
    pub is_challenge: fn(&ClientHello<'_>) -> bool,
    pub challenge_certificate: fn(&str) -> Option<Arc<CertifiedKey>>,
    pub note_tls_listener: fn(),
    pub note_redirect_listener: fn(u16),
    pub http_challenge: for<'a> fn(&'a str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<String>> + Send + 'a>>,
}

static ACME_HOOKS: std::sync::OnceLock<AcmeHooks> = std::sync::OnceLock::new();

/// Install [`AcmeHooks`] (once; later calls are ignored).
#[doc(hidden)]
pub fn set_acme_hooks(hooks: AcmeHooks) {
    let _ = ACME_HOOKS.set(hooks);
}

/// The installed [`AcmeHooks`], if ACME has been used.
#[doc(hidden)]
pub fn acme_hooks() -> Option<&'static AcmeHooks> {
    ACME_HOOKS.get()
}

/// The TLS-ALPN-01 protocol (RFC 8737): a handshake that negotiated it was
/// the challenge answer, nothing more.
#[doc(hidden)]
pub const ACME_TLS_ALPN: &[u8] = b"acme-tls/1";

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

        let leaves = certs.iter().map(|(name, key)| (name.clone(), key)).chain(default.iter().map(|key| ("default".to_string(), key)));
        crate::metrics::tls::set_configured_certificates(leaves.filter_map(|(name, key)| Some((name, key.cert.first()?.as_ref()))));

        Ok(SniCertResolver { certs, default })
    }
}

impl ResolvesServerCert for SniCertResolver {
    fn resolve(&self, hello: ClientHello) -> Option<Arc<CertifiedKey>> {
        // A TLS-ALPN-01 validation gets the challenge certificate or
        // nothing — never the real one (RFC 8737 §3).
        let acme = acme_hooks();
        if let Some(acme) = acme.filter(|acme| (acme.is_challenge)(&hello)) {
            return hello.server_name().and_then(acme.challenge_certificate);
        }
        let sni = hello.server_name();
        let configured = sni.and_then(|sni| self.certs.get(sni).cloned());
        let configured = configured.or_else(|| sni.zip(acme).and_then(|(sni, acme)| (acme.on_demand_certificate)(sni)));
        configured.or_else(|| self.default.clone())
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
    let _ = crate::crypto_provider().install_default();

    let resolver = SniCertResolver::build(vhosts, default_cert, default_key)?;

    // mTLS: if RWS_CONFIG_TLS_CLIENT_CA_FILE is set, require a valid client cert.
    let ca_path = std::env::var(crate::entry_point::Config::RWS_CONFIG_TLS_CLIENT_CA_FILE)
        .unwrap_or_default();
    let mut config = if ca_path.is_empty() {
        server_builder()?
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(resolver))
    } else {
        let verifier = load_client_verifier(&ca_path)?;
        server_builder()?
            .with_client_cert_verifier(verifier)
            .with_cert_resolver(Arc::new(resolver))
    };

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    if acme_hooks().is_some_and(|acme| (acme.tls_alpn_enabled)()) {
        config.alpn_protocols.push(ACME_TLS_ALPN.to_vec());
    }

    require_fips(config.fips())?;
    Ok(TlsAcceptor::from(Arc::new(config)))
}

/// A server config builder on [`crate::crypto_provider`] (FIPS-only
/// algorithms in a `fips` build), whatever provider is the process default.
fn server_builder() -> Result<rustls::ConfigBuilder<ServerConfig, rustls::WantsVerifier>, String> {
    ServerConfig::builder_with_provider(Arc::new(crate::crypto_provider())).with_safe_default_protocol_versions().map_err(|e| e.to_string())
}

/// In a `fips` build, refuse a TLS configuration rustls doesn't report as
/// FIPS-compliant (18.23.0).
#[doc(hidden)]
pub fn require_fips(compliant: bool) -> Result<(), String> {
    if crate::FIPS && !compliant {
        return Err("this is a FIPS build, but the TLS configuration isn't FIPS-compliant".to_string());
    }
    Ok(())
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
    let _ = crate::crypto_provider().install_default();

    let resolver = SniCertResolver::build(vhosts, default_cert, default_key)?;

    let ca_path = std::env::var(crate::entry_point::Config::RWS_CONFIG_TLS_CLIENT_CA_FILE)
        .unwrap_or_default();
    let mut tls_config = if ca_path.is_empty() {
        server_builder()?
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(resolver))
    } else {
        let verifier = load_client_verifier(&ca_path)?;
        server_builder()?
            .with_client_cert_verifier(verifier)
            .with_cert_resolver(Arc::new(resolver))
    };

    tls_config.max_early_data_size = u32::MAX;
    tls_config.alpn_protocols = vec![b"h3".to_vec()];
    require_fips(tls_config.fips())?;

    let quic_config = quinn::crypto::rustls::QuicServerConfig::try_from(tls_config)
        .map_err(|e| format!("QUIC TLS config error: {}", e))?;

    Ok(quinn::ServerConfig::with_crypto(Arc::new(quic_config)))
}

#[cfg(feature = "http3")]
pub fn create_quinn_server_config(cert_path: &str, key_path: &str) -> Result<quinn::ServerConfig, String> {
    create_quinn_server_config_from_vhosts(&[], cert_path, key_path)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// A certificate chain and key from PEM bytes (a Kubernetes TLS Secret's
/// `tls.crt` / `tls.key`).
#[cfg(feature = "gateway-api")]
#[doc(hidden)]
pub fn certified_key_from_pem(cert: &[u8], key: &[u8]) -> Result<CertifiedKey, String> {
    let certs = CertificateDer::pem_slice_iter(cert).collect::<Result<Vec<_>, _>>().map_err(|e| format!("certificate: {e}"))?;
    let key = PrivateKeyDer::from_pem_slice(key).map_err(|e| format!("key: {e}"))?;
    let signing_key = rustls::crypto::aws_lc_rs::sign::any_supported_type(&key).map_err(|e| format!("unsupported key: {e}"))?;
    Ok(CertifiedKey::new(certs, signing_key))
}

#[doc(hidden)]
pub fn load_certified_key(cert_path: &str, key_path: &str) -> Result<CertifiedKey, String> {
    let certs = load_certs(cert_path)?;
    // A key on a token: `pkcs11:` URI (18.23.0).
    if pkcs11_uri(key_path) {
        #[cfg(feature = "pkcs11")]
        return Ok(CertifiedKey::new(certs, pkcs11::signing_key(key_path)?));
        #[cfg(not(feature = "pkcs11"))]
        return Err(format!("'{key_path}' is a PKCS#11 URI, but this rws was built without the pkcs11 feature"));
    }
    let key = load_key(key_path)?;
    let signing_key = crate::crypto_provider()
        .key_provider
        .load_private_key(key)
        .map_err(|e| format!("unsupported key type in '{}': {}", key_path, e))?;
    Ok(CertifiedKey::new(certs, signing_key))
}

/// Whether a key setting is a `pkcs11:` URI rather than a file.
pub fn pkcs11_uri(key: &str) -> bool {
    key.trim_start().get(..7).is_some_and(|p| p.eq_ignore_ascii_case("pkcs11:"))
}

fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("failed to read cert file '{}': {}", path, e))?;
    CertificateDer::pem_slice_iter(&bytes)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("failed to parse certs from '{}': {}", path, e))
}

fn load_key(path: &str) -> Result<PrivateKeyDer<'static>, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("failed to read key file '{}': {}", path, e))?;
    // Accepts the first PKCS#8, PKCS#1 (RSA) or SEC1 (EC) key in the file.
    PrivateKeyDer::from_pem_slice(&bytes).map_err(|e| match e {
        pem::Error::NoItemsFound => format!("no private key found in '{}'", path),
        e => format!("failed to parse key from '{}': {}", path, e),
    })
}

fn load_client_verifier(ca_path: &str) -> Result<Arc<dyn rustls::server::danger::ClientCertVerifier>, String> {
    let certs = load_certs(ca_path)?;
    let mut root_store = RootCertStore::empty();
    for cert in certs {
        root_store
            .add(cert)
            .map_err(|e| format!("invalid CA cert in '{}': {}", ca_path, e))?;
    }
    let mut builder = WebPkiClientVerifier::builder_with_provider(Arc::new(root_store), Arc::new(crate::crypto_provider()));
    let crl_files = std::env::var("RWS_CONFIG_TLS_CLIENT_CRL_FILE").unwrap_or_default();
    let crls = load_crls(&crl_files)?;
    if !crls.is_empty() {
        builder = builder.with_crls(crls);
        if std::env::var("RWS_CONFIG_TLS_CLIENT_CRL_STRICT").is_ok_and(|v| v.trim().eq_ignore_ascii_case("true")) {
            // A CRL past its nextUpdate, or a certificate no CRL covers, fails.
            builder = builder.enforce_revocation_expiration();
        } else {
            // Only certificates whose CA has a CRL are checked.
            builder = builder.allow_unknown_revocation_status();
        }
    }
    builder.build().map_err(|e| format!("client cert verifier build error: {}", e))
}

/// Certificate revocation lists from `files` (comma-separated; each PEM
/// with any number of `X509 CRL` blocks, or one DER CRL), 18.20.0.
#[doc(hidden)]
pub fn load_crls(files: &str) -> Result<Vec<CertificateRevocationListDer<'static>>, String> {
    let mut crls = Vec::new();
    for path in files.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let bytes = std::fs::read(path).map_err(|e| format!("can't read CRL file '{path}': {e}"))?;
        let pem: Vec<_> = CertificateRevocationListDer::pem_slice_iter(&bytes).collect::<Result<_, _>>().map_err(|e| format!("CRL file '{path}': {e}"))?;
        if pem.is_empty() {
            // DER: an X.509 CRL is a SEQUENCE.
            if bytes.first() != Some(&0x30) {
                return Err(format!("CRL file '{path}' is neither PEM nor DER"));
            }
            crls.push(CertificateRevocationListDer::from(bytes));
        } else {
            crls.extend(pem);
        }
    }
    Ok(crls)
}
