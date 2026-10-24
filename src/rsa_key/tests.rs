use super::*;

// Generated with OpenSSL (an independent implementation):
//   openssl genrsa -out k.pem 2048
//   printf 'rws rsa fixture' > msg
//   openssl dgst -sha256 -sign k.pem -out sig256 msg
//   openssl dgst -sha1   -sign k.pem -out sig1   msg
const OPENSSL_SPKI_PEM: &str = "-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAsutLG2NcWANxl2ssNH6H
JDrqJFRqwl4fuDFX+7yHQNAjBxxZGAetmRcc/6LiiU8oYbexC7beORPOs6Z+y07a
FcvAHvtoVwLBNXnAmmW+zuSx2KAkPNt5F4MQIe+aDtZaDznurx2xHCSE/KTu0mvt
FUTc/9C70nBX3oh4qqJZxEL+ABZ2ci3YGId8vZkeIYPIpUULns3y0GYDwh5pAWwF
kJ0SiYNB260ScxqmNUzNJHHpsuhh8BnvS7MuLbTjLv4BWUOPifGumuIGKF/qGTRO
iB/9sUf9Ai+r7RmbyXG5UDOywbgWu8EqGbC0VAR42B7aDygYghK7wP/mmfxhUHnB
+QIDAQAB
-----END PUBLIC KEY-----
";

const OPENSSL_PKCS1_PEM: &str = "-----BEGIN RSA PUBLIC KEY-----
MIIBCgKCAQEAsutLG2NcWANxl2ssNH6HJDrqJFRqwl4fuDFX+7yHQNAjBxxZGAet
mRcc/6LiiU8oYbexC7beORPOs6Z+y07aFcvAHvtoVwLBNXnAmmW+zuSx2KAkPNt5
F4MQIe+aDtZaDznurx2xHCSE/KTu0mvtFUTc/9C70nBX3oh4qqJZxEL+ABZ2ci3Y
GId8vZkeIYPIpUULns3y0GYDwh5pAWwFkJ0SiYNB260ScxqmNUzNJHHpsuhh8Bnv
S7MuLbTjLv4BWUOPifGumuIGKF/qGTROiB/9sUf9Ai+r7RmbyXG5UDOywbgWu8Eq
GbC0VAR42B7aDygYghK7wP/mmfxhUHnB+QIDAQAB
-----END RSA PUBLIC KEY-----
";

const MESSAGE: &[u8] = b"rws rsa fixture";
const SIG_SHA256: &str = "17a1ea38015ae70127382d61313a0b3cdb64b4fec05f8f5b2eecf245fe37b2e1502f36db3ae1ed786168e2b2b9f45b568f1597071a74ac3ff01ff0bac82e8cfd1de64f5a25db5cc4a977524b85405c2f8b2800b143cf53473584e5e0df97a003d8b0d1984704152ecbe39e41e9fecd6ebf625f849cc7066e2e8867ddac1da3798dd85ad074c8ac92a58348071a88ab64e1a98bc5b62d1494fcb249d5b7839d0de6058cc87d466ea239eab936efc3822d7e236fabc2a80e3e12366a914b1225ef3aba6d0e97073c91a11ec1f7ad3cb8161964e3cec21cbc2c910e651d2ab4bb824c98ed903ab022b62b510449a9d54eeac0134c3d4dcc029c27d3d71547523440";
const SIG_SHA1: &str = "7b374d48660634dbd9a5024812cbe6316fc2cc9422bf1d20f9a9b5593b3f791bad0c1610848fc26dd1feccdce7c85e835e4c71608f83084a13c21cd96d8d3a37d3e50189a83d5fa32e5e2952a1bfff56c818742e9eef08800869d99a51e2ee57ede6e68384c9ffd849d6a331f2a2060b82921c76aeeb272c7da92e0a9295a0cd6d03aad7a327b19d0067de81c44749457e79106abc80446f3d3ef7d7d3168bcc4bb3d6abfd12d6c9b351b7550ade858d82237edb71726fb3743aaa3a0327407f4d9c1969f4b4d5bb6e7886dbd501a963aedd4ccaa1e0b748965835af09e2b56d1a08ae25118d0e47cc9f1662757f44b0f5dd73f8f1bfc89789e1f61975711dc2";

fn hex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

#[test]
fn verifies_an_openssl_signature() {
    let key = RsaPublicKey::from_public_key_pem(OPENSSL_SPKI_PEM).unwrap();
    assert_eq!(2048, key.size_bits());
    assert_eq!(65537, key.exponent());
    assert!(key.verify_pkcs1v15_sha256(MESSAGE, &hex(SIG_SHA256)));
}

#[test]
fn spki_and_pkcs1_pem_give_the_same_key() {
    let spki = RsaPublicKey::from_public_key_pem(OPENSSL_SPKI_PEM).unwrap();
    let pkcs1 = RsaPublicKey::from_public_key_pem(OPENSSL_PKCS1_PEM).unwrap();
    assert_eq!(spki, pkcs1);
    assert!(pkcs1.verify_pkcs1v15_sha256(MESSAGE, &hex(SIG_SHA256)));
}

#[test]
fn rejects_tampering() {
    let key = RsaPublicKey::from_public_key_pem(OPENSSL_SPKI_PEM).unwrap();
    let signature = hex(SIG_SHA256);
    assert!(!key.verify_pkcs1v15_sha256(b"rws rsa fixturE", &signature), "other message");
    let mut flipped = signature.clone();
    flipped[100] ^= 1;
    assert!(!key.verify_pkcs1v15_sha256(MESSAGE, &flipped), "flipped bit");
    assert!(!key.verify_pkcs1v15_sha256(MESSAGE, &signature[1..]), "short signature");
    let mut long = vec![0u8];
    long.extend_from_slice(&signature);
    assert!(!key.verify_pkcs1v15_sha256(MESSAGE, &long), "long signature");
    assert!(!key.verify_pkcs1v15_sha256(MESSAGE, &vec![0xff; signature.len()]), "signature ≥ modulus");
    assert!(!key.verify_pkcs1v15_sha256(MESSAGE, &vec![0x00; signature.len()]), "zero signature");
}

#[test]
fn rejects_a_valid_signature_with_another_hash() {
    // A genuine PKCS#1 v1.5 signature, but over SHA-1: the DigestInfo differs.
    let key = RsaPublicKey::from_public_key_pem(OPENSSL_SPKI_PEM).unwrap();
    assert!(!key.verify_pkcs1v15_sha256(MESSAGE, &hex(SIG_SHA1)));
}

#[test]
fn rejects_unsuitable_keys() {
    let key = RsaPublicKey::from_public_key_pem(OPENSSL_SPKI_PEM).unwrap();
    let n = key.modulus().to_vec();
    assert!(RsaPublicKey::from_components(&n, &[0x01, 0x00, 0x01]).is_ok());
    assert!(RsaPublicKey::from_components(&n, &[0x01]).is_err(), "e = 1");
    assert!(RsaPublicKey::from_components(&n, &[0x02]).is_err(), "even e");
    assert!(RsaPublicKey::from_components(&n[..128], &[0x01, 0x00, 0x01]).is_err(), "1024-bit key");
    let mut even = n.clone();
    *even.last_mut().unwrap() &= 0xfe;
    assert!(RsaPublicKey::from_components(&even, &[0x01, 0x00, 0x01]).is_err(), "even modulus");
    assert!(RsaPublicKey::from_public_key_pem("-----BEGIN CERTIFICATE-----\nAA==\n-----END CERTIFICATE-----").is_err());
    assert!(RsaPublicKey::from_public_key_der(&[0x30, 0x03, 0x02, 0x01, 0x01]).is_err());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn verifies_aws_lc_signatures_across_many_messages() {
    let key = test_support::TestRsaKey::generate();
    let public = key.public_key();
    let from_spki = RsaPublicKey::from_public_key_der(&key.spki_der()).unwrap();
    let from_pem = RsaPublicKey::from_public_key_pem(&key.public_key_pem()).unwrap();
    let (n, e) = key.n_e();
    let from_parts = RsaPublicKey::from_components(&n, &e).unwrap();
    assert!(public == from_spki && public == from_pem && public == from_parts);
    for i in 0..20u32 {
        let message = format!("message {i}").repeat(i as usize + 1);
        let signature = key.sign_sha256(message.as_bytes());
        assert!(public.verify_pkcs1v15_sha256(message.as_bytes(), &signature), "message {i}");
        assert!(!public.verify_pkcs1v15_sha256(b"something else", &signature));
    }
    let other = test_support::TestRsaKey::generate();
    assert!(!other.public_key().verify_pkcs1v15_sha256(b"m", &key.sign_sha256(b"m")), "another key");
}

#[test]
fn montgomery_inverse_is_correct() {
    for n0 in [1u32, 3, 0xffff_ffff, 0x1234_5679, 0x8000_0001] {
        assert_eq!(n0.wrapping_mul(montgomery_n0_inv(n0)), u32::MAX, "n0 = {n0:#x}"); // n0·(−n0⁻¹) ≡ −1
    }
}
