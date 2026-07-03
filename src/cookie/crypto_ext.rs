//! Cookie signing (HMAC-SHA256) and encryption (AES-256-GCM), gated behind
//! the `crypto` Cargo feature (the same feature that provides
//! [`crate::crypto::hash_password`] and [`crate::crypto::generate_token`]).
//!
//! Plain [`super::SetCookie`] values are readable and tamperable by the
//! client — fine for a theme preference, wrong for a session token or any
//! value a handler trusts without re-verifying server-side.
//!
//! - [`signed_cookie`] / [`verify_signed_cookie`] — the value stays
//!   plain-text and readable by the client, but any tampering (to the value
//!   or the signature) is detected. Use when the client only needs to see
//!   the value, not modify it undetected — e.g. a discount code or a
//!   feature-flag override the client shouldn't be able to forge.
//! - [`encrypted_cookie`] / [`decrypt_cookie`] — the value is neither
//!   readable nor tamperable by the client. Use for anything the client
//!   shouldn't see at all, e.g. a serialized session token.
//!
//! ```rust
//! use rust_web_server::cookie::{signed_cookie, verify_signed_cookie, SetCookie};
//!
//! let secret = b"my-signing-secret";
//! let cookie_value = signed_cookie("plan=pro", secret);
//! let header_value = SetCookie::new("prefs", &cookie_value).http_only().build();
//!
//! // ... later, reading the incoming Cookie header ...
//! assert_eq!(Some("plan=pro".to_string()), verify_signed_cookie(&cookie_value, secret));
//! assert_eq!(None, verify_signed_cookie(&cookie_value, b"wrong-secret"));
//! ```

#[cfg(test)]
mod tests;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use hmac::{Hmac, Mac};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// Signs `value` with HMAC-SHA256 keyed by `secret`. Returns
/// `"<value>.<hex-signature>"` — store the result directly as a cookie value
/// (e.g. `SetCookie::new("prefs", signed_cookie(value, secret))`).
///
/// The value itself remains plain-text and readable by the client; only
/// tampering is detected, by [`verify_signed_cookie`].
pub fn signed_cookie(value: &str, secret: &[u8]) -> String {
    let sig = hmac_sha256(secret, value.as_bytes());
    format!("{}.{}", value, to_hex(&sig))
}

/// Verifies a cookie value produced by [`signed_cookie`]. Returns the
/// original value on success. Returns `None` on any failure — missing
/// separator, malformed hex, or a signature mismatch (tampering, or the
/// wrong `secret`).
///
/// Splits on the *last* `.` so a `value` that itself contains dots is
/// handled correctly — the hex-encoded HMAC signature never contains one.
pub fn verify_signed_cookie(signed: &str, secret: &[u8]) -> Option<String> {
    let (value, sig_hex) = signed.rsplit_once('.')?;
    let given_sig = from_hex(sig_hex)?;
    let mut mac = <HmacSha256 as Mac>::new_from_slice(secret).ok()?;
    mac.update(value.as_bytes());
    mac.verify_slice(&given_sig).ok()?;
    Some(value.to_string())
}

/// Encrypts `value` with AES-256-GCM. `key` may be any length — it's hashed
/// with SHA-256 first to produce the required 256-bit key, the same
/// convenience [`signed_cookie`]'s HMAC secret already offers. A fresh
/// random 96-bit nonce is generated on every call. Returns
/// `"<hex-nonce>.<hex-ciphertext-and-tag>"` — store the result directly as a
/// cookie value.
///
/// Unlike [`signed_cookie`], the value is not readable by the client at all,
/// not just tamper-evident.
pub fn encrypted_cookie(value: &str, key: &[u8]) -> String {
    let cipher = Aes256Gcm::new_from_slice(&Sha256::digest(key))
        .expect("SHA-256 digest is always 32 bytes, the size AES-256-GCM requires");

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, value.as_bytes())
        .expect("AES-256-GCM encryption of a cookie-sized value cannot fail");

    format!("{}.{}", to_hex(&nonce_bytes), to_hex(&ciphertext))
}

/// Decrypts a cookie value produced by [`encrypted_cookie`]. Returns the
/// original value on success. Returns `None` on any failure — malformed
/// input, the wrong `key`, or a tampered/corrupted ciphertext (GCM's
/// authentication tag check fails closed rather than returning garbage).
pub fn decrypt_cookie(encrypted: &str, key: &[u8]) -> Option<String> {
    let (nonce_hex, ciphertext_hex) = encrypted.split_once('.')?;
    let nonce_bytes = from_hex(nonce_hex)?;
    if nonce_bytes.len() != 12 {
        return None;
    }
    let ciphertext = from_hex(ciphertext_hex)?;

    let cipher = Aes256Gcm::new_from_slice(&Sha256::digest(key)).ok()?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plaintext = cipher.decrypt(nonce, ciphertext.as_ref()).ok()?;
    String::from_utf8(plaintext).ok()
}

fn hmac_sha256(secret: &[u8], message: &[u8]) -> Vec<u8> {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(secret).expect("HMAC accepts any key size");
    mac.update(message);
    mac.finalize().into_bytes().to_vec()
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if s.is_empty() || s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}
