//! The part of `storage` the server layer needs: [`StorageError`] and the
//! AWS (SigV4, credential chain) and Azure (Managed Identity) credential
//! helpers `secrets` shares with the S3 and Azure Blob backends, which are
//! rust-web-server's `storage`.

// Also compiled under `secrets` (not just `storage-s3`) — `secrets::aws_secrets_manager`
// reuses this exact SigV4 signer and IRSA/ECS/IMDSv2 credential chain rather than
// duplicating AWS's auth story a second time.
#[cfg(any(feature = "storage-s3", feature = "secrets"))]
#[doc(hidden)]
pub mod aws_credentials;
#[cfg(any(feature = "storage-s3", feature = "secrets"))]
#[doc(hidden)]
pub mod aws_sigv4;
// Also compiled under `secrets` — `secrets::azure_key_vault` reuses the Managed
// Identity token fetch (parameterized by `resource`) rather than duplicating it.
#[cfg(any(feature = "storage-azure", feature = "secrets"))]
#[doc(hidden)]
pub mod azure_credentials;

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
