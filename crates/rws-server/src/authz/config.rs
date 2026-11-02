//! The config file's authorizer (`[authz]`): built per configuration
//! generation, its tuple store kept across reloads so tuples written at
//! runtime survive a SIGHUP.

use std::sync::{Arc, OnceLock, RwLock};

use super::{Authorizer, MemoryStore, TupleStore};

/// The tuple store config-file routes use (memory; SQL comes with
/// `store = "sql"`). Lives as long as the process.
#[doc(hidden)]
pub fn memory_store() -> Arc<dyn TupleStore> {
    static STORE: OnceLock<Arc<MemoryStore>> = OnceLock::new();
    STORE.get_or_init(|| Arc::new(MemoryStore::new())).clone()
}

fn slot() -> &'static RwLock<Option<Result<Authorizer, String>>> {
    static CURRENT: OnceLock<RwLock<Option<Result<Authorizer, String>>>> = OnceLock::new();
    CURRENT.get_or_init(Default::default)
}

/// Make `authorizer` the config file's (each compile).
#[doc(hidden)]
pub fn set_current(authorizer: Option<Result<Authorizer, String>>) {
    *slot().write().unwrap_or_else(|e| e.into_inner()) = authorizer;
}

/// The config file's authorizer: `None` without `[authz]`, `Err` when it
/// couldn't be built (routes needing it answer 503).
#[doc(hidden)]
pub fn current() -> Option<Result<Authorizer, String>> {
    slot().read().unwrap_or_else(|e| e.into_inner()).clone()
}
