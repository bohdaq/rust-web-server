//! Small shared utilities: [`base64`] and the [`New`] trait (a constructor
//! used across the crate, much like `Default`).

pub mod base64;

pub trait New {
    fn new() -> Self;
}
