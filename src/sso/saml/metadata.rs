//! IdP metadata: entity ID, SSO endpoint, and signing certificate.

use rsa::pkcs8::DecodePublicKey;
use rsa::RsaPublicKey;

use super::der::extract_subject_public_key_info;
use super::xml;
use crate::sso::pkce::base64url_decode;
use crate::sso::SsoError;

/// Parsed IdP SAML metadata — everything the SP needs to build an
/// `AuthnRequest` and verify a signed `Response`.
pub struct SamlIdpMetadata {
    /// The IdP's `entityID`, used as the expected `Issuer` on responses.
    pub entity_id: String,
    /// The IdP's SSO endpoint (`SingleSignOnService/@Location`) the SP
    /// redirects the user to. Prefers an `HTTP-POST` binding entry (this SP
    /// only implements the POST binding for `AuthnRequest`s — see
    /// [`super::SamlSp`]'s module docs); falls back to the first
    /// `SingleSignOnService` entry found if no POST binding is advertised.
    pub sso_url: String,
    /// The IdP's signing certificate's RSA public key, used to verify
    /// `Response`/`Assertion` signatures. Only RSA signing certificates are
    /// supported (see [`super::assertion`]'s module docs).
    pub signing_key: RsaPublicKey,
}

const POST_BINDING: &str = "urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST";

impl SamlIdpMetadata {
    /// Parse IdP metadata already read into memory (e.g. from a file).
    pub fn from_str(xml_str: &str) -> Result<Self, SsoError> {
        let root = xml::parse(xml_str).map_err(SsoError)?;
        let entity_descriptor = if root.name == "EntityDescriptor" { &root } else { root.find("EntityDescriptor").ok_or_else(|| SsoError("metadata is missing EntityDescriptor".into()))? };
        let entity_id = entity_descriptor
            .attr("entityID")
            .ok_or_else(|| SsoError("EntityDescriptor is missing entityID".into()))?
            .to_string();

        let idp_descriptor = entity_descriptor
            .find("IDPSSODescriptor")
            .ok_or_else(|| SsoError("metadata is missing IDPSSODescriptor".into()))?;

        let mut sso_services = Vec::new();
        idp_descriptor.find_all("SingleSignOnService", &mut sso_services);
        if sso_services.is_empty() {
            return Err(SsoError("metadata has no SingleSignOnService".into()));
        }
        let sso_url = sso_services
            .iter()
            .find(|s| s.attr("Binding") == Some(POST_BINDING))
            .or_else(|| sso_services.first())
            .and_then(|s| s.attr("Location"))
            .ok_or_else(|| SsoError("SingleSignOnService is missing Location".into()))?
            .to_string();

        let mut key_descriptors = Vec::new();
        idp_descriptor.find_all("KeyDescriptor", &mut key_descriptors);
        let signing_descriptor = key_descriptors
            .iter()
            .find(|k| k.attr("use") == Some("signing"))
            .or_else(|| key_descriptors.first())
            .ok_or_else(|| SsoError("metadata has no KeyDescriptor with a signing certificate".into()))?;
        let cert_b64 = signing_descriptor
            .find("X509Certificate")
            .map(|n| n.text.clone())
            .ok_or_else(|| SsoError("KeyDescriptor is missing X509Certificate".into()))?;
        let cert_der = decode_base64_flexible(&cert_b64)?;
        let spki_der = extract_subject_public_key_info(&cert_der).map_err(SsoError)?;
        let signing_key = RsaPublicKey::from_public_key_der(&spki_der)
            .map_err(|e| SsoError(format!("signing certificate does not contain an RSA public key: {e}")))?;

        Ok(SamlIdpMetadata { entity_id, sso_url, signing_key })
    }

    /// Read and parse IdP metadata from a local file.
    pub fn from_file(path: &str) -> Result<Self, SsoError> {
        let contents = std::fs::read_to_string(path).map_err(|e| SsoError(format!("failed to read {path}: {e}")))?;
        Self::from_str(&contents)
    }

    /// Fetch and parse IdP metadata from a URL.
    pub fn from_url(url: &str) -> Result<Self, SsoError> {
        let resp = crate::http_client::Client::new()
            .get(url)
            .timeout_ms(10_000)
            .send()
            .map_err(|e| SsoError(format!("metadata fetch failed: {e}")))?;
        if !resp.is_success() {
            return Err(SsoError(format!("metadata endpoint returned {}", resp.status())));
        }
        let body = resp.text().map_err(|e| SsoError(e.to_string()))?;
        Self::from_str(&body)
    }
}

/// X.509 certificates in metadata are standard base64 (not base64url) and
/// commonly wrap at 64/76 columns — strip whitespace before decoding.
/// `base64url_decode` already accepts the `+`/`/` alphabet as aliases for
/// `-`/`_`, so it doubles as a standard-base64 decoder once whitespace is
/// removed.
pub(crate) fn decode_base64_flexible(s: &str) -> Result<Vec<u8>, SsoError> {
    let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    base64url_decode(&cleaned).map_err(SsoError)
}
