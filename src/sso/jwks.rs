//! JSON Web Key Set (JWKS) cache and JWT verification.
//!
//! [`JwksCache`] fetches and caches public keys from a JWKS endpoint, then
//! verifies RS256 and ES256 JWTs with those keys.  It re-fetches on the first
//! verification failure to handle key rotation.
//!
//! [`OidcClaims`] holds the standard OIDC claims extracted from a verified
//! id_token payload.

use std::sync::Mutex;

use p256::ecdsa::signature::Verifier as EcVerifier;
use p256::ecdsa::{Signature as EcSignature, VerifyingKey as EcVerifyingKey};
#[allow(deprecated)]
use p256::elliptic_curve::generic_array::GenericArray;
use p256::EncodedPoint;
use rsa::pkcs1v15::VerifyingKey as RsaVerifyingKey;
use rsa::{BigUint, RsaPublicKey};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::http_client::Client;

use super::pkce::base64url_decode;
use super::SsoError;

/// Standard OIDC claims extracted from an id_token or userinfo response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcClaims {
    pub sub:            String,
    pub iss:            String,
    pub aud:            Vec<String>,
    pub exp:            u64,
    pub iat:            u64,
    pub nonce:          Option<String>,
    pub email:          Option<String>,
    pub email_verified: Option<bool>,
    pub name:           Option<String>,
    pub given_name:     Option<String>,
    pub family_name:    Option<String>,
    pub picture:        Option<String>,
    pub locale:         Option<String>,
}

/// Options for JWT verification.
pub struct VerifyOptions<'a> {
    /// Expected `aud` claim value — typically the OAuth client ID.
    pub audience:    &'a str,
    /// Expected `iss` claim value.
    pub issuer:      &'a str,
    /// Clock skew tolerance in seconds.
    pub leeway_secs: u64,
}

// ── internal key cache ────────────────────────────────────────────────────────

enum CachedKey {
    Rsa { n: Vec<u8>, e: Vec<u8> },
    Ec  { x: Vec<u8>, y: Vec<u8> },
}

struct JwkEntry {
    kid: Option<String>,
    key: CachedKey,
}

/// Thread-safe cache of public keys from a JWKS endpoint.
pub struct JwksCache {
    uri:  String,
    keys: Mutex<Vec<JwkEntry>>,
}

impl JwksCache {
    /// Create a new (empty) cache for the given JWKS URI.
    pub fn new(uri: &str) -> Self {
        JwksCache { uri: uri.to_string(), keys: Mutex::new(Vec::new()) }
    }

    /// Fetch keys from the JWKS URI and replace the cache.
    pub fn fetch(&self) -> Result<(), SsoError> {
        let resp = Client::new()
            .get(&self.uri)
            .timeout_ms(10_000)
            .send()
            .map_err(|e| SsoError(format!("JWKS fetch failed: {e}")))?;
        if !resp.is_success() {
            return Err(SsoError(format!("JWKS returned {}", resp.status())));
        }
        let body = resp.text().map_err(|e| SsoError(e.to_string()))?;
        let entries = parse_jwks(&body)?;
        *self.keys.lock().unwrap() = entries;
        Ok(())
    }

    /// Verify a JWT and return the parsed claims.
    ///
    /// Fetches keys on first call or on signature failure (key rotation).
    pub fn verify_jwt(
        &self,
        token: &str,
        opts: &VerifyOptions<'_>,
    ) -> Result<OidcClaims, SsoError> {
        let parts: Vec<&str> = token.splitn(3, '.').collect();
        if parts.len() != 3 {
            return Err(SsoError("JWT must have 3 parts".into()));
        }

        let header_bytes = base64url_decode(parts[0])?;
        let header_json = std::str::from_utf8(&header_bytes)
            .map_err(|_| SsoError("JWT header is not UTF-8".into()))?;

        let alg = json_str(header_json, "alg").unwrap_or_default();
        let kid = json_str(header_json, "kid");

        let message = format!("{}.{}", parts[0], parts[1]).into_bytes();
        let signature = base64url_decode(parts[2])?;

        // Lazy-load keys on first use
        {
            let keys = self.keys.lock().unwrap();
            if keys.is_empty() {
                drop(keys);
                self.fetch()?;
            }
        }

        let verified =
            self.try_verify(&message, &signature, &alg, kid.as_deref())?;
        if !verified {
            // Try re-fetching in case of key rotation
            self.fetch()?;
            if !self.try_verify(&message, &signature, &alg, kid.as_deref())? {
                return Err(SsoError("JWT signature verification failed".into()));
            }
        }

        let payload_bytes = base64url_decode(parts[1])?;
        let payload_json = std::str::from_utf8(&payload_bytes)
            .map_err(|_| SsoError("JWT payload is not UTF-8".into()))?;

        let claims = parse_claims(payload_json)?;

        // Validate standard claims
        let now = unix_now();
        if claims.exp + opts.leeway_secs < now {
            return Err(SsoError(format!(
                "JWT expired (exp={}, now={})",
                claims.exp, now
            )));
        }
        if claims.iat > now + opts.leeway_secs {
            return Err(SsoError("JWT issued in the future".into()));
        }
        if claims.iss != opts.issuer {
            return Err(SsoError(format!(
                "JWT issuer mismatch: expected {}, got {}",
                opts.issuer, claims.iss
            )));
        }
        if !claims.aud.iter().any(|a| a == opts.audience) {
            return Err(SsoError(format!(
                "JWT audience does not include {}",
                opts.audience
            )));
        }

        Ok(claims)
    }

    #[allow(deprecated)]
    fn try_verify(
        &self,
        message: &[u8],
        signature: &[u8],
        alg: &str,
        kid: Option<&str>,
    ) -> Result<bool, SsoError> {
        let keys = self.keys.lock().unwrap();
        let candidates: Vec<&JwkEntry> = if let Some(kid_val) = kid {
            keys.iter()
                .filter(|k| k.kid.as_deref() == Some(kid_val))
                .collect()
        } else {
            keys.iter().collect()
        };

        for entry in candidates {
            match (&entry.key, alg) {
                (CachedKey::Rsa { n, e }, "RS256") => {
                    let pub_key = RsaPublicKey::new(
                        BigUint::from_bytes_be(n),
                        BigUint::from_bytes_be(e),
                    )
                    .map_err(|err| SsoError(format!("invalid RSA key: {err}")))?;
                    let vk = RsaVerifyingKey::<Sha256>::new(pub_key);
                    use rsa::pkcs1v15::Signature as RsaSig;
                    if let Ok(sig) = RsaSig::try_from(signature) {
                        if vk.verify(message, &sig).is_ok() {
                            return Ok(true);
                        }
                    }
                }
                (CachedKey::Ec { x, y }, "ES256") => {
                    if x.len() != 32 || y.len() != 32 {
                        continue;
                    }
                    let xb = GenericArray::from_slice(x.as_slice());
                    let yb = GenericArray::from_slice(y.as_slice());
                    let point = EncodedPoint::from_affine_coordinates(xb, yb, false);
                    if let Ok(vk) = EcVerifyingKey::from_encoded_point(&point) {
                        // JWT ES256 signatures are r||s (64 bytes), not DER
                        let sig_arr: &[u8; 64] = signature
                            .try_into()
                            .map_err(|_| SsoError("ES256 sig wrong length".into()))?;
                        if let Ok(sig) = EcSignature::from_bytes(sig_arr.into()) {
                            if vk.verify(message, &sig).is_ok() {
                                return Ok(true);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(false)
    }
}

// ── JWKS JSON parsing ─────────────────────────────────────────────────────────

fn parse_jwks(json: &str) -> Result<Vec<JwkEntry>, SsoError> {
    // Find the start of the "keys" array
    let keys_start = json
        .find("\"keys\"")
        .and_then(|p| json[p..].find('[').map(|q| p + q + 1))
        .ok_or_else(|| SsoError("JWKS missing 'keys' array".into()))?;

    let arr = &json[keys_start..];
    let mut entries = Vec::new();
    let mut depth = 0i32;
    let mut obj_start = None;

    for (i, ch) in arr.char_indices() {
        match ch {
            '{' => {
                if depth == 0 {
                    obj_start = Some(i);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(start) = obj_start {
                        let obj = &arr[start..=i];
                        if let Some(entry) = parse_jwk_object(obj) {
                            entries.push(entry);
                        }
                        obj_start = None;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(entries)
}

fn parse_jwk_object(obj: &str) -> Option<JwkEntry> {
    let kty = json_str(obj, "kty")?;
    let kid = json_str(obj, "kid");
    match kty.as_str() {
        "RSA" => {
            let n = base64url_decode(&json_str(obj, "n")?).ok()?;
            let e = base64url_decode(&json_str(obj, "e")?).ok()?;
            Some(JwkEntry { kid, key: CachedKey::Rsa { n, e } })
        }
        "EC" => {
            let crv = json_str(obj, "crv")?;
            if crv != "P-256" {
                return None;
            }
            let x = base64url_decode(&json_str(obj, "x")?).ok()?;
            let y = base64url_decode(&json_str(obj, "y")?).ok()?;
            Some(JwkEntry { kid, key: CachedKey::Ec { x, y } })
        }
        _ => None,
    }
}

// ── claims parsing ────────────────────────────────────────────────────────────

fn parse_claims(json: &str) -> Result<OidcClaims, SsoError> {
    let sub = json_str(json, "sub")
        .ok_or_else(|| SsoError("JWT missing 'sub' claim".into()))?;
    let iss = json_str(json, "iss").unwrap_or_default();
    let exp = json_u64(json, "exp")
        .ok_or_else(|| SsoError("JWT missing 'exp' claim".into()))?;
    let iat = json_u64(json, "iat").unwrap_or(0);
    let nonce = json_str(json, "nonce");
    let email = json_str(json, "email");
    let email_verified = json_bool(json, "email_verified");
    let name = json_str(json, "name");
    let given_name = json_str(json, "given_name");
    let family_name = json_str(json, "family_name");
    let picture = json_str(json, "picture");
    let locale = json_str(json, "locale");
    let aud = parse_aud(json);

    Ok(OidcClaims {
        sub,
        iss,
        aud,
        exp,
        iat,
        nonce,
        email,
        email_verified,
        name,
        given_name,
        family_name,
        picture,
        locale,
    })
}

fn parse_aud(json: &str) -> Vec<String> {
    let needle = "\"aud\"";
    let Some(pos) = json.find(needle) else {
        return vec![];
    };
    let rest =
        json[pos + needle.len()..].trim_start_matches(|c: char| c.is_whitespace() || c == ':');
    if rest.starts_with('[') {
        // Array form
        let end = rest.find(']').unwrap_or(rest.len());
        rest[1..end]
            .split(',')
            .filter_map(|s| {
                let s = s.trim();
                if s.starts_with('"') && s.ends_with('"') {
                    Some(s[1..s.len() - 1].to_string())
                } else {
                    None
                }
            })
            .collect()
    } else if rest.starts_with('"') {
        let inner = &rest[1..];
        let end = inner.find('"').unwrap_or(inner.len());
        vec![inner[..end].to_string()]
    } else {
        vec![]
    }
}

// ── minimal JSON field extractors ─────────────────────────────────────────────

/// Extract a JSON string field by key (handles simple escape sequences).
pub(crate) fn json_str(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let start = json.find(&needle)? + needle.len();
    let rest = json[start..].trim_start_matches(|c: char| c.is_whitespace() || c == ':');
    if rest.starts_with('"') {
        let inner = &rest[1..];
        let mut escaped = false;
        let mut out = String::new();
        for ch in inner.chars() {
            if escaped {
                out.push(ch);
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '"' {
                return Some(out);
            }
            out.push(ch);
        }
        None
    } else {
        None
    }
}

fn json_u64(json: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{key}\"");
    let start = json.find(&needle)? + needle.len();
    let rest = json[start..].trim_start_matches(|c: char| c.is_whitespace() || c == ':');
    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn json_bool(json: &str, key: &str) -> Option<bool> {
    let needle = format!("\"{key}\"");
    let start = json.find(&needle)? + needle.len();
    let rest = json[start..].trim_start_matches(|c: char| c.is_whitespace() || c == ':');
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
