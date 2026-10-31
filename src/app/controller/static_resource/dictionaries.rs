//! Static files as compression dictionaries (RFC 9842, 18.12.0, #38).
//!
//! A file whose path matches one of `RWS_CONFIG_COMPRESSION_DICTIONARIES`
//! (`/js/app.*.js`, `*` = any characters) is sent with
//! `Use-As-Dictionary: match="<pattern>"`, and its bytes are kept here by
//! SHA-256, once per file version. When a browser later asks for a matching
//! path with `Available-Dictionary: :<hash>:` naming a version kept here, the
//! file goes out as a `dcz` / `dcb` delta against it (see
//! `OpenFile::delta`). Old versions stay after the file changes on disk —
//! that's the point — until the store's budget pushes them out, oldest
//! first. A restart empties it: browsers then get `br` / `zstd` until they
//! fetch the file (and so a dictionary) again.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

use crate::compression::dictionary::DictionaryHash;

/// Bytes of dictionaries kept, at most.
pub(crate) const STORE_BUDGET: usize = 32 * 1024 * 1024;

/// The first configured pattern `path` matches.
pub(crate) fn matching_pattern<'a>(patterns: &'a [String], path: &str) -> Option<&'a str> {
    patterns.iter().map(String::as_str).find(|p| glob(p, path))
}

/// `pattern` with `*` matching any run of characters (including `/`), as
/// URL patterns in `Use-As-Dictionary` do; everything else literal.
pub(crate) fn glob(pattern: &str, path: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or("");
    let Some(mut rest) = path.strip_prefix(first) else { return false };
    let parts: Vec<&str> = parts.collect();
    let Some((last, middle)) = parts.split_last() else { return rest.is_empty() };
    for part in middle {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.len() >= last.len() && rest.ends_with(last)
}

/// `pattern` as a `Use-As-Dictionary` value (an sf-string; `"` and `\`
/// escaped).
pub(crate) fn use_as_dictionary(pattern: &str) -> String {
    let mut value = String::with_capacity(pattern.len() + 9);
    value.push_str("match=\"");
    for c in pattern.chars() {
        if c == '"' || c == '\\' {
            value.push('\\');
        }
        value.push(c);
    }
    value.push('"');
    value
}

#[derive(Default)]
struct Store {
    by_hash: HashMap<DictionaryHash, Arc<[u8]>>,
    /// Oldest first.
    order: VecDeque<DictionaryHash>,
    bytes: usize,
}

fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(Mutex::default)
}

/// Keep `bytes` as the dictionary named `hash`.
pub(crate) fn remember(hash: DictionaryHash, bytes: Arc<[u8]>) {
    if bytes.len() > STORE_BUDGET {
        return;
    }
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    if store.by_hash.contains_key(&hash) {
        return;
    }
    store.bytes += bytes.len();
    store.by_hash.insert(hash, bytes);
    store.order.push_back(hash);
    while store.bytes > STORE_BUDGET {
        let Some(oldest) = store.order.pop_front() else { break };
        if let Some(gone) = store.by_hash.remove(&oldest) {
            store.bytes -= gone.len();
        }
    }
}

/// The dictionary named `hash`, if it is kept.
pub(crate) fn get(hash: &DictionaryHash) -> Option<Arc<[u8]>> {
    store().lock().unwrap_or_else(|e| e.into_inner()).by_hash.get(hash).cloned()
}

#[cfg(test)]
pub(crate) fn kept() -> usize {
    store().lock().unwrap_or_else(|e| e.into_inner()).by_hash.len()
}
