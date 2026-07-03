//! Built-in authentication middleware (`auth` Cargo feature).
//!
//! Enable with `features = ["auth"]` in your `Cargo.toml`. Adds `hmac` and
//! `sha2` (RustCrypto) as dependencies.
//!
//! # HTTP Basic Auth
//!
//! [`BasicAuthLayer`] validates `Authorization: Basic <base64>` credentials
//! against a caller-supplied closure. Issues a `WWW-Authenticate` challenge
//! when the header is absent.
//!
//! # JWT (HS256)
//!
//! [`JwtLayer`] verifies `Authorization: Bearer <token>` JWTs signed with
//! HMAC-SHA256. Tokens with a past `exp` claim are rejected. Use
//! [`verify_jwt`] directly in a handler if you also need the decoded
//! [`Claims`].
//!
//! # JWT (RS256 / ES256), `auth-asymmetric` feature
//!
//! [`JwtLayer::rs256`] and [`JwtLayer::es256`] verify JWTs signed with a
//! separate auth server's RSA or P-256 private key — the caller presents a
//! token, `JwtLayer` only needs the corresponding *public* key (PEM,
//! SubjectPublicKeyInfo format) to verify it. No JWKS endpoint or the full
//! `sso` feature is required; use [`crate::sso::JwksCache`] instead if you
//! need multi-key rotation via a live JWKS URL.
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::auth::{BasicAuthLayer, JwtLayer};
//! use rust_web_server::core::New;
//!
//! // Basic Auth
//! let app = App::new()
//!     .wrap(BasicAuthLayer::new(|user, pass| user == "admin" && pass == "secret"));
//!
//! // JWT (HS256)
//! let app = App::new()
//!     .wrap(JwtLayer::new(b"my-signing-secret"));
//! ```
//!
//! ```rust,no_run
//! # #[cfg(feature = "auth-asymmetric")] {
//! use rust_web_server::app::App;
//! use rust_web_server::auth::JwtLayer;
//! use rust_web_server::core::New;
//!
//! // JWT (RS256) — public_key_pem is a SubjectPublicKeyInfo PEM string,
//! // e.g. produced by `openssl rsa -in key.pem -pubout`.
//! let public_key_pem = "-----BEGIN PUBLIC KEY-----\n...\n-----END PUBLIC KEY-----\n";
//! let app = App::new().wrap(JwtLayer::rs256(public_key_pem).unwrap());
//! # }
//! ```

pub mod forward;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

use crate::application::Application;
use crate::error::{AppError, IntoResponse};
use crate::header::Header;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

type HmacSha256 = Hmac<Sha256>;

// ── Base64 helpers ────────────────────────────────────────────────────────────

// Decodes standard base64 (+/) and base64url (-_) — accepts either alphabet.
// Padding characters ('=') are stripped before decoding.
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let bytes: Vec<u8> = input.bytes().filter(|&b| b != b'=').collect();
    if bytes.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let a = b64_val(chunk[0])?;
        let b = b64_val(chunk[1])?;
        out.push((a << 2) | (b >> 4));
        if chunk.len() > 2 {
            let c = b64_val(chunk[2])?;
            out.push((b << 4) | (c >> 2));
            if chunk.len() > 3 {
                let d = b64_val(chunk[3])?;
                out.push((c << 6) | d);
            }
        }
    }
    Some(out)
}

fn b64_val(b: u8) -> Option<u8> {
    match b {
        b'A'..=b'Z' => Some(b - b'A'),
        b'a'..=b'z' => Some(b - b'a' + 26),
        b'0'..=b'9' => Some(b - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    }
}

// URL-safe base64 encoding without padding — used for JWT signature computation.
fn base64url_encode(input: &[u8]) -> String {
    const C: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 { chunk[1] as usize } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as usize } else { 0 };
        out.push(C[b0 >> 2] as char);
        out.push(C[((b0 & 3) << 4) | (b1 >> 4)] as char);
        if chunk.len() > 1 { out.push(C[((b1 & 0xf) << 2) | (b2 >> 6)] as char); }
        if chunk.len() > 2 { out.push(C[b2 & 0x3f] as char); }
    }
    out
}

// Standard base64 encoding with padding — used to build Basic Auth headers.
// pub(crate): reused by proxy_config's tests to build Authorization headers.
pub(crate) fn base64_encode(input: &[u8]) -> String {
    const C: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 { chunk[1] as usize } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as usize } else { 0 };
        out.push(C[b0 >> 2] as char);
        out.push(C[((b0 & 3) << 4) | (b1 >> 4)] as char);
        out.push(if chunk.len() > 1 { C[((b1 & 0xf) << 2) | (b2 >> 6)] as char } else { '=' });
        out.push(if chunk.len() > 2 { C[b2 & 0x3f] as char } else { '=' });
    }
    out
}

// ── Mini JSON claim extractor ─────────────────────────────────────────────────

fn extract_string_claim(json: &str, field: &str) -> Option<String> {
    let key = format!("\"{}\"", field);
    let start = json.find(key.as_str())?;
    let rest = json[start + key.len()..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    Some(rest[..rest.find('"')?].to_string())
}

fn extract_u64_claim(json: &str, field: &str) -> Option<u64> {
    let key = format!("\"{}\"", field);
    let start = json.find(key.as_str())?;
    let rest = json[start + key.len()..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

// ── Claims ────────────────────────────────────────────────────────────────────

/// Decoded JWT payload.
///
/// Standard claims (`sub`, `exp`) are pre-extracted. For other claims, parse
/// [`Claims::raw`] with `serde_json` or the built-in json module.
pub struct Claims {
    /// The `sub` (subject) claim, if present.
    pub sub: Option<String>,
    /// The `exp` (expiration) claim as Unix seconds, if present.
    pub exp: Option<u64>,
    /// Raw UTF-8 JSON payload — inspect for custom claims.
    pub raw: String,
}

impl Claims {
    fn from_json(json: String) -> Self {
        Claims {
            sub: extract_string_claim(&json, "sub"),
            exp: extract_u64_claim(&json, "exp"),
            raw: json,
        }
    }

    /// Return `true` if the token is not yet expired at `now_secs` (Unix
    /// timestamp). Returns `true` when `exp` is absent (no expiry set).
    pub fn is_valid_at(&self, now_secs: u64) -> bool {
        self.exp.map_or(true, |exp| now_secs < exp)
    }
}

// ── JWT helpers ───────────────────────────────────────────────────────────────

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Extract the raw token string from `Authorization: Bearer <token>`.
/// Returns `None` if the header is absent or does not start with `Bearer `.
pub fn extract_bearer_token(request: &Request) -> Option<String> {
    let h = request.get_header(Header::_AUTHORIZATION.to_string())?;
    h.value.strip_prefix("Bearer ").map(str::to_string)
}

/// Build a signed HS256 JWT from a JSON claims object.
///
/// Useful for generating test tokens or issuing tokens from a login handler.
///
/// ```rust,no_run
/// use rust_web_server::auth::build_jwt;
///
/// let token = build_jwt(r#"{"sub":"42","exp":9999999999}"#, b"secret");
/// ```
pub fn build_jwt(claims_json: &str, secret: &[u8]) -> String {
    let header = base64url_encode(br#"{"alg":"HS256","typ":"JWT"}"#);
    let payload = base64url_encode(claims_json.as_bytes());
    let message = format!("{}.{}", header, payload);
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts any key size");
    mac.update(message.as_bytes());
    let sig = mac.finalize().into_bytes();
    format!("{}.{}.{}", header, payload, base64url_encode(&sig))
}

/// Verify a JWT string against `secret` (HS256 only).
///
/// Returns [`Claims`] on success. Returns `None` on any failure: bad format,
/// unsupported algorithm, signature mismatch, or expired `exp` claim.
pub fn verify_jwt(token: &str, secret: &[u8]) -> Option<Claims> {
    let mut parts = token.splitn(3, '.');
    let header_b64 = parts.next()?;
    let payload_b64 = parts.next()?;
    let sig_b64 = parts.next()?;

    if sig_b64.contains('.') {
        return None; // more than 3 parts
    }

    // Verify algorithm is HS256
    let header_bytes = base64_decode(header_b64)?;
    let header_str = String::from_utf8(header_bytes).ok()?;
    if !header_str.contains("\"HS256\"") {
        return None;
    }

    // Constant-time signature verification
    let message = format!("{}.{}", header_b64, payload_b64);
    let expected = base64_decode(sig_b64)?;
    let mut mac = HmacSha256::new_from_slice(secret).ok()?;
    mac.update(message.as_bytes());
    mac.verify_slice(&expected).ok()?;

    // Decode claims
    let payload_bytes = base64_decode(payload_b64)?;
    let payload_str = String::from_utf8(payload_bytes).ok()?;
    let claims = Claims::from_json(payload_str);

    // Reject expired tokens
    if !claims.is_valid_at(unix_now()) {
        return None;
    }

    Some(claims)
}

/// Splits a JWT into its three base64url segments. Returns `None` if the
/// token doesn't have exactly three `.`-separated parts.
#[cfg(feature = "auth-asymmetric")]
fn split_jwt(token: &str) -> Option<(&str, &str, &str)> {
    let mut parts = token.splitn(3, '.');
    let header_b64 = parts.next()?;
    let payload_b64 = parts.next()?;
    let sig_b64 = parts.next()?;
    if sig_b64.contains('.') {
        return None; // more than 3 parts
    }
    Some((header_b64, payload_b64, sig_b64))
}

/// Shared tail of `verify_jwt_rs256`/`verify_jwt_es256`, run after the
/// signature has already checked out: confirms the header's `alg` matches
/// what was actually verified (so an RS256 key can't be used to wave
/// through a token whose header merely claims HS256), decodes the claims,
/// and rejects expired tokens.
#[cfg(feature = "auth-asymmetric")]
fn finish_asymmetric_verification(header_str: &str, expected_alg: &str, payload_b64: &str) -> Option<Claims> {
    if !header_str.contains(&format!("\"{}\"", expected_alg)) {
        return None;
    }
    let payload_bytes = base64_decode(payload_b64)?;
    let payload_str = String::from_utf8(payload_bytes).ok()?;
    let claims = Claims::from_json(payload_str);
    if !claims.is_valid_at(unix_now()) {
        return None;
    }
    Some(claims)
}

/// Verify a JWT signed with RS256 (RSASSA-PKCS1-v1_5 using SHA-256) against
/// an RSA public key.
///
/// Returns [`Claims`] on success. Returns `None` on any failure: bad format,
/// a header `alg` other than `"RS256"`, signature mismatch, or an expired
/// `exp` claim.
#[cfg(feature = "auth-asymmetric")]
pub fn verify_jwt_rs256(token: &str, public_key: &rsa::RsaPublicKey) -> Option<Claims> {
    use rsa::pkcs1v15::{Signature as RsaSignature, VerifyingKey as RsaVerifyingKey};
    use rsa::signature::Verifier;

    let (header_b64, payload_b64, sig_b64) = split_jwt(token)?;
    let header_str = String::from_utf8(base64_decode(header_b64)?).ok()?;

    let message = format!("{}.{}", header_b64, payload_b64);
    let signature = base64_decode(sig_b64)?;

    let verifying_key = RsaVerifyingKey::<Sha256>::new(public_key.clone());
    let sig = RsaSignature::try_from(signature.as_slice()).ok()?;
    verifying_key.verify(message.as_bytes(), &sig).ok()?;

    finish_asymmetric_verification(&header_str, "RS256", payload_b64)
}

/// Verify a JWT signed with ES256 (ECDSA using P-256 and SHA-256) against a
/// P-256 public key.
///
/// Returns [`Claims`] on success. Returns `None` on any failure: bad format,
/// a header `alg` other than `"ES256"`, signature mismatch, or an expired
/// `exp` claim.
///
/// JWT ES256 signatures are the raw 64-byte `r || s` concatenation, not the
/// ASN.1 DER encoding OpenSSL produces by default.
#[cfg(feature = "auth-asymmetric")]
pub fn verify_jwt_es256(token: &str, public_key: &p256::ecdsa::VerifyingKey) -> Option<Claims> {
    use p256::ecdsa::signature::Verifier;
    use p256::ecdsa::Signature as EcSignature;

    let (header_b64, payload_b64, sig_b64) = split_jwt(token)?;
    let header_str = String::from_utf8(base64_decode(header_b64)?).ok()?;

    let message = format!("{}.{}", header_b64, payload_b64);
    let signature = base64_decode(sig_b64)?;
    if signature.len() != 64 {
        return None;
    }
    let sig = EcSignature::from_slice(&signature).ok()?;
    public_key.verify(message.as_bytes(), &sig).ok()?;

    finish_asymmetric_verification(&header_str, "ES256", payload_b64)
}

// ── BasicAuthLayer ────────────────────────────────────────────────────────────

/// Middleware that validates HTTP Basic Auth credentials.
///
/// Issues `401 Unauthorized` with `WWW-Authenticate: Basic realm="Protected"`
/// when the header is absent or malformed. Issues `401` (without the
/// challenge) when credentials are present but the validator returns `false`.
///
/// Passwords containing `:` are handled correctly (only the first `:` splits
/// username from password, per RFC 7617).
///
/// # Example
///
/// ```rust,no_run
/// use rust_web_server::app::App;
/// use rust_web_server::auth::BasicAuthLayer;
/// use rust_web_server::core::New;
///
/// let app = App::new().wrap(BasicAuthLayer::new(|user, pass| {
///     user == "admin" && pass == "s3cret"
/// }));
/// ```
pub struct BasicAuthLayer<F> {
    validate: F,
}

impl<F: Fn(&str, &str) -> bool + Send + Sync + 'static> BasicAuthLayer<F> {
    /// Create a layer with a `validate(username, password) -> bool` closure.
    pub fn new(validate: F) -> Self {
        BasicAuthLayer { validate }
    }
}

impl BasicAuthLayer<Box<dyn Fn(&str, &str) -> bool + Send + Sync>> {
    /// Create a layer that validates credentials against an htpasswd-style
    /// file, loaded once at construction time (not re-read per request).
    ///
    /// Each non-empty, non-comment (`#`-prefixed) line must be
    /// `username:credential`, where `credential` is one of:
    /// - a plain-text password (Apache's `htpasswd -p` format), or
    /// - `{SHA256}` followed by the base64-encoded SHA-256 digest of the
    ///   password — **this is rws's own scheme, not Apache's**.
    ///
    /// # Not supported
    ///
    /// Apache's real `{SHA}` scheme (SHA-1), `$apr1$` (iterated MD5), and
    /// bcrypt (`$2y$`/`$2b$`) are **not** supported — this crate has no
    /// third-party HTTP/crypto dependencies beyond the audited RustCrypto
    /// hash crates it already uses, and hand-rolling SHA-1/MD5/bcrypt from
    /// scratch is not a risk worth taking for a security check. A real
    /// Apache-generated htpasswd file (which defaults to bcrypt or `$apr1$`
    /// in modern `htpasswd` versions) will **not** verify against this —
    /// regenerate it with `htpasswd -p` (plain text) or write your own
    /// `{SHA256}` entries, or use [`BasicAuthLayer::new`] with your own
    /// closure (e.g. backed by the `bcrypt` crate) if you need real
    /// Apache-hash compatibility.
    ///
    /// # Errors
    ///
    /// Returns `Err` if `path` can't be read.
    pub fn from_htpasswd_file(path: &str) -> Result<Self, String> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read htpasswd file '{path}': {e}"))?;
        let users = parse_htpasswd(&contents);

        let validate: Box<dyn Fn(&str, &str) -> bool + Send + Sync> =
            Box::new(move |user: &str, pass: &str| match users.get(user) {
                Some(stored) => verify_htpasswd_credential(pass, stored),
                None => false,
            });

        Ok(BasicAuthLayer::new(validate))
    }
}

fn parse_htpasswd(contents: &str) -> HashMap<String, String> {
    contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once(':'))
        .map(|(user, cred)| (user.to_string(), cred.to_string()))
        .collect()
}

fn verify_htpasswd_credential(password: &str, stored: &str) -> bool {
    match stored.strip_prefix("{SHA256}") {
        Some(expected_b64) => base64_encode(&Sha256::digest(password.as_bytes())) == expected_b64,
        None => stored == password,
    }
}

impl<F: Fn(&str, &str) -> bool + Send + Sync + 'static> Middleware for BasicAuthLayer<F> {
    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        let challenge = || {
            let mut r = AppError::Unauthorized.into_response();
            r.headers.push(Header {
                name: "WWW-Authenticate".to_string(),
                value: "Basic realm=\"Protected\"".to_string(),
            });
            r
        };

        let Some(header) = request.get_header(Header::_AUTHORIZATION.to_string()) else {
            return Ok(challenge());
        };
        let Some(encoded) = header.value.strip_prefix("Basic ") else {
            return Ok(challenge());
        };
        let Some(decoded) = base64_decode(encoded) else {
            return Ok(challenge());
        };
        let Ok(credentials) = String::from_utf8(decoded) else {
            return Ok(challenge());
        };
        let Some((user, pass)) = credentials.split_once(':') else {
            return Ok(challenge());
        };

        if (self.validate)(user, pass) {
            next.execute(request, connection)
        } else {
            Ok(AppError::Unauthorized.into_response())
        }
    }
}

// ── JwtLayer ──────────────────────────────────────────────────────────────────

/// Middleware that verifies `Authorization: Bearer <token>` JWTs.
///
/// [`JwtLayer::new`] verifies HMAC-SHA256 (HS256) tokens against a shared
/// secret. With the `auth-asymmetric` feature, [`JwtLayer::rs256`] and
/// [`JwtLayer::es256`] verify tokens signed by a separate auth server's
/// private key, given only its public key.
///
/// Rejects tokens with a past `exp` claim. All other validation (format,
/// algorithm, signature) is performed by [`verify_jwt`] / [`verify_jwt_rs256`]
/// / [`verify_jwt_es256`].
///
/// If a handler also needs the decoded claims, call the matching `verify_*`
/// function again inside the handler — verification is cheap.
///
/// # Example
///
/// ```rust,no_run
/// use rust_web_server::app::App;
/// use rust_web_server::auth::JwtLayer;
/// use rust_web_server::core::New;
///
/// let app = App::new().wrap(JwtLayer::new(b"my-signing-secret"));
/// ```
pub struct JwtLayer {
    verifier: JwtVerifier,
}

enum JwtVerifier {
    Hs256(Vec<u8>),
    #[cfg(feature = "auth-asymmetric")]
    Rs256(Box<rsa::RsaPublicKey>),
    #[cfg(feature = "auth-asymmetric")]
    Es256(Box<p256::ecdsa::VerifyingKey>),
}

impl JwtLayer {
    /// Create a layer that verifies HS256 JWTs signed with `secret`.
    pub fn new(secret: impl Into<Vec<u8>>) -> Self {
        JwtLayer { verifier: JwtVerifier::Hs256(secret.into()) }
    }

    /// Create a layer that verifies RS256 JWTs against an RSA public key.
    ///
    /// `public_key_pem` must be in SubjectPublicKeyInfo PEM format (the
    /// `-----BEGIN PUBLIC KEY-----` block produced by, e.g.,
    /// `openssl rsa -in key.pem -pubout` or `openssl req -pubkey`).
    ///
    /// # Errors
    ///
    /// Returns `Err` if `public_key_pem` is not a valid PEM-encoded RSA
    /// public key.
    #[cfg(feature = "auth-asymmetric")]
    pub fn rs256(public_key_pem: &str) -> Result<Self, String> {
        use rsa::pkcs8::DecodePublicKey;
        let key = rsa::RsaPublicKey::from_public_key_pem(public_key_pem)
            .map_err(|e| format!("invalid RSA public key: {e}"))?;
        Ok(JwtLayer { verifier: JwtVerifier::Rs256(Box::new(key)) })
    }

    /// Create a layer that verifies ES256 JWTs against a P-256 public key.
    ///
    /// `public_key_pem` must be in SubjectPublicKeyInfo PEM format (the
    /// `-----BEGIN PUBLIC KEY-----` block produced by, e.g.,
    /// `openssl ec -in key.pem -pubout`).
    ///
    /// # Errors
    ///
    /// Returns `Err` if `public_key_pem` is not a valid PEM-encoded P-256
    /// public key.
    #[cfg(feature = "auth-asymmetric")]
    pub fn es256(public_key_pem: &str) -> Result<Self, String> {
        use p256::pkcs8::DecodePublicKey;
        let key = p256::ecdsa::VerifyingKey::from_public_key_pem(public_key_pem)
            .map_err(|e| format!("invalid P-256 public key: {e}"))?;
        Ok(JwtLayer { verifier: JwtVerifier::Es256(Box::new(key)) })
    }
}

impl Middleware for JwtLayer {
    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        let claims = extract_bearer_token(request).and_then(|t| match &self.verifier {
            JwtVerifier::Hs256(secret) => verify_jwt(&t, secret),
            #[cfg(feature = "auth-asymmetric")]
            JwtVerifier::Rs256(key) => verify_jwt_rs256(&t, key),
            #[cfg(feature = "auth-asymmetric")]
            JwtVerifier::Es256(key) => verify_jwt_es256(&t, key),
        });
        match claims {
            Some(_) => next.execute(request, connection),
            None => Ok(AppError::Unauthorized.into_response()),
        }
    }
}
