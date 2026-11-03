//! Object storage for rust-web-server: local files, S3, Azure Blob and Google Cloud Storage backends.
//!
//! Internal to rust-web-server, released with it at the same version: use
//! these through the `rust_web_server` paths, which stay stable; this
//! crate's own layout may change in any release. See
//! `spec/WORKSPACE_SPLIT.md`.

// Derive output (rws-macros) names `::rust_web_server::…`; in this crate's
// tests that is this crate.
#[cfg(test)]
extern crate self as rust_web_server;

// The layers below, under the paths the moved code uses (`crate::request`, …);
// public (hidden) so doctests can name them as `rust_web_server::…`.
#[allow(unused_imports)]
#[doc(hidden)]
pub use rws_server::{core, http, http_client, request, scheduler, service_discovery};

// Also compiled under `secrets` — `secrets::aws_secrets_manager`/`azure_key_vault`
// reuse `StorageError` and the SigV4/Managed-Identity credential helpers from here.
#[cfg(any(feature = "storage-local", feature = "storage-s3", feature = "storage-azure", feature = "storage-gcs"))]
pub mod storage;

#[cfg(test)]
pub(crate) mod test_env;
