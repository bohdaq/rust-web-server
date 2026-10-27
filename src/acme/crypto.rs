use aws_lc_rs::digest as lc_digest;
use aws_lc_rs::signature::{EcdsaKeyPair, KeyPair as SignKeyPair, ECDSA_P256_SHA256_FIXED_SIGNING};

// ── AccountKey ────────────────────────────────────────────────────────────────

/// ACME account key backed by ECDSA P-256.  Signing produces the 64-byte
/// fixed r||s format required by JOSE (JWS compact form).
pub struct AccountKey {
    inner: EcdsaKeyPair,
}

impl AccountKey {
    /// Generate a fresh P-256 key pair. Returns `(key, pkcs8_der)`.
    pub fn generate() -> Result<(Self, Vec<u8>), String> {
        let key = EcdsaKeyPair::generate(&ECDSA_P256_SHA256_FIXED_SIGNING)
            .map_err(|_| "account key generation failed".to_string())?;
        let doc = key.to_pkcs8v1()
            .map_err(|_| "account key serialization failed".to_string())?;
        let der = doc.as_ref().to_vec();
        Ok((AccountKey { inner: key }, der))
    }

    /// Load a key from raw PKCS#8 DER bytes.
    pub fn from_pkcs8(der: &[u8]) -> Result<Self, String> {
        let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, der)
            .map_err(|e| format!("account key parse failed: {e}"))?;
        Ok(AccountKey { inner: key })
    }

    /// Sign `data` with ECDSA P-256 SHA-256.  Returns raw r||s (64 bytes).
    pub fn sign(&self, data: &[u8]) -> Result<[u8; 64], String> {
        let rng = aws_lc_rs::rand::SystemRandom::new();
        let sig = self.inner.sign(&rng, data)
            .map_err(|_| "ECDSA signing failed".to_string())?;
        let bytes = sig.as_ref();
        if bytes.len() != 64 {
            return Err(format!("unexpected ECDSA fixed signature length: {}", bytes.len()));
        }
        let mut out = [0u8; 64];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    /// Returns the uncompressed EC public key: `04 || x(32) || y(32)` (65 bytes).
    pub fn public_key_raw(&self) -> &[u8] {
        self.inner.public_key().as_ref()
    }
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let d = lc_digest::digest(&lc_digest::SHA256, data);
    let mut out = [0u8; 32];
    out.copy_from_slice(d.as_ref());
    out
}

// ── EC key helpers ────────────────────────────────────────────────────────────

/// Extract x, y from an uncompressed EC P-256 point `04 || x(32) || y(32)`.
pub fn ec_point_xy(raw: &[u8]) -> Result<([u8; 32], [u8; 32]), String> {
    if raw.len() != 65 || raw[0] != 0x04 {
        return Err(format!(
            "unexpected EC public key: {} bytes, first byte 0x{:02x}",
            raw.len(),
            raw.first().copied().unwrap_or(0)
        ));
    }
    let mut x = [0u8; 32];
    let mut y = [0u8; 32];
    x.copy_from_slice(&raw[1..33]);
    y.copy_from_slice(&raw[33..65]);
    Ok((x, y))
}

/// Build the canonical JWK JSON for ES256 (sorted keys — required for thumbprint).
pub fn ec_jwk_json(x: &[u8; 32], y: &[u8; 32]) -> String {
    // RFC 7638 §3.3: keys must be sorted alphabetically ("crv", "kty", "x", "y")
    format!(
        r#"{{"crv":"P-256","kty":"EC","x":"{}","y":"{}"}}"#,
        base64url(x),
        base64url(y),
    )
}

/// Compute the JOSE key thumbprint (RFC 7638) for a P-256 key pair.
pub fn key_thumbprint(key: &AccountKey) -> Result<String, String> {
    let (x, y) = ec_point_xy(key.public_key_raw())?;
    Ok(base64url(&sha256(ec_jwk_json(&x, &y).as_bytes())))
}

// ── JWS construction ──────────────────────────────────────────────────────────

/// Build a JWS-signed POST body for an ACME request.
///
/// - `account_url = None` → use `"jwk"` (for `newAccount`)
/// - `account_url = Some(_)` → use `"kid"` (for all subsequent requests)
/// - `payload = None` → POST-as-GET (empty payload)
pub fn build_jws(
    key: &AccountKey,
    nonce: &str,
    url: &str,
    account_url: Option<&str>,
    payload: Option<&str>,
) -> Result<String, String> {
    let protected = match account_url {
        Some(kid) => format!(
            r#"{{"alg":"ES256","kid":"{kid}","nonce":"{nonce}","url":"{url}"}}"#,
            kid = kid,
            nonce = nonce,
            url = url,
        ),
        None => {
            let (x, y) = ec_point_xy(key.public_key_raw())?;
            let jwk = ec_jwk_json(&x, &y);
            format!(
                r#"{{"alg":"ES256","jwk":{jwk},"nonce":"{nonce}","url":"{url}"}}"#,
                jwk = jwk,
                nonce = nonce,
                url = url,
            )
        }
    };

    let protected_b64 = base64url(protected.as_bytes());
    let payload_b64 = payload.map(|p| base64url(p.as_bytes())).unwrap_or_default();

    let signing_input = format!("{}.{}", protected_b64, payload_b64);
    let sig_bytes = key.sign(signing_input.as_bytes())?;
    let sig_b64 = base64url(&sig_bytes);

    Ok(format!(
        r#"{{"protected":"{protected_b64}","payload":"{payload_b64}","signature":"{sig_b64}"}}"#,
        protected_b64 = protected_b64,
        payload_b64 = payload_b64,
        sig_b64 = sig_b64,
    ))
}

pub use crate::core::x509::{base64_decode_std, base64url, cert_info_from_pem, days_since_epoch, read_cert_info, CertInfo};
#[cfg(any(test, fuzzing))]
pub use crate::core::x509::cert_info_from_der;

