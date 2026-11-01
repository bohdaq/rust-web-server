use super::*;

#[test]
fn uris_parse() {
    let u = Pkcs11Uri::parse("pkcs11:token=rws%20prod;object=web-key;type=private?module-path=/usr/lib/softhsm/libsofthsm2.so&pin-source=env:HSM_PIN").unwrap();
    assert_eq!(u.token.as_deref(), Some("rws prod"));
    assert_eq!(u.object.as_deref(), Some("web-key"));
    assert_eq!(u.module_path, "/usr/lib/softhsm/libsofthsm2.so");
    assert_eq!(u.pin, Some(PinSource::Env("HSM_PIN".into())));
    let u = Pkcs11Uri::parse("PKCS11:id=%01%02?module-path=/m.so&pin-source=file:///run/secrets/pin").unwrap();
    assert_eq!((u.id, u.pin), (Some(vec![1, 2]), Some(PinSource::File("/run/secrets/pin".into()))));
    assert!(Pkcs11Uri::parse("pkcs11:object=k").unwrap_err().contains("module-path"));
    assert!(Pkcs11Uri::parse("pkcs11:token=t?module-path=/m.so").unwrap_err().contains("object="));
    assert!(Pkcs11Uri::parse("pkcs11:color=red;object=k?module-path=/m.so").unwrap_err().contains("unknown attribute"));
    assert!(!Pkcs11Uri::is_uri("/etc/rws/key.pem"));
}

#[test]
fn raw_ecdsa_signatures_become_der() {
    // r has its top bit set (needs a 0 pad), s has leading zeros (trimmed).
    let mut raw = vec![0x80u8; 32];
    raw.extend(std::iter::repeat_n(0u8, 30).chain([0x01, 0x02]));
    let der = ecdsa_der(&raw);
    assert_eq!(&der[..4], &[0x30, 2 + 33 + 2 + 2, 0x02, 33]);
    assert_eq!(der[4], 0);
    assert_eq!(&der[der.len() - 4..], &[0x02, 2, 0x01, 0x02]);
}

#[test]
fn a_missing_module_is_a_clear_error() {
    let e = signing_key("pkcs11:object=k?module-path=/nonexistent/libnothing.so").unwrap_err();
    assert!(e.contains("/nonexistent/libnothing.so"), "{e}");
}

/// Against SoftHSM2 when the test environment provides one (CI's `pkcs11`
/// job sets these after creating a token and a key).
#[test]
fn signs_with_a_softhsm_key_when_available() {
    let Ok(uri) = std::env::var("RWS_TEST_PKCS11_URI") else { return };
    let key = signing_key(&uri).expect("opens the key");
    let signer = key.choose_scheme(&[SignatureScheme::ECDSA_NISTP256_SHA256, SignatureScheme::RSA_PSS_SHA256]).expect("a scheme");
    let sig = signer.sign(b"hello").expect("signs");
    assert!(!sig.is_empty());
}
