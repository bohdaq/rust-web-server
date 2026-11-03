//! Tests of the feature crates (rws-acme, rws-storage, rws-tower,
//! rws-wasm-plugin, rws-proxy, …) that run through modules of this crate or read
//! repository fixtures (spec/WORKSPACE_SPLIT.md).

#[cfg(feature = "wasm-plugins")]
mod wasm_plugin;
mod config_check;
