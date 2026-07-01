//! Runtime feature toggles.
//!
//! `FeatureStore` is a global, mutable map of named boolean flags. Handlers
//! check flags at request time; an MCP agent (or admin API) flips them without
//! restarting the server.
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::feature;
//!
//! // Register a flag with a default value at startup.
//! feature::global().set("dark_launch_v2", false);
//!
//! // In a handler:
//! if feature::global().is_enabled("dark_launch_v2") {
//!     // serve new code path
//! }
//! ```

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// A thread-safe map of named boolean feature flags.
pub struct FeatureStore {
    flags: Mutex<HashMap<String, bool>>,
}

impl FeatureStore {
    fn new() -> Self {
        FeatureStore { flags: Mutex::new(HashMap::new()) }
    }

    /// Set (or create) a flag. `false` disables, `true` enables.
    pub fn set(&self, name: &str, enabled: bool) {
        self.flags.lock().unwrap().insert(name.to_string(), enabled);
    }

    /// Returns `true` if the flag exists and is set to `true`.
    pub fn is_enabled(&self, name: &str) -> bool {
        *self.flags.lock().unwrap().get(name).unwrap_or(&false)
    }

    /// Snapshot of all flags sorted by name.
    pub fn list(&self) -> Vec<(String, bool)> {
        let mut pairs: Vec<(String, bool)> = self.flags.lock().unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        pairs
    }
}

static INSTANCE: OnceLock<FeatureStore> = OnceLock::new();

/// Return the process-wide `FeatureStore` singleton.
pub fn global() -> &'static FeatureStore {
    INSTANCE.get_or_init(FeatureStore::new)
}
