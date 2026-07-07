//! Tests for the SAML 2.0 Service Provider (`sso-saml` feature).
//!
//! All tests operate against a real, freshly-generated RSA-2048 keypair
//! and hand-built XML/DER documents — no network calls, no fake IdP
//! server (unlike `oidc_auth_tests`/`server_tests`, `SamlSp` makes no
//! outbound HTTP calls of its own; `SamlIdpMetadata::from_url` is the only
//! network-touching function here, and it isn't exercised — `from_str`
//! covers the parsing logic it shares with `from_file`/`from_url`).

use super::assertion::parse_and_verify;
use super::der::extract_subject_public_key_info;
use super::metadata::SamlIdpMetadata;
use super::xml;
use super::{AttributeMap, SamlConfig, SamlSp};
use crate::application::Application;
use crate::core::New;
use crate::header::Header;
use crate::http::VERSION;
use crate::middleware::Middleware;
use crate::request::{Request, METHOD};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::{Address, ConnectionInfo};
use crate::session::SessionStore;
use rsa::pkcs8::{DecodePublicKey, EncodePublicKey};
use rsa::RsaPublicKey;
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

// ── shared RSA test key + fake X.509 wrapper ────────────────────────────────

fn idp_key() -> &'static rsa::RsaPrivateKey {
    static KEY: OnceLock<rsa::RsaPrivateKey> = OnceLock::new();
    KEY.get_or_init(|| rsa::RsaPrivateKey::new(&mut rand_core::OsRng, 2048).unwrap())
}

fn der_len(n: usize) -> Vec<u8> {
    if n < 128 {
        vec![n as u8]
    } else {
        let mut bytes = Vec::new();
        let mut x = n;
        while x > 0 {
            bytes.insert(0, (x & 0xff) as u8);
            x >>= 8;
        }
        let mut out = vec![0x80 | bytes.len() as u8];
        out.extend(bytes);
        out
    }
}

fn der_sequence(children: &[Vec<u8>]) -> Vec<u8> {
    let value: Vec<u8> = children.concat();
    let mut out = vec![0x30u8];
    out.extend(der_len(value.len()));
    out.extend(value);
    out
}

fn der_small_integer(n: u8) -> Vec<u8> {
    vec![0x02, 0x01, n]
}

/// Build a syntactically-valid-enough (for our own DER walker) fake X.509
/// certificate wrapping a real RSA public key's SubjectPublicKeyInfo.
/// `issuer`/`validity`/`subject`/signature fields are empty SEQUENCEs —
/// `extract_subject_public_key_info` only needs to skip them by TLV
/// length, not interpret their contents.
fn fake_cert_der(pub_key: &RsaPublicKey) -> Vec<u8> {
    let spki = pub_key.to_public_key_der().unwrap().as_bytes().to_vec();
    let tbs = der_sequence(&[
        der_small_integer(1),  // serialNumber
        der_sequence(&[]),     // signature AlgorithmIdentifier
        der_sequence(&[]),     // issuer
        der_sequence(&[]),     // validity
        der_sequence(&[]),     // subject
        spki,                  // subjectPublicKeyInfo
    ]);
    der_sequence(&[
        tbs,
        der_sequence(&[]), // signatureAlgorithm
        der_sequence(&[]), // signatureValue (not a real BIT STRING, but never read)
    ])
}

fn fake_cert_b64(pub_key: &RsaPublicKey) -> String {
    super::base64_standard_encode(&fake_cert_der(pub_key))
}

// ── DER tests ────────────────────────────────────────────────────────────────

#[test]
fn der_extracts_subject_public_key_info_matching_original_key() {
    let pub_key = idp_key().to_public_key();
    let cert_der = fake_cert_der(&pub_key);
    let spki = extract_subject_public_key_info(&cert_der).unwrap();
    let recovered = RsaPublicKey::from_public_key_der(&spki).unwrap();
    assert_eq!(recovered, pub_key);
}

#[test]
fn der_rejects_truncated_input() {
    let bad = vec![0x30, 0x7f]; // SEQUENCE claiming 127 bytes, none present
    assert!(extract_subject_public_key_info(&bad).is_err());
}

// ── XML parser tests ─────────────────────────────────────────────────────────

#[test]
fn xml_parses_nested_elements_and_text() {
    let doc = xml::parse("<a><b>hello</b><c/></a>").unwrap();
    assert_eq!(doc.name, "a");
    assert_eq!(doc.children.len(), 2);
    assert_eq!(doc.children[0].name, "b");
    assert_eq!(doc.children[0].text, "hello");
    assert_eq!(doc.children[1].name, "c");
}

#[test]
fn xml_parses_attributes_both_quote_styles() {
    let doc = xml::parse(r#"<a x="1" y='2'/>"#).unwrap();
    assert_eq!(doc.attr("x"), Some("1"));
    assert_eq!(doc.attr("y"), Some("2"));
}

#[test]
fn xml_strips_namespace_prefixes() {
    let doc = xml::parse(r#"<samlp:Response xmlns:samlp="urn:x"><saml:Issuer xmlns:saml="urn:y">idp</saml:Issuer></samlp:Response>"#).unwrap();
    assert_eq!(doc.name, "Response");
    assert_eq!(doc.child("Issuer").unwrap().text, "idp");
}

#[test]
fn xml_handles_same_name_nested_elements() {
    let doc = xml::parse("<a><a>inner</a></a>").unwrap();
    assert_eq!(doc.children.len(), 1);
    assert_eq!(doc.children[0].name, "a");
    assert_eq!(doc.children[0].text, "inner");
}

#[test]
fn xml_decodes_entities() {
    let doc = xml::parse("<a>&lt;tag&gt; &amp; &#65;&#x42;</a>").unwrap();
    assert_eq!(doc.text, "<tag> & AB");
}

#[test]
fn xml_handles_cdata() {
    let doc = xml::parse("<a><![CDATA[<raw> & stuff]]></a>").unwrap();
    assert_eq!(doc.text, "<raw> & stuff");
}

#[test]
fn xml_skips_comments() {
    let doc = xml::parse("<a><!-- a comment --><b>x</b></a>").unwrap();
    assert_eq!(doc.children.len(), 1);
    assert_eq!(doc.children[0].name, "b");
}

#[test]
fn xml_rejects_doctype() {
    assert!(xml::parse("<!DOCTYPE a><a/>").is_err());
}

#[test]
fn xml_find_all_searches_every_depth() {
    let doc = xml::parse("<a><b><c>1</c></b><c>2</c></a>").unwrap();
    let mut found = Vec::new();
    doc.find_all("c", &mut found);
    assert_eq!(found.len(), 2);
}

#[test]
fn xml_collect_attributes_keeps_first_value_for_duplicates() {
    let doc = xml::parse(
        r#"<AttributeStatement><Attribute Name="email"><AttributeValue>a@x.com</AttributeValue></Attribute><Attribute Name="email"><AttributeValue>b@x.com</AttributeValue></Attribute></AttributeStatement>"#,
    )
    .unwrap();
    let attrs = xml::collect_attributes(&doc);
    assert_eq!(attrs.get("email"), Some(&"a@x.com".to_string()));
}

// ── metadata tests ───────────────────────────────────────────────────────────

fn idp_metadata_xml(entity_id: &str, sso_url: &str, cert_b64: &str) -> String {
    format!(
        r#"<EntityDescriptor xmlns="urn:oasis:names:tc:SAML:2.0:metadata" entityID="{entity_id}">
            <IDPSSODescriptor protocolSupportEnabled="urn:oasis:names:tc:SAML:2.0:protocol">
                <KeyDescriptor use="signing">
                    <ds:KeyInfo xmlns:ds="http://www.w3.org/2000/09/xmldsig#">
                        <ds:X509Data><ds:X509Certificate>{cert_b64}</ds:X509Certificate></ds:X509Data>
                    </ds:KeyInfo>
                </KeyDescriptor>
                <SingleSignOnService Binding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-Redirect" Location="{sso_url}/redirect"/>
                <SingleSignOnService Binding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST" Location="{sso_url}"/>
            </IDPSSODescriptor>
        </EntityDescriptor>"#
    )
}

#[test]
fn metadata_parses_entity_id_and_prefers_post_binding() {
    let pub_key = idp_key().to_public_key();
    let xml_str = idp_metadata_xml("https://idp.example.com", "https://idp.example.com/sso", &fake_cert_b64(&pub_key));
    let meta = SamlIdpMetadata::from_str(&xml_str).unwrap();
    assert_eq!(meta.entity_id, "https://idp.example.com");
    assert_eq!(meta.sso_url, "https://idp.example.com/sso");
    assert_eq!(meta.signing_key, pub_key);
}

#[test]
fn metadata_missing_idp_descriptor_fails() {
    let result = SamlIdpMetadata::from_str(r#"<EntityDescriptor entityID="x"></EntityDescriptor>"#);
    assert!(result.is_err());
}

#[test]
fn metadata_from_file_reads_and_parses() {
    let pub_key = idp_key().to_public_key();
    let xml_str = idp_metadata_xml("https://idp.example.com", "https://idp.example.com/sso", &fake_cert_b64(&pub_key));
    let path = std::env::temp_dir().join(format!("rws-saml-test-metadata-{}.xml", std::process::id()));
    std::fs::write(&path, &xml_str).unwrap();
    let meta = SamlIdpMetadata::from_file(path.to_str().unwrap()).unwrap();
    assert_eq!(meta.entity_id, "https://idp.example.com");
    std::fs::remove_file(&path).ok();
}

// ── signed assertion builder (shared by assertion_tests + saml_sp_tests) ────

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

struct SignedResponseOptions<'a> {
    idp_entity_id: &'a str,
    sp_entity_id: &'a str,
    acs_url: &'a str,
    in_response_to: &'a str,
    name_id: &'a str,
    attrs: &'a [(&'a str, &'a str)],
    not_before_offset: i64,
    not_after_offset: i64,
    confirmation_method: &'a str,
}

fn build_signed_response(opts: &SignedResponseOptions) -> String {
    let now = unix_now() as i64;
    let issue_instant = super::format_iso8601(now as u64);
    let not_before = super::format_iso8601((now + opts.not_before_offset).max(0) as u64);
    let not_after = super::format_iso8601((now + opts.not_after_offset).max(0) as u64);
    let assertion_id = "_assertion00000000000000000000000000";
    let response_id = "_response0000000000000000000000000000";

    let attrs_xml: String = opts
        .attrs
        .iter()
        .map(|(name, value)| format!(r#"<saml:Attribute Name="{name}"><saml:AttributeValue>{value}</saml:AttributeValue></saml:Attribute>"#))
        .collect();

    let before_signature = format!(
        r#"<saml:Assertion xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion" ID="{assertion_id}" IssueInstant="{issue_instant}" Version="2.0"><saml:Issuer>{issuer}</saml:Issuer>"#,
        issuer = opts.idp_entity_id,
    );
    let after_signature = format!(
        r#"<saml:Subject><saml:NameID>{name_id}</saml:NameID><saml:SubjectConfirmation Method="{method}"><saml:SubjectConfirmationData Recipient="{acs}" NotOnOrAfter="{not_after}" InResponseTo="{irt}"/></saml:SubjectConfirmation></saml:Subject><saml:Conditions NotBefore="{not_before}" NotOnOrAfter="{not_after}"><saml:AudienceRestriction><saml:Audience>{sp_entity_id}</saml:Audience></saml:AudienceRestriction></saml:Conditions><saml:AttributeStatement>{attrs_xml}</saml:AttributeStatement></saml:Assertion>"#,
        name_id = opts.name_id,
        method = opts.confirmation_method,
        acs = opts.acs_url,
        irt = opts.in_response_to,
        sp_entity_id = opts.sp_entity_id,
    );

    let unsigned_assertion = format!("{before_signature}{after_signature}");
    let digest = sha2::Sha256::digest(unsigned_assertion.as_bytes());
    let digest_b64 = super::base64_standard_encode(&digest);

    let signed_info = format!(
        r##"<ds:SignedInfo xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><ds:CanonicalizationMethod Algorithm="http://www.w3.org/2001/10/xml-exc-c14n#"/><ds:SignatureMethod Algorithm="http://www.w3.org/2001/04/xmldsig-more#rsa-sha256"/><ds:Reference URI="#{assertion_id}"><ds:Transforms><ds:Transform Algorithm="http://www.w3.org/2000/09/xmldsig#enveloped-signature"/></ds:Transforms><ds:DigestMethod Algorithm="http://www.w3.org/2001/04/xmlenc#sha256"/><ds:DigestValue>{digest_b64}</ds:DigestValue></ds:Reference></ds:SignedInfo>"##
    );

    use rsa::pkcs1v15::SigningKey;
    use rsa::signature::{SignatureEncoding, Signer};
    let signing_key = SigningKey::<sha2::Sha256>::new(idp_key().clone());
    let signature = signing_key.sign(signed_info.as_bytes());
    let signature_b64 = super::base64_standard_encode(&signature.to_bytes());

    let signature_xml = format!(
        r#"<ds:Signature xmlns:ds="http://www.w3.org/2000/09/xmldsig#">{signed_info}<ds:SignatureValue>{signature_b64}</ds:SignatureValue></ds:Signature>"#
    );

    let signed_assertion = format!("{before_signature}{signature_xml}{after_signature}");

    format!(
        r#"<samlp:Response xmlns:samlp="urn:oasis:names:tc:SAML:2.0:protocol" xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion" ID="{response_id}" IssueInstant="{issue_instant}" InResponseTo="{irt}" Version="2.0"><saml:Issuer>{issuer}</saml:Issuer>{signed_assertion}</samlp:Response>"#,
        irt = opts.in_response_to,
        issuer = opts.idp_entity_id,
    )
}

use sha2::Digest;

const IDP_ENTITY: &str = "https://idp.example.com";
const SP_ENTITY: &str = "https://sp.example.com/saml/metadata";
const ACS_URL: &str = "https://sp.example.com/saml/acs";
const REQUEST_ID: &str = "_request0000000000000000000000000000";
const BEARER: &str = "urn:oasis:names:tc:SAML:2.0:cm:bearer";

fn default_opts<'a>() -> SignedResponseOptions<'a> {
    SignedResponseOptions {
        idp_entity_id: IDP_ENTITY,
        sp_entity_id: SP_ENTITY,
        acs_url: ACS_URL,
        in_response_to: REQUEST_ID,
        name_id: "alice",
        attrs: &[("email", "alice@example.com")],
        not_before_offset: -300,
        not_after_offset: 300,
        confirmation_method: BEARER,
    }
}

fn verify(xml_str: &str, expected_request_id: Option<&str>) -> Result<super::assertion::SamlAssertion, crate::sso::SsoError> {
    let pub_key = idp_key().to_public_key();
    parse_and_verify(xml_str, IDP_ENTITY, &pub_key, SP_ENTITY, ACS_URL, expected_request_id, unix_now())
}

// ── assertion verification tests ─────────────────────────────────────────────

#[test]
fn assertion_valid_signature_and_claims_succeed() {
    let xml_str = build_signed_response(&default_opts());
    let result = verify(&xml_str, Some(REQUEST_ID)).unwrap();
    assert_eq!(result.name_id, "alice");
    assert_eq!(result.attributes.get("email"), Some(&"alice@example.com".to_string()));
}

#[test]
fn assertion_tampered_attribute_fails_digest_check() {
    let xml_str = build_signed_response(&default_opts());
    let tampered = xml_str.replace("alice@example.com", "eve@evil.com");
    assert!(verify(&tampered, Some(REQUEST_ID)).is_err());
}

#[test]
fn assertion_tampered_name_id_fails_digest_check() {
    let xml_str = build_signed_response(&default_opts());
    let tampered = xml_str.replace("<saml:NameID>alice</saml:NameID>", "<saml:NameID>mallory</saml:NameID>");
    assert!(verify(&tampered, Some(REQUEST_ID)).is_err());
}

#[test]
fn assertion_signed_by_different_key_fails() {
    let xml_str = build_signed_response(&default_opts());
    let other_key = rsa::RsaPrivateKey::new(&mut rand_core::OsRng, 2048).unwrap();
    let result = parse_and_verify(&xml_str, IDP_ENTITY, &other_key.to_public_key(), SP_ENTITY, ACS_URL, Some(REQUEST_ID), unix_now());
    assert!(result.is_err());
}

#[test]
fn assertion_wrong_issuer_fails() {
    let mut opts = default_opts();
    opts.idp_entity_id = "https://evil.example.com";
    let xml_str = build_signed_response(&opts);
    // Re-verify against the REAL expected issuer, which won't match.
    assert!(verify(&xml_str, Some(REQUEST_ID)).is_err());
}

#[test]
fn assertion_expired_fails() {
    let mut opts = default_opts();
    opts.not_before_offset = -7200;
    opts.not_after_offset = -3600;
    let xml_str = build_signed_response(&opts);
    let err = verify(&xml_str, Some(REQUEST_ID)).unwrap_err();
    assert!(err.0.contains("expired"), "expected expiry error, got: {}", err.0);
}

#[test]
fn assertion_not_yet_valid_fails() {
    let mut opts = default_opts();
    opts.not_before_offset = 3600;
    opts.not_after_offset = 7200;
    let xml_str = build_signed_response(&opts);
    let err = verify(&xml_str, Some(REQUEST_ID)).unwrap_err();
    assert!(err.0.contains("not yet valid"), "expected not-yet-valid error, got: {}", err.0);
}

#[test]
fn assertion_wrong_audience_fails() {
    let pub_key = idp_key().to_public_key();
    let xml_str = build_signed_response(&default_opts());
    let result = parse_and_verify(&xml_str, IDP_ENTITY, &pub_key, "https://someone-else.example.com", ACS_URL, Some(REQUEST_ID), unix_now());
    assert!(result.is_err());
}

#[test]
fn assertion_wrong_in_response_to_fails() {
    let xml_str = build_signed_response(&default_opts());
    assert!(verify(&xml_str, Some("_different_request_id")).is_err());
}

#[test]
fn assertion_wrong_recipient_fails() {
    let pub_key = idp_key().to_public_key();
    let xml_str = build_signed_response(&default_opts());
    let result = parse_and_verify(&xml_str, IDP_ENTITY, &pub_key, SP_ENTITY, "https://wrong-acs.example.com", Some(REQUEST_ID), unix_now());
    assert!(result.is_err());
}

#[test]
fn assertion_non_bearer_confirmation_method_fails() {
    let mut opts = default_opts();
    opts.confirmation_method = "urn:oasis:names:tc:SAML:2.0:cm:holder-of-key";
    let xml_str = build_signed_response(&opts);
    assert!(verify(&xml_str, Some(REQUEST_ID)).is_err());
}

#[test]
fn assertion_encrypted_assertion_rejected() {
    let result = verify("<samlp:Response><saml:EncryptedAssertion>opaque</saml:EncryptedAssertion></samlp:Response>", None);
    assert!(result.is_err());
}

#[test]
fn assertion_zero_assertions_rejected() {
    let result = verify(r#"<samlp:Response xmlns:samlp="urn:oasis:names:tc:SAML:2.0:protocol"></samlp:Response>"#, None);
    assert!(result.is_err());
}

#[test]
fn assertion_multiple_assertions_rejected() {
    let xml_str = build_signed_response(&default_opts());
    // Splice in a second, identical Assertion to simulate an XSW-style attempt.
    let marker = "</samlp:Response>";
    let (assertion_start, _) = {
        let idx = xml_str.find("<saml:Assertion").unwrap();
        let end = xml_str.find(marker).unwrap();
        (idx, end)
    };
    let duplicated = xml_str[..marker_pos(&xml_str)].to_string() + &xml_str[assertion_start..marker_pos(&xml_str)] + marker;
    fn marker_pos(s: &str) -> usize {
        s.find("</samlp:Response>").unwrap()
    }
    assert!(verify(&duplicated, Some(REQUEST_ID)).is_err());
}

#[test]
fn assertion_unsigned_fails() {
    let opts = default_opts();
    // Reuse build_signed_response but strip the Signature element out entirely.
    let signed = build_signed_response(&opts);
    let sig_start = signed.find("<ds:Signature").unwrap();
    let sig_end = signed.find("</ds:Signature>").unwrap() + "</ds:Signature>".len();
    let unsigned = format!("{}{}", &signed[..sig_start], &signed[sig_end..]);
    assert!(verify(&unsigned, Some(REQUEST_ID)).is_err());
}

// ── SamlSp middleware tests ──────────────────────────────────────────────────

fn conn() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 16000,
        sni_hostname: None,
    }
}

fn get(uri: &str) -> Request {
    Request {
        method: METHOD.get.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    }
}

fn post(uri: &str, body: &str) -> Request {
    Request {
        method: METHOD.post.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: body.as_bytes().to_vec(),
    }
}

fn with_cookie(mut req: Request, name: &str, value: &str) -> Request {
    req.headers.push(Header { name: "Cookie".to_string(), value: format!("{name}={value}") });
    req
}

fn header(response: &Response, name: &str) -> Option<String> {
    response.headers.iter().find(|h| h.name.eq_ignore_ascii_case(name)).map(|h| h.value.clone())
}

fn extract_cookie(response: &Response) -> (String, String) {
    let set_cookie = header(response, "Set-Cookie").expect("expected Set-Cookie");
    let first = set_cookie.split(';').next().unwrap();
    let mut parts = first.splitn(2, '=');
    (parts.next().unwrap().to_string(), parts.next().unwrap_or("").to_string())
}

struct OkApp;
impl Application for OkApp {
    fn execute(&self, _: &Request, _: &ConnectionInfo) -> Result<Response, String> {
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        Ok(r)
    }
}

fn test_metadata() -> SamlIdpMetadata {
    let pub_key = idp_key().to_public_key();
    let xml_str = idp_metadata_xml(IDP_ENTITY, "https://idp.example.com/sso", &fake_cert_b64(&pub_key));
    SamlIdpMetadata::from_str(&xml_str).unwrap()
}

fn test_sp(sessions: Arc<SessionStore>) -> SamlSp {
    SamlSp::new(SamlConfig {
        sp_entity_id: SP_ENTITY.to_string(),
        sp_acs_url: ACS_URL.to_string(),
        idp_metadata: test_metadata(),
        sessions,
    })
}

#[test]
fn saml_sp_unauthenticated_request_redirects_to_login() {
    let sessions = Arc::new(SessionStore::new(3600));
    let sp = test_sp(sessions);
    let resp = sp.handle(&get("/dashboard"), &conn(), &OkApp).unwrap();
    assert_eq!(302, resp.status_code);
    assert!(header(&resp, "Location").unwrap().starts_with("/saml/login?return_to="));
}

#[test]
fn saml_sp_metadata_endpoint_returns_sp_entity_and_acs() {
    let sessions = Arc::new(SessionStore::new(3600));
    let sp = test_sp(sessions);
    let resp = sp.handle(&get("/saml/metadata"), &conn(), &OkApp).unwrap();
    assert_eq!(200, resp.status_code);
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert!(body.contains(SP_ENTITY));
    assert!(body.contains(ACS_URL));
}

#[test]
fn saml_sp_login_renders_auto_submit_form_and_sets_cookie() {
    let sessions = Arc::new(SessionStore::new(3600));
    let sp = test_sp(sessions.clone());
    let resp = sp.handle(&get("/saml/login"), &conn(), &OkApp).unwrap();
    assert_eq!(200, resp.status_code);
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert!(body.contains("https://idp.example.com/sso"));
    assert!(body.contains("SAMLRequest"));

    let (_, sid) = extract_cookie(&resp);
    let session = sessions.load(&sid).unwrap();
    assert!(!session.get("_saml_request_id").unwrap().is_empty());
}

#[test]
fn saml_sp_acs_without_cookie_fails() {
    let sessions = Arc::new(SessionStore::new(3600));
    let sp = test_sp(sessions);
    let resp = sp.handle(&post("/saml/acs", "SAMLResponse=x"), &conn(), &OkApp).unwrap();
    assert_eq!(500, resp.status_code);
}

#[test]
fn saml_sp_acs_success_round_trip_stores_claims_and_redirects() {
    let sessions = Arc::new(SessionStore::new(3600));
    let sp = test_sp(sessions.clone());

    let mut pre = sessions.create();
    pre.set("_saml_request_id", REQUEST_ID);
    pre.set("_saml_return_to", "/dashboard");
    sessions.save(&pre);

    let response_xml = build_signed_response(&default_opts());
    let body = format!("SAMLResponse={}", super::base64_standard_encode(response_xml.as_bytes()));
    let req = with_cookie(post("/saml/acs", &body), "_rws_saml_sid", &pre.id);
    let resp = sp.handle(&req, &conn(), &OkApp).unwrap();

    assert_eq!(302, resp.status_code);
    assert_eq!(header(&resp, "Location"), Some("/dashboard".to_string()));

    let session = sessions.load(&pre.id).unwrap();
    let claims_json = session.get("_saml_claims").unwrap();
    assert!(claims_json.contains("alice"));
    assert!(session.get("_saml_request_id").is_none());
}

#[test]
fn saml_sp_attribute_map_translates_saml_names() {
    let sessions = Arc::new(SessionStore::new(3600));
    let sp = SamlSp::new(SamlConfig {
        sp_entity_id: SP_ENTITY.to_string(),
        sp_acs_url: ACS_URL.to_string(),
        idp_metadata: test_metadata(),
        sessions: sessions.clone(),
    })
    .attribute_map(AttributeMap::new().map("email", "mapped_email"));

    let mut pre = sessions.create();
    pre.set("_saml_request_id", REQUEST_ID);
    sessions.save(&pre);

    let response_xml = build_signed_response(&default_opts());
    let body = format!("SAMLResponse={}", super::base64_standard_encode(response_xml.as_bytes()));
    let req = with_cookie(post("/saml/acs", &body), "_rws_saml_sid", &pre.id);
    sp.handle(&req, &conn(), &OkApp).unwrap();

    let session = sessions.load(&pre.id).unwrap();
    let claims_json = session.get("_saml_claims").unwrap();
    assert!(claims_json.contains("mapped_email"));
    assert!(claims_json.contains("alice@example.com"));
}

#[test]
fn saml_sp_logout_destroys_session_and_redirects_home() {
    let sessions = Arc::new(SessionStore::new(3600));
    let sp = test_sp(sessions.clone());
    let mut session = sessions.create();
    session.set("_saml_claims", "{}");
    sessions.save(&session);

    let req = with_cookie(get("/saml/logout"), "_rws_saml_sid", &session.id);
    let resp = sp.handle(&req, &conn(), &OkApp).unwrap();
    assert_eq!(302, resp.status_code);
    assert_eq!(header(&resp, "Location"), Some("/".to_string()));
    assert!(sessions.load(&session.id).is_none());
}

#[test]
fn saml_sp_authenticated_request_injects_claims_header() {
    let sessions = Arc::new(SessionStore::new(3600));
    let sp = test_sp(sessions.clone());
    let mut session = sessions.create();
    session.set("_saml_claims", r#"{"name_id":"alice","attributes":{}}"#);
    sessions.save(&session);

    struct CapturingApp(std::sync::Mutex<Option<Request>>);
    impl Application for CapturingApp {
        fn execute(&self, req: &Request, _: &ConnectionInfo) -> Result<Response, String> {
            *self.0.lock().unwrap() = Some(req.clone());
            let mut r = Response::new();
            r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
            r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
            Ok(r)
        }
    }
    let inner = CapturingApp(std::sync::Mutex::new(None));
    let req = with_cookie(get("/dashboard"), "_rws_saml_sid", &session.id);
    let resp = sp.handle(&req, &conn(), &inner).unwrap();
    assert_eq!(200, resp.status_code);

    let captured = inner.0.lock().unwrap().clone().unwrap();
    let claims_header = captured.headers.iter().find(|h| h.name == super::CLAIMS_HEADER).unwrap();
    assert!(claims_header.value.contains("alice"));
}
