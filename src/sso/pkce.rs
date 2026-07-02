//! PKCE (Proof Key for Code Exchange) helpers — RFC 7636.
//!
//! Generate a [`PkceVerifier`] once per authorization request, store it in
//! the pre-auth session, and pass the derived [`PkceChallenge`] in the
//! authorization URL.  On the callback, supply the stored verifier string to
//! the token endpoint via `code_verifier`.

use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

/// Cryptographically random code verifier (43–128 base64url chars, per RFC 7636).
pub struct PkceVerifier(String);

/// SHA-256 base64url challenge derived from the verifier.
pub struct PkceChallenge(String);

impl PkceVerifier {
    /// Generate a fresh verifier with 32 random bytes (43-char base64url output).
    pub fn new() -> Self {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        PkceVerifier(base64url_encode(&bytes))
    }

    /// Return the verifier string.  Store this in the session before redirecting.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Derive the S256 challenge from this verifier.
    pub fn challenge(&self) -> PkceChallenge {
        let digest = Sha256::digest(self.0.as_bytes());
        PkceChallenge(base64url_encode(&digest))
    }
}

impl Default for PkceVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl PkceChallenge {
    /// Return the challenge string to include in the authorization URL.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ── base64url helpers (no external dep) ──────────────────────────────────────

const TABLE: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Encode `bytes` as base64url without padding.
pub(crate) fn base64url_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(((bytes.len() + 2) / 3) * 4);
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let b0 = bytes[i] as usize;
        let b1 = bytes[i + 1] as usize;
        let b2 = bytes[i + 2] as usize;
        out.push(TABLE[b0 >> 2] as char);
        out.push(TABLE[((b0 & 3) << 4) | (b1 >> 4)] as char);
        out.push(TABLE[((b1 & 0xf) << 2) | (b2 >> 6)] as char);
        out.push(TABLE[b2 & 0x3f] as char);
        i += 3;
    }
    let rem = bytes.len() - i;
    if rem == 1 {
        let b0 = bytes[i] as usize;
        out.push(TABLE[b0 >> 2] as char);
        out.push(TABLE[(b0 & 3) << 4] as char);
    } else if rem == 2 {
        let b0 = bytes[i] as usize;
        let b1 = bytes[i + 1] as usize;
        out.push(TABLE[b0 >> 2] as char);
        out.push(TABLE[((b0 & 3) << 4) | (b1 >> 4)] as char);
        out.push(TABLE[(b1 & 0xf) << 2] as char);
    }
    out
}

/// Decode a base64url string (with or without `=` padding; accepts `-`/`_`
/// and `+`/`/` variants).
pub(crate) fn base64url_decode(s: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4 + 1);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for ch in s.chars() {
        if ch == '=' {
            break;
        }
        let v: u32 = match ch {
            'A'..='Z' => ch as u32 - b'A' as u32,
            'a'..='z' => ch as u32 - b'a' as u32 + 26,
            '0'..='9' => ch as u32 - b'0' as u32 + 52,
            '-' | '+' => 62,
            '_' | '/' => 63,
            _ => return Err(format!("invalid base64url char: {ch}")),
        };
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}
