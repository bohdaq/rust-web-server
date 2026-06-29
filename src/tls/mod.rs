use std::io::Cursor;
use std::sync::Arc;
use rustls::ServerConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio_rustls::TlsAcceptor;

pub fn create_tls_acceptor(cert_path: &str, key_path: &str) -> Result<TlsAcceptor, String> {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let certs = load_certs(cert_path)?;
    let key = load_key(key_path)?;

    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| format!("TLS config error: {}", e))?;

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(TlsAcceptor::from(Arc::new(config)))
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
