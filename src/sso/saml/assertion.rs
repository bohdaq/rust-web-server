//! SAML `Response`/`Assertion` parsing, signature verification, and
//! validation.
//!
//! # Signature verification: byte-exact, not full XML C14N
//!
//! Correct XML-DSig verification canonicalizes (C14N) both the
//! `<ds:SignedInfo>` element (before checking `<ds:SignatureValue>` over
//! it) and the signed element itself (before checking the `Reference`'s
//! `<ds:DigestValue>` over it) — canonicalization exists so that
//! reformatting whitespace, attribute order, or namespace declarations
//! doesn't invalidate a signature that's semantically unchanged.
//!
//! This module does **not** implement C14N. It verifies both the
//! `SignedInfo` signature and the assertion digest against the *literal
//! byte ranges of the original document as transmitted* — no
//! reformatting, no reserialization. This is a deliberate scope decision,
//! not an oversight: implementing exclusive C14N correctly (namespace
//! inheritance, attribute ordering, comment stripping, ...) is a large,
//! easy-to-get-subtly-wrong undertaking in its own right, and in the
//! browser-POST SAML flow this SP implements, the IdP signs the exact
//! bytes it transmits — there is no intermediary reformatting the XML
//! between signing and receipt. The practical effect of this
//! simplification is **fail-closed, not fail-open**: an IdP whose signing
//! process happens to reformat XML between signing and transmission would
//! have *legitimate* logins rejected (a compatibility problem, loud and
//! immediate), not have *forged* assertions accepted (a security
//! problem). No IdP this was tested against (metadata shapes matching
//! Okta, Azure AD, Google Workspace, Keycloak) reformats between signing
//! and transmission.
//!
//! # XML Signature Wrapping (XSW) mitigation
//!
//! A well-documented SAML vulnerability class involves an attacker adding
//! a second, attacker-controlled `Assertion` (or duplicate-ID element)
//! alongside a legitimately-signed one, hoping a naive verifier checks the
//! signed copy but the application reads attributes from the unsigned
//! one. This module closes that gap structurally: it requires **exactly
//! one** `Assertion` element in the response (rejecting zero or more than
//! one outright), requires the `Signature` to be a **direct child** of
//! that same `Assertion` (the enveloped-signature convention), requires
//! the signature's `Reference/@URI` to point at that assertion's own `ID`,
//! and reads every claim from that same parsed node — there is never a
//! second search pass that could be pointed at different content than what
//! was verified.
//!
//! # Other scope limits
//!
//! Only `RSA-SHA256`-signed assertions are supported (matching this
//! crate's existing JWKS/JWT verification capability — no SHA-1, no EC).
//! `EncryptedAssertion` is rejected outright, not silently ignored — no
//! XML decryption is implemented. Multi-valued SAML attributes keep only
//! their first value.

use std::collections::HashMap;

use rsa::pkcs1v15::{Signature as RsaSignature, VerifyingKey};
use rsa::signature::Verifier;
use rsa::RsaPublicKey;
use sha2::{Digest, Sha256};

use super::metadata::decode_base64_flexible;
use super::xml::{self, XmlNode};
use crate::sso::SsoError;

const BEARER_METHOD: &str = "urn:oasis:names:tc:SAML:2.0:cm:bearer";
const RSA_SHA256: &str = "http://www.w3.org/2001/04/xmldsig-more#rsa-sha256";
const SHA256_DIGEST: &str = "http://www.w3.org/2001/04/xmlenc#sha256";
/// Clock-skew tolerance applied to `NotBefore`/`NotOnOrAfter` checks.
const LEEWAY_SECS: i64 = 60;

/// The verified identity and attributes from a SAML `Response`.
#[derive(Debug)]
pub struct SamlAssertion {
    /// The `Subject/NameID` value.
    pub name_id: String,
    /// Raw SAML attribute name → first value. Apply
    /// [`super::AttributeMap`] to translate IdP-specific names.
    pub attributes: HashMap<String, String>,
}

/// Parse, verify, and validate a base64-decoded `SAMLResponse` body.
///
/// `expected_request_id` is the ID of the `AuthnRequest` this response is
/// replying to (`None` only makes sense for an unsolicited, IdP-initiated
/// response, which skips the anti-replay `InResponseTo` check).
pub(crate) fn parse_and_verify(
    raw_xml: &str,
    idp_entity_id: &str,
    idp_signing_key: &RsaPublicKey,
    sp_entity_id: &str,
    acs_url: &str,
    expected_request_id: Option<&str>,
    now: u64,
) -> Result<SamlAssertion, SsoError> {
    if raw_xml.contains("EncryptedAssertion") {
        return Err(SsoError("encrypted assertions are not supported".into()));
    }

    let response = xml::parse(raw_xml).map_err(SsoError)?;
    if response.name != "Response" {
        return Err(SsoError("expected a top-level Response element".into()));
    }

    if let Some(expected_id) = expected_request_id {
        let in_response_to = response.attr("InResponseTo").unwrap_or("");
        if in_response_to != expected_id {
            return Err(SsoError("InResponseTo does not match the original AuthnRequest".into()));
        }
    }

    let mut assertions = Vec::new();
    response.find_all("Assertion", &mut assertions);
    if assertions.len() != 1 {
        return Err(SsoError(format!(
            "expected exactly one Assertion, found {}",
            assertions.len()
        )));
    }
    let assertion = assertions[0];

    let issuer = assertion.child("Issuer").map(|n| n.text.as_str()).unwrap_or("");
    if issuer != idp_entity_id {
        return Err(SsoError(format!("assertion issuer mismatch: expected {idp_entity_id}, got {issuer}")));
    }

    verify_signature(raw_xml, assertion, idp_signing_key)?;

    let conditions = assertion.child("Conditions").ok_or_else(|| SsoError("assertion is missing Conditions".into()))?;
    check_time_window(conditions.attr("NotBefore"), conditions.attr("NotOnOrAfter"), now)?;

    let mut audiences = Vec::new();
    conditions.find_all("Audience", &mut audiences);
    if !audiences.iter().any(|a| a.text == sp_entity_id) {
        return Err(SsoError(format!("AudienceRestriction does not include {sp_entity_id}")));
    }

    let subject = assertion.child("Subject").ok_or_else(|| SsoError("assertion is missing Subject".into()))?;
    let name_id = subject
        .child("NameID")
        .map(|n| n.text.clone())
        .ok_or_else(|| SsoError("Subject is missing NameID".into()))?;

    let confirmation = subject
        .find("SubjectConfirmation")
        .ok_or_else(|| SsoError("Subject is missing SubjectConfirmation".into()))?;
    if confirmation.attr("Method") != Some(BEARER_METHOD) {
        return Err(SsoError("SubjectConfirmation method is not bearer".into()));
    }
    let confirmation_data = confirmation
        .child("SubjectConfirmationData")
        .ok_or_else(|| SsoError("SubjectConfirmation is missing SubjectConfirmationData".into()))?;
    check_time_window(None, confirmation_data.attr("NotOnOrAfter"), now)?;
    if let Some(recipient) = confirmation_data.attr("Recipient") {
        if recipient != acs_url {
            return Err(SsoError("SubjectConfirmationData Recipient does not match the ACS URL".into()));
        }
    }
    if let Some(expected_id) = expected_request_id {
        if let Some(in_response_to) = confirmation_data.attr("InResponseTo") {
            if in_response_to != expected_id {
                return Err(SsoError("SubjectConfirmationData InResponseTo does not match the original AuthnRequest".into()));
            }
        }
    }

    let attributes = assertion
        .find("AttributeStatement")
        .map(xml::collect_attributes)
        .unwrap_or_default();

    Ok(SamlAssertion { name_id, attributes })
}

fn verify_signature(raw_xml: &str, assertion: &XmlNode, idp_signing_key: &RsaPublicKey) -> Result<(), SsoError> {
    let assertion_id = assertion.attr("ID").ok_or_else(|| SsoError("Assertion is missing ID".into()))?;

    let signature = assertion
        .child("Signature")
        .ok_or_else(|| SsoError("Assertion is not signed (no direct-child Signature element)".into()))?;
    let signed_info = signature
        .child("SignedInfo")
        .ok_or_else(|| SsoError("Signature is missing SignedInfo".into()))?;

    let signature_method = signed_info.child("SignatureMethod").and_then(|n| n.attr("Algorithm"));
    if signature_method != Some(RSA_SHA256) {
        return Err(SsoError(format!("unsupported SignatureMethod (only {RSA_SHA256} is supported)")));
    }

    let reference = signed_info
        .child("Reference")
        .ok_or_else(|| SsoError("SignedInfo is missing Reference".into()))?;
    let reference_uri = reference.attr("URI").unwrap_or("");
    if reference_uri != format!("#{assertion_id}") {
        return Err(SsoError("Reference URI does not point at the signed Assertion's own ID".into()));
    }
    let digest_method = reference.child("DigestMethod").and_then(|n| n.attr("Algorithm"));
    if digest_method != Some(SHA256_DIGEST) {
        return Err(SsoError(format!("unsupported DigestMethod (only {SHA256_DIGEST} is supported)")));
    }
    let digest_value_b64 = reference
        .child("DigestValue")
        .map(|n| n.text.clone())
        .ok_or_else(|| SsoError("Reference is missing DigestValue".into()))?;
    let expected_digest = decode_base64_flexible(&digest_value_b64)?;

    // Digest the Assertion's own raw bytes with the Signature element's
    // raw bytes excised — the enveloped-signature transform, applied to
    // the literal document rather than a canonical form (see module docs).
    let mut signed_bytes = Vec::with_capacity(assertion.end - assertion.start);
    signed_bytes.extend_from_slice(raw_xml[assertion.start..signature.start].as_bytes());
    signed_bytes.extend_from_slice(raw_xml[signature.end..assertion.end].as_bytes());
    let actual_digest = Sha256::digest(&signed_bytes);
    if actual_digest.as_slice() != expected_digest.as_slice() {
        return Err(SsoError("Assertion digest does not match Reference/DigestValue".into()));
    }

    let signature_value_b64 = signature
        .child("SignatureValue")
        .map(|n| n.text.clone())
        .ok_or_else(|| SsoError("Signature is missing SignatureValue".into()))?;
    let signature_bytes = decode_base64_flexible(&signature_value_b64)?;
    let rsa_signature = RsaSignature::try_from(signature_bytes.as_slice())
        .map_err(|e| SsoError(format!("malformed SignatureValue: {e}")))?;

    let verifying_key = VerifyingKey::<Sha256>::new(idp_signing_key.clone());
    let signed_info_bytes = raw_xml[signed_info.start..signed_info.end].as_bytes();
    verifying_key
        .verify(signed_info_bytes, &rsa_signature)
        .map_err(|_| SsoError("SignatureValue does not verify against the IdP's signing certificate".into()))?;

    Ok(())
}

fn check_time_window(not_before: Option<&str>, not_on_or_after: Option<&str>, now: u64) -> Result<(), SsoError> {
    let now = now as i64;
    if let Some(nb) = not_before {
        let nb = parse_iso8601(nb).ok_or_else(|| SsoError(format!("unparseable NotBefore: {nb}")))?;
        if now + LEEWAY_SECS < nb as i64 {
            return Err(SsoError("assertion is not yet valid (NotBefore)".into()));
        }
    }
    if let Some(noa) = not_on_or_after {
        let noa = parse_iso8601(noa).ok_or_else(|| SsoError(format!("unparseable NotOnOrAfter: {noa}")))?;
        if now - LEEWAY_SECS >= noa as i64 {
            return Err(SsoError("assertion has expired (NotOnOrAfter)".into()));
        }
    }
    Ok(())
}

/// Parse a SAML `xs:dateTime` value (`2024-01-01T00:00:00Z`, optionally
/// with fractional seconds) into Unix seconds. Only the `Z` (UTC) form is
/// supported — every IdP this was tested against emits UTC timestamps.
pub(crate) fn parse_iso8601(s: &str) -> Option<u64> {
    let s = s.strip_suffix('Z')?;
    let (date, time) = s.split_once('T')?;
    let mut d = date.splitn(3, '-');
    let y: u32 = d.next()?.parse().ok()?;
    let m: u32 = d.next()?.parse().ok()?;
    let day: u32 = d.next()?.parse().ok()?;
    let time = time.split('.').next().unwrap_or(time);
    let mut t = time.splitn(3, ':');
    let h: u64 = t.next()?.parse().ok()?;
    let mi: u64 = t.next()?.parse().ok()?;
    let se: u64 = t.next()?.parse().ok()?;
    let days = crate::scheduler::cron::ymd_to_days(y, m, day);
    Some(days * 86_400 + h * 3600 + mi * 60 + se)
}
