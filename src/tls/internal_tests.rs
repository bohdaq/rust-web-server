use super::*;
use crate::test_env::TempPath;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use std::sync::Arc;

/// A TLS handshake to a server using `chain`/`key`, from a client that
/// trusts only `ca_pem`, for `name`.
fn handshake(ca_pem: &str, chain: &str, key: &str, name: &str) -> Result<(), String> {
    let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_slice_iter(chain.as_bytes()).collect::<Result<_, _>>().map_err(|e| e.to_string())?;
    let key = PrivateKeyDer::from_pem_slice(key.as_bytes()).map_err(|e| e.to_string())?;
    let server = rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::aws_lc_rs::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| e.to_string())?;
    let mut roots = rustls::RootCertStore::empty();
    for ca in CertificateDer::pem_slice_iter(ca_pem.as_bytes()) {
        roots.add(ca.unwrap()).unwrap();
    }
    let client = crate::tls_client_builder().with_root_certificates(roots).with_no_client_auth();
    let mut c = rustls::ClientConnection::new(Arc::new(client), ServerName::try_from(name.to_string()).unwrap()).unwrap();
    let mut s = rustls::ServerConnection::new(Arc::new(server)).unwrap();
    for _ in 0..10 {
        let mut buf = Vec::new();
        c.write_tls(&mut buf).unwrap();
        s.read_tls(&mut &buf[..]).unwrap();
        s.process_new_packets().map_err(|e| e.to_string())?;
        buf.clear();
        s.write_tls(&mut buf).unwrap();
        c.read_tls(&mut &buf[..]).unwrap();
        c.process_new_packets().map_err(|e| e.to_string())?;
        if !c.is_handshaking() && !s.is_handshaking() {
            return Ok(());
        }
    }
    Err("handshake didn't finish".into())
}

#[test]
fn the_ca_is_created_once_and_reused() {
    let dir = TempPath::new("tls-internal-ca");
    let first = LocalCa::load_or_create(&dir).unwrap();
    assert!(first.cert_pem().contains("BEGIN CERTIFICATE"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(dir.join("ca.key")).unwrap().permissions().mode() & 0o777, 0o600);
    }
    let again = LocalCa::load_or_create(&dir).unwrap();
    assert_eq!(first.cert_pem(), again.cert_pem());
    std::fs::remove_dir_all(&*dir).ok();
}

#[test]
fn issued_certificates_are_trusted_through_the_ca_for_their_names_only() {
    let dir = TempPath::new("tls-internal-issue");
    let ca = LocalCa::load_or_create(&dir).unwrap();
    let names: Vec<String> = ["localhost", "app.localhost", "127.0.0.1", "::1", "*.app.test"].iter().map(|s| s.to_string()).collect();
    let (chain, key) = ca.issue(&names, LEAF_LIFETIME).unwrap();
    assert_eq!(chain.matches("BEGIN CERTIFICATE").count(), 2, "leaf + CA");
    for name in ["localhost", "app.localhost", "127.0.0.1", "::1", "api.app.test"] {
        handshake(ca.cert_pem(), &chain, &key, name).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
    assert!(handshake(ca.cert_pem(), &chain, &key, "example.com").is_err(), "a name not on the certificate");

    // An issuer rebuilt from disk signs certificates the same CA verifies.
    let reloaded = LocalCa::load_or_create(&dir).unwrap();
    let (chain, key) = reloaded.issue(&names, LEAF_LIFETIME).unwrap();
    handshake(ca.cert_pem(), &chain, &key, "localhost").unwrap();

    // Another CA's client doesn't trust it.
    let other_dir = TempPath::new("tls-internal-other");
    let other = LocalCa::load_or_create(&other_dir).unwrap();
    assert!(handshake(other.cert_pem(), &chain, &key, "localhost").is_err());
    std::fs::remove_dir_all(&*dir).ok();
    std::fs::remove_dir_all(&*other_dir).ok();
}

#[test]
fn bad_names_are_refused() {
    let dir = TempPath::new("tls-internal-bad");
    let ca = LocalCa::load_or_create(&dir).unwrap();
    for bad in ["bad name", "a\u{0}b", "*.localhost", "-x.test", "", "x..test"] {
        assert!(ca.issue(&[bad.to_string()], LEAF_LIFETIME).is_err(), "{bad:?}");
    }
    assert!(ca.issue(&["*.dev.test".to_string(), "xn--bcher-kva.test".to_string()], LEAF_LIFETIME).is_ok());
    std::fs::remove_dir_all(&*dir).ok();
}

#[test]
fn names_cover_loopback_listen_address_vhosts_and_extras() {
    let _g = crate::test_env::lock();
    std::env::set_var(crate::entry_point::Config::RWS_CONFIG_IP, "192.168.1.20");
    std::env::set_var("RWS_CONFIG_VIRTUAL_HOST_0_DOMAIN", "Shop.Test");
    std::env::set_var(RWS_CONFIG_TLS_INTERNAL_NAMES, "api.test, localhost,");
    let n = names();
    std::env::remove_var("RWS_CONFIG_VIRTUAL_HOST_0_DOMAIN");
    std::env::remove_var(RWS_CONFIG_TLS_INTERNAL_NAMES);
    std::env::set_var(crate::entry_point::Config::RWS_CONFIG_IP, "0.0.0.0");
    assert_eq!(n, ["localhost", "127.0.0.1", "::1", "192.168.1.20", "shop.test", "api.test"]);
    assert!(!names().contains(&"0.0.0.0".to_string()));
    crate::server_config::invalidate();
}

#[test]
fn the_default_directory_is_per_user() {
    let d = default_dir();
    assert!(d.ends_with(Path::new("rws").join("pki")), "{}", d.display());
}

#[test]
fn trust_commands_are_planned_for_this_system() {
    let ca = Path::new("/tmp/rws-ca.pem");
    let add = trust_steps(true, ca);
    let remove = trust_steps(false, ca);
    assert_eq!(add.len(), remove.len());
    #[cfg(target_os = "linux")]
    if Path::new("/usr/local/share/ca-certificates").is_dir() {
        assert!(add.iter().any(|s| s.args.iter().any(|a| a == "/usr/local/share/ca-certificates/rws-local-ca.crt")), "{add:?}");
        assert!(add.iter().any(|s| s.program == "update-ca-certificates" || s.args.first().is_some_and(|a| a == "update-ca-certificates")), "{add:?}");
    }
    assert!(trust_command(&["rws".to_string(), "serve".to_string()]).is_none());
}

#[test]
fn start_from_env_points_tls_at_the_issued_certificate() {
    let _g = crate::test_env::lock();
    let dir = TempPath::new("tls-internal-start");
    let cert_var = crate::entry_point::Config::RWS_CONFIG_TLS_CERT_FILE;
    let key_var = crate::entry_point::Config::RWS_CONFIG_TLS_KEY_FILE;
    let saved: Vec<(&str, Option<String>)> = [cert_var, key_var, RWS_CONFIG_TLS, RWS_CONFIG_TLS_INTERNAL_DIR, crate::entry_point::Config::RWS_CONFIG_ACME_DOMAINS]
        .iter()
        .map(|v| (*v, std::env::var(v).ok()))
        .collect();
    std::env::set_var(cert_var, "");
    std::env::set_var(key_var, "");
    std::env::set_var(crate::entry_point::Config::RWS_CONFIG_ACME_DOMAINS, "");
    std::env::set_var(RWS_CONFIG_TLS, "internal");
    std::env::set_var(RWS_CONFIG_TLS_INTERNAL_DIR, dir.as_str());
    start_from_env();
    let cert = std::env::var(cert_var).unwrap();
    let key = std::env::var(key_var).unwrap();
    for (var, value) in saved {
        match value {
            Some(v) => std::env::set_var(var, v),
            None => std::env::remove_var(var),
        }
    }
    crate::server_config::invalidate();
    assert_eq!(Path::new(&cert), dir.join("localhost.pem"));
    let ca = std::fs::read_to_string(dir.join("ca.pem")).unwrap();
    handshake(&ca, &std::fs::read_to_string(&cert).unwrap(), &std::fs::read_to_string(&key).unwrap(), "localhost").unwrap();
    std::fs::remove_dir_all(&*dir).ok();
}
