//! File / object storage abstraction.
//!
//! `FormMultipartData::parse()` hands back raw bytes with no place to put
//! them. This module gives handlers a single `Storage` trait so the same
//! upload code works against local disk in development and an S3-compatible
//! bucket (AWS S3, Cloudflare R2, MinIO) in production.
//!
//! # Local disk (requires the `storage-local` feature)
//!
//! ```rust
//! # #[cfg(feature = "storage-local")]
//! # fn example() -> Result<(), rust_web_server::storage::StorageError> {
//! use rust_web_server::storage::{LocalStorage, Storage};
//!
//! let dir = std::env::temp_dir().join("rws-storage-doctest");
//! let store = LocalStorage::new(dir.to_str().unwrap()).with_base_url("/uploads");
//!
//! let key = store.put("avatars/42.png", b"...png bytes...", "image/png")?;
//! assert_eq!("/uploads/avatars/42.png", store.url(&key));
//!
//! let bytes = store.get(&key)?;
//! store.delete(&key)?;
//! # std::fs::remove_dir_all(&dir).ok();
//! # Ok(())
//! # }
//! ```
//!
//! # S3-compatible object storage (requires the `storage-s3` feature)
//!
//! ```rust,no_run
//! # #[cfg(feature = "storage-s3")]
//! # fn example() -> Result<(), rust_web_server::storage::StorageError> {
//! use rust_web_server::storage::{S3Storage, Storage};
//!
//! // Reads RWS_S3_BUCKET, RWS_S3_REGION, RWS_S3_ACCESS_KEY, RWS_S3_SECRET_KEY,
//! // and optionally RWS_S3_ENDPOINT (for R2 / MinIO / any S3-compatible host).
//! let store = S3Storage::from_env()?;
//!
//! store.put("avatars/42.png", b"...png bytes...", "image/png")?;
//! let bytes = store.get("avatars/42.png")?;
//! store.delete("avatars/42.png")?;
//! # Ok(())
//! # }
//! ```

#[cfg(feature = "storage-s3")]
mod aws_sigv4;

#[cfg(feature = "storage-local")]
mod local;
#[cfg(feature = "storage-s3")]
mod s3;

#[cfg(feature = "storage-local")]
pub use local::LocalStorage;
#[cfg(feature = "storage-s3")]
pub use s3::{S3Config, S3Storage};

// ── StorageError ─────────────────────────────────────────────────────────────

/// Error returned by all `Storage` operations.
#[derive(Debug)]
pub struct StorageError(pub String);

impl StorageError {
    pub fn new(msg: impl Into<String>) -> Self {
        StorageError(msg.into())
    }
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StorageError {}

// ── Storage ──────────────────────────────────────────────────────────────────

/// Backend-independent object storage.
///
/// Implement this trait to plug a custom backend (GCS, Azure Blob, a
/// database-backed store, ...) into the same handler code that already works
/// against [`LocalStorage`] and [`S3Storage`].
pub trait Storage: Send + Sync {
    /// Store `data` under `key`, overwriting any existing object at that key.
    /// Returns the key that was stored (backends that normalize or namespace
    /// keys may return a different string than the one passed in).
    fn put(&self, key: &str, data: &[u8], content_type: &str) -> Result<String, StorageError>;

    /// Retrieve the bytes stored under `key`.
    fn get(&self, key: &str) -> Result<Vec<u8>, StorageError>;

    /// Delete the object stored under `key`. Backends generally treat
    /// deleting a missing key as a no-op success, matching S3's semantics.
    fn delete(&self, key: &str) -> Result<(), StorageError>;

    /// A URL for retrieving `key`. For [`LocalStorage`] this is only
    /// meaningful if a base URL was configured (e.g. because the directory
    /// is also served as static files); for [`S3Storage`] it's the object's
    /// HTTPS URL. This performs no I/O and never fails.
    fn url(&self, key: &str) -> String;
}
