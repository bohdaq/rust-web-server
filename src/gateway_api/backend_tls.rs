//! BackendTLSPolicy: which Service ports rws reaches over TLS, trusting
//! which CAs, under which name. A policy targets Services in its namespace
//! (optionally one port, by name); between two policies for the same target
//! the older wins and the other is `Conflicted`, and a port's own policy
//! beats one for the whole Service. The client configuration is built once
//! per distinct policy content and shared.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use kube::ResourceExt;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};

use super::resources::BackendTlsPolicy;
use super::translate::Snapshot;
use super::types::*;
use crate::core::x509::SubjectAltName;

/// How rws connects to a backend a policy covers.
#[derive(Clone)]
pub struct BackendTls {
    /// Identifies the content (CAs, hostname, SANs): the pool key.
    pub label: String,
    pub config: Arc<rustls::ClientConfig>,
    /// SNI, and the name verified when the policy lists no SANs.
    pub server_name: String,
    /// How the backend's certificate is checked (kept to add a client
    /// certificate, [`BackendTls::with_client_cert`]).
    verifier: Arc<dyn ServerCertVerifier>,
}

impl PartialEq for BackendTls {
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label
    }
}

impl std::fmt::Debug for BackendTls {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackendTls").field("label", &self.label).field("server_name", &self.server_name).finish()
    }
}

/// A policy's verdict: accepted (with its configuration) or why not.
#[derive(Clone, Debug, PartialEq)]
pub struct Evaluated {
    /// `Accepted`: `Ok`, or `Err((reason, message))`.
    pub accepted: Result<(), (&'static str, String)>,
    /// `ResolvedRefs`: `Ok`, or `Err((reason, message))`.
    pub resolved: Result<(), (&'static str, String)>,
    /// What connections use, when accepted.
    pub tls: Option<BackendTls>,
}

/// Every policy's verdict, by (namespace, name), oldest first.
pub fn evaluate(snapshot: &Snapshot) -> Vec<((String, String), Evaluated)> {
    let mut policies: Vec<&BackendTlsPolicy> = snapshot.backend_tls_policies.iter().collect();
    policies.sort_by_key(|p| (p.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string()).unwrap_or_default(), p.namespace(), p.name_any()));
    let mut claimed: Vec<(String, String, Option<String>)> = Vec::new();
    policies
        .into_iter()
        .map(|policy| {
            let namespace = policy.namespace().unwrap_or_default();
            let mut evaluated = evaluate_one(policy, &namespace, snapshot);
            let targets: Vec<(String, String, Option<String>)> = policy
                .spec
                .target_refs
                .iter()
                .filter(|t| t.group.is_empty() && t.kind == "Service")
                .map(|t| (namespace.clone(), t.name.clone(), t.section_name.clone()))
                .collect();
            if evaluated.accepted.is_ok() {
                if targets.iter().any(|t| claimed.contains(t)) {
                    evaluated.accepted = Err(("Conflicted", "An older BackendTLSPolicy targets the same Service and section".to_string()));
                    evaluated.tls = None;
                } else {
                    claimed.extend(targets);
                }
            }
            ((namespace, policy.name_any()), evaluated)
        })
        .collect()
}

fn evaluate_one(policy: &BackendTlsPolicy, namespace: &str, snapshot: &Snapshot) -> Evaluated {
    let validation = &policy.spec.validation;
    let fail = |accepted: &'static str, resolved: Option<(&'static str, String)>, message: String| Evaluated {
        accepted: Err((accepted, message.clone())),
        resolved: resolved.map_or(Ok(()), Err),
        tls: None,
    };
    let mut pem = Vec::new();
    for reference in &validation.ca_certificate_refs {
        if !reference.group.is_empty() || reference.kind != "ConfigMap" {
            let message = format!("{} isn't a supported CA certificate reference", reference.kind);
            return fail("NoValidCACertificate", Some(("InvalidKind", message.clone())), message);
        }
        let ca = snapshot
            .config_maps
            .iter()
            .find(|c| c.namespace().as_deref() == Some(namespace) && c.name_any() == reference.name)
            .and_then(|c| c.data.as_ref())
            .and_then(|d| d.get("ca.crt"))
            .filter(|ca| crate::core::x509::cert_info_from_pem(ca.as_bytes()).is_some());
        let Some(ca) = ca else {
            let message = format!("ConfigMap {namespace}/{} has no valid ca.crt", reference.name);
            return fail("NoValidCACertificate", Some(("InvalidCACertificateRef", message.clone())), message);
        };
        pem.extend_from_slice(ca.as_bytes());
        pem.push(b'\n');
    }
    let system = validation.well_known_ca_certificates.as_deref() == Some("System");
    if pem.is_empty() && !system {
        return fail("NoValidCACertificate", None, "Neither caCertificateRefs nor wellKnownCACertificates: System".to_string());
    }
    match build(if system && pem.is_empty() { None } else { Some(&pem) }, validation) {
        Ok(tls) => Evaluated { accepted: Ok(()), resolved: Ok(()), tls: Some(tls) },
        Err(e) => fail("NoValidCACertificate", Some(("InvalidCACertificateRef", e.clone())), e),
    }
}

/// The policy for a Service port: the one naming the port, else the one for
/// the whole Service. `None`: plain HTTP; `Some(Err)`: a policy covers it
/// but isn't usable — requests fail rather than go out in the clear.
pub fn for_service(evaluated: &[((String, String), Evaluated)], snapshot: &Snapshot, namespace: &str, service: &str, port_name: Option<&str>) -> Option<Result<BackendTls, String>> {
    let targets = |section: Option<&str>| {
        evaluated.iter().find(|(key, e)| {
            let policy = snapshot.backend_tls_policies.iter().find(|p| p.namespace().as_deref() == Some(key.0.as_str()) && p.name_any() == key.1);
            key.0 == namespace
                && policy.is_some_and(|p| p.spec.target_refs.iter().any(|t| t.group.is_empty() && t.kind == "Service" && t.name == service && t.section_name.as_deref() == section))
                && !matches!(&e.accepted, Err((reason, _)) if *reason == "Conflicted")
        })
    };
    let (_, found) = port_name.and_then(|p| targets(Some(p))).or_else(|| targets(None))?;
    Some(found.tls.clone().ok_or_else(|| found.accepted.as_ref().err().map(|(_, m)| m.clone()).unwrap_or_default()))
}

/// Client configurations by label, built once.
fn cache() -> &'static Mutex<HashMap<String, BackendTls>> {
    static CACHE: OnceLock<Mutex<HashMap<String, BackendTls>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// The client configuration for `validation`, trusting `ca_pem` (the
/// platform's roots when `None`).
pub fn build(ca_pem: Option<&[u8]>, validation: &BackendTlsValidation) -> Result<BackendTls, String> {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    ca_pem.hash(&mut hasher);
    validation.hostname.hash(&mut hasher);
    for san in &validation.subject_alt_names {
        (&san.san_type, &san.hostname, &san.uri).hash(&mut hasher);
    }
    let label = format!("backend-tls:{:016x}", hasher.finish());
    if let Some(found) = cache().lock().unwrap_or_else(|e| e.into_inner()).get(&label) {
        return Ok(found.clone());
    }
    let mut roots = rustls::RootCertStore::empty();
    match ca_pem {
        Some(pem) => {
            use rustls::pki_types::pem::PemObject;
            for cert in CertificateDer::pem_slice_iter(pem) {
                roots.add(cert.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            }
        }
        None => roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned()),
    }
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let webpki = rustls::client::WebPkiServerVerifier::builder(Arc::new(roots)).build().map_err(|e| e.to_string())?;
    let verifier: Arc<dyn ServerCertVerifier> = if validation.subject_alt_names.is_empty() {
        webpki
    } else {
        let sans = validation
            .subject_alt_names
            .iter()
            .filter_map(|s| match s.san_type.as_str() {
                "Hostname" => Some(SubjectAltName::Dns(s.hostname.to_ascii_lowercase())),
                "URI" => Some(SubjectAltName::Uri(s.uri.clone())),
                _ => None,
            })
            .collect();
        Arc::new(SanVerifier { webpki, sans })
    };
    let config = crate::tls_client_builder().dangerous().with_custom_certificate_verifier(Arc::clone(&verifier)).with_no_client_auth();
    let tls = BackendTls { label: label.clone(), config: Arc::new(config), server_name: validation.hostname.clone(), verifier };
    cache().lock().unwrap_or_else(|e| e.into_inner()).insert(label, tls.clone());
    Ok(tls)
}

impl BackendTls {
    /// The same, presenting a client certificate (the Gateway's
    /// `spec.tls.backend.clientCertificateRef`): PEM chain and key.
    pub fn with_client_cert(&self, chain_pem: &[u8], key_pem: &[u8]) -> Result<BackendTls, String> {
        use rustls::pki_types::pem::PemObject;
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (chain_pem, key_pem).hash(&mut hasher);
        let label = format!("{}+client:{:016x}", self.label, hasher.finish());
        if let Some(found) = cache().lock().unwrap_or_else(|e| e.into_inner()).get(&label) {
            return Ok(found.clone());
        }
        let chain = CertificateDer::pem_slice_iter(chain_pem).collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
        let key = rustls::pki_types::PrivateKeyDer::from_pem_slice(key_pem).map_err(|e| e.to_string())?;
        let config = crate::tls_client_builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::clone(&self.verifier))
            .with_client_auth_cert(chain, key)
            .map_err(|e| e.to_string())?;
        let tls = BackendTls { label: label.clone(), config: Arc::new(config), server_name: self.server_name.clone(), verifier: Arc::clone(&self.verifier) };
        cache().lock().unwrap_or_else(|e| e.into_inner()).insert(label, tls.clone());
        Ok(tls)
    }
}

/// Verifies the chain with the trusted CAs, then that the certificate
/// carries one of the policy's subjectAltNames (instead of the hostname).
#[derive(Debug)]
struct SanVerifier {
    webpki: Arc<rustls::client::WebPkiServerVerifier>,
    sans: Vec<SubjectAltName>,
}

/// Whether a certificate name (maybe `*.`) covers a wanted DNS name.
fn dns_matches(certificate: &str, wanted: &str) -> bool {
    let certificate = certificate.to_ascii_lowercase();
    match certificate.strip_prefix("*.") {
        Some(suffix) => wanted.split_once('.').is_some_and(|(label, rest)| !label.is_empty() && rest == suffix),
        None => certificate == wanted,
    }
}

impl ServerCertVerifier for SanVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        // The chain must verify; only the name check is replaced.
        match self.webpki.verify_server_cert(end_entity, intermediates, server_name, ocsp_response, now) {
            Ok(_) | Err(rustls::Error::InvalidCertificate(rustls::CertificateError::NotValidForName | rustls::CertificateError::NotValidForNameContext { .. })) => {}
            Err(e) => return Err(e),
        }
        let names = crate::core::x509::subject_alt_names(end_entity.as_ref()).unwrap_or_default();
        let matched = self.sans.iter().any(|wanted| {
            names.iter().any(|name| match (wanted, name) {
                (SubjectAltName::Dns(w), SubjectAltName::Dns(n)) => dns_matches(n, w),
                (SubjectAltName::Uri(w), SubjectAltName::Uri(n)) => w == n,
                _ => false,
            })
        });
        if matched {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::InvalidCertificate(rustls::CertificateError::NotValidForName))
        }
    }

    fn verify_tls12_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &rustls::DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.webpki.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &rustls::DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.webpki.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.webpki.supported_verify_schemes()
    }
}

#[cfg(test)]
mod tests;
