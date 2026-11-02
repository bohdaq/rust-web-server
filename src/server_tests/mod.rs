//! rws-server tests that run through modules of this crate (spec/WORKSPACE_SPLIT.md).

mod cache;
mod early_hints;
mod authz;
#[cfg(feature = "model-sqlite")]
mod authz_sql;
mod application;
mod di;
mod server;
#[cfg(all(feature = "http2", feature = "storage-s3"))]
mod http1_async;
#[cfg(not(target_arch = "wasm32"))]
mod app_index;
#[cfg(not(target_arch = "wasm32"))]
mod static_resource;
#[cfg(not(target_arch = "wasm32"))]
mod app;
#[cfg(not(target_arch = "wasm32"))]
mod log;
#[cfg(not(target_arch = "wasm32"))]
mod framing;
#[cfg(not(target_arch = "wasm32"))]
mod server_fixtures;
