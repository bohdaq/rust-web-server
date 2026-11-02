//! Small shared utilities: [`base64`], [`sha256`] (SHA-256, HMAC-SHA256) and the [`New`] trait (a constructor
//! used across the crate, much like `Default`).

pub mod base64;
pub mod json;
pub mod sha256;
pub mod x509;

/// Compares in time independent of where the inputs first differ, so response
/// timing doesn't reveal how much of a guessed token was right.
#[doc(hidden)]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

pub trait New {
    fn new() -> Self;
}
