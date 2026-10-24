//! RSA keys for tests: generated and signed with `aws-lc-rs` (a dev
//! dependency), so the `rsa` crate isn't needed even to mint fixtures.

use super::RsaPublicKey;
use aws_lc_rs::signature::KeyPair;

pub(crate) struct TestRsaKey(aws_lc_rs::rsa::KeyPair);

impl TestRsaKey {
    pub(crate) fn generate() -> TestRsaKey {
        TestRsaKey(aws_lc_rs::rsa::KeyPair::generate(aws_lc_rs::rsa::KeySize::Rsa2048).expect("RSA keygen"))
    }

    pub(crate) fn to_public_key(&self) -> RsaPublicKey {
        self.public_key()
    }

    /// RSASSA-PKCS1-v1_5 / SHA-256 signature of `message`.
    pub(crate) fn sign_sha256(&self, message: &[u8]) -> Vec<u8> {
        let mut signature = vec![0u8; self.0.public_modulus_len()];
        self.0
            .sign(&aws_lc_rs::signature::RSA_PKCS1_SHA256, &aws_lc_rs::rand::SystemRandom::new(), message, &mut signature)
            .expect("RSA sign");
        signature
    }

    /// PKCS#1 `RSAPublicKey`, DER.
    pub(crate) fn pkcs1_der(&self) -> Vec<u8> {
        self.0.public_key().as_ref().to_vec()
    }

    /// X.509 `SubjectPublicKeyInfo`, DER.
    pub(crate) fn spki_der(&self) -> Vec<u8> {
        spki_der_for(&self.pkcs1_der())
    }

    /// `-----BEGIN PUBLIC KEY-----` PEM.
    pub(crate) fn public_key_pem(&self) -> String {
        self.public_key().to_public_key_pem()
    }

    /// Big-endian modulus and exponent (a JWK's `n` and `e`).
    pub(crate) fn n_e(&self) -> (Vec<u8>, Vec<u8>) {
        let key = self.public_key();
        (key.modulus().to_vec(), key.exponent_bytes())
    }

    pub(crate) fn public_key(&self) -> RsaPublicKey {
        RsaPublicKey::from_pkcs1_der(&self.pkcs1_der()).expect("own public key parses")
    }
}

/// One DER element.
pub(crate) fn der(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let len = content.len();
    if len < 0x80 {
        out.push(len as u8);
    } else {
        let bytes = len.to_be_bytes();
        let first = bytes.iter().position(|&b| b != 0).unwrap();
        out.push(0x80 | (bytes.len() - first) as u8);
        out.extend_from_slice(&bytes[first..]);
    }
    out.extend_from_slice(content);
    out
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(TABLE[(n >> (18 - 6 * i)) as usize & 63] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn spki_der_for(pkcs1: &[u8]) -> Vec<u8> {
    let algorithm = der(0x30, &[der(0x06, &super::RSA_ENCRYPTION_OID), vec![0x05, 0x00]].concat());
    let mut bit_string = vec![0x00];
    bit_string.extend_from_slice(pkcs1);
    der(0x30, &[algorithm, der(0x03, &bit_string)].concat())
}

/// A DER INTEGER for a non-negative big-endian magnitude.
fn der_uint(magnitude: &[u8]) -> Vec<u8> {
    let first = magnitude.iter().position(|&b| b != 0).unwrap_or(magnitude.len().saturating_sub(1));
    let mut content = magnitude[first..].to_vec();
    if content.first().is_some_and(|b| b & 0x80 != 0) {
        content.insert(0, 0);
    }
    der(0x02, &content)
}

/// Encoders for test fixtures (the `rsa` crate's `EncodePublicKey` and
/// `PublicKeyParts`, in the shapes the tests used).
impl RsaPublicKey {
    pub(crate) fn to_public_key_der(&self) -> Vec<u8> {
        let pkcs1 = der(0x30, &[der_uint(self.modulus()), der_uint(&self.exponent().to_be_bytes())].concat());
        spki_der_for(&pkcs1)
    }

    pub(crate) fn to_public_key_pem(&self) -> String {
        let body = base64_encode(&self.to_public_key_der());
        let lines: Vec<&str> = body.as_bytes().chunks(64).map(|c| std::str::from_utf8(c).unwrap()).collect();
        format!("-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----\n", lines.join("\n"))
    }

    /// Big-endian exponent without leading zeros (a JWK's `e`).
    pub(crate) fn exponent_bytes(&self) -> Vec<u8> {
        let e = self.exponent().to_be_bytes();
        let first = e.iter().position(|&b| b != 0).unwrap_or(7);
        e[first..].to_vec()
    }
}
