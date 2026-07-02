//! Password hashing and secure token generation.
//!
//! Enabled by the `crypto` Cargo feature:
//!
//! ```toml
//! rust-web-server = { version = "17", features = ["crypto"] }
//! ```
//!
//! # Password hashing
//!
//! Uses Argon2id (OWASP recommended) with a random 128-bit salt. The output
//! is a self-describing PHC string that includes the algorithm, parameters,
//! salt, and hash — store it as-is in your database.
//!
//! ```rust
//! use rust_web_server::crypto::{hash_password, verify_password};
//!
//! let hash = hash_password("hunter2").unwrap();
//! assert!(verify_password("hunter2", &hash).unwrap());
//! assert!(!verify_password("wrong", &hash).unwrap());
//! ```
//!
//! # Secure token generation
//!
//! ```rust
//! use rust_web_server::crypto::generate_token;
//!
//! let token = generate_token(32); // 32 random bytes → 64-char lowercase hex string
//! ```

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand_core::{OsRng, RngCore};

/// Error returned by crypto operations.
#[derive(Debug)]
pub struct CryptoError(pub String);

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CryptoError: {}", self.0)
    }
}

impl std::error::Error for CryptoError {}

/// Hash `password` using Argon2id with a random salt.
///
/// Returns a PHC string (`$argon2id$v=19$...`) suitable for storing in a
/// database. The salt is embedded in the string — no separate storage needed.
///
/// # Errors
///
/// Returns `CryptoError` if the hashing operation fails (extremely unlikely
/// with valid inputs).
pub fn hash_password(password: &str) -> Result<String, CryptoError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| CryptoError(e.to_string()))
}

/// Verify `password` against a PHC hash string produced by [`hash_password`].
///
/// Returns `Ok(true)` if the password matches, `Ok(false)` if it does not.
/// Comparison is constant-time to prevent timing attacks.
///
/// # Errors
///
/// Returns `CryptoError` if `hash` is not a valid PHC string.
pub fn verify_password(password: &str, hash: &str) -> Result<bool, CryptoError> {
    let parsed = PasswordHash::new(hash)
        .map_err(|e| CryptoError(format!("invalid hash string: {}", e)))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// Generate `n_bytes` of cryptographically secure random bytes and return them
/// as a lowercase hex string of length `n_bytes * 2`.
///
/// Suitable for password-reset tokens, email verification codes, and API keys.
/// Use at least 16 bytes (32 hex chars) for tokens; 32 bytes (64 hex chars) for
/// high-value secrets like API keys.
pub fn generate_token(n_bytes: usize) -> String {
    let mut bytes = vec![0u8; n_bytes];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests;
