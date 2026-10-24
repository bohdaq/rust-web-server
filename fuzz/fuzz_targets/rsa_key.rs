//! Untrusted bytes → RSA key parsing (DER, PEM, JWK components) and
//! RS256 / rsa-sha256 signature verification (SAML SignatureValue arrives in
//! an unauthenticated POST).
#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_web_server::rsa_key::RsaPublicKey;

/// A real 2048-bit key (OpenSSL-generated, from src/rsa_key/tests.rs), so
/// arbitrary signatures exercise the full modular exponentiation.
const KEY_PEM: &str = "-----BEGIN RSA PUBLIC KEY-----
MIIBCgKCAQEAsutLG2NcWANxl2ssNH6HJDrqJFRqwl4fuDFX+7yHQNAjBxxZGAet
mRcc/6LiiU8oYbexC7beORPOs6Z+y07aFcvAHvtoVwLBNXnAmmW+zuSx2KAkPNt5
F4MQIe+aDtZaDznurx2xHCSE/KTu0mvtFUTc/9C70nBX3oh4qqJZxEL+ABZ2ci3Y
GId8vZkeIYPIpUULns3y0GYDwh5pAWwFkJ0SiYNB260ScxqmNUzNJHHpsuhh8Bnv
S7MuLbTjLv4BWUOPifGumuIGKF/qGTROiB/9sUf9Ai+r7RmbyXG5UDOywbgWu8Eq
GbC0VAR42B7aDygYghK7wP/mmfxhUHnB+QIDAQAB
-----END RSA PUBLIC KEY-----
";

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else { return };
    let key = match mode % 4 {
        0 => RsaPublicKey::from_public_key_der(rest),
        1 => RsaPublicKey::from_pkcs1_der(rest),
        2 => match std::str::from_utf8(rest) {
            Ok(pem) => RsaPublicKey::from_public_key_pem(pem),
            Err(_) => return,
        },
        _ => {
            let split = rest.first().map_or(0, |&b| b as usize).min(rest.len());
            RsaPublicKey::from_components(&rest[..split], &rest[split..])
        }
    };
    if let Ok(key) = key {
        let _ = key.verify_pkcs1v15_sha256(b"message", rest);
    }
    // A fixed real key against an arbitrary 256-byte signature.
    let fixed = RsaPublicKey::from_public_key_pem(KEY_PEM).unwrap();
    let mut signature = rest.to_vec();
    signature.resize(256, 0);
    assert!(!fixed.verify_pkcs1v15_sha256(b"no fuzzer forges this", &signature) || rest.len() >= 256);
});
