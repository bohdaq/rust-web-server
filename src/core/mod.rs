//! Small shared utilities: [`base64`], [`sha256`] (SHA-256, HMAC-SHA256) and the [`New`] trait (a constructor
//! used across the crate, much like `Default`).

pub mod base64;
pub mod sha256;

pub trait New {
    fn new() -> Self;
}
