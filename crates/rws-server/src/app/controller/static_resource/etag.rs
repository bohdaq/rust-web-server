//! The tag in a static file's ETag: a hash of its bytes (18.27.0), so every
//! replica serving the same file gives the same ETag and a revalidation gets
//! its `304` from whichever pod answers. It was the modification time, which
//! differs per pod for the same ConfigMap or image layer.
//!
//! Worked out once per file version — `(path, len, modified)` — and kept;
//! files over [`MAX_HASHED`] keep the modification time (one pass over a
//! multi-gigabyte file per version isn't worth it).

use std::collections::HashMap;
use std::io::Read;
use std::sync::Mutex;

/// Bigger files are tagged by modification time.
const MAX_HASHED: u64 = 256 * 1024 * 1024;
/// Versions kept; the map is cleared when it fills (a cache, not a record).
const MAX_ENTRIES: usize = 4096;
/// As `open_file_cache`: a file modified less than this long ago may still
/// change within its timestamp's tick, so its tag isn't kept.
const SETTLE_NANOS: u128 = 2_000_000_000;

type Key = (String, u64, u128);
static TAGS: Mutex<Option<HashMap<Key, String>>> = Mutex::new(None);

/// The tag for `path` at `len` bytes and `modified` (nanoseconds since the
/// epoch): 16 hex digits of its FNV-1a hash, or the modification time when
/// the file is too big or can't be read.
pub(super) fn tag(path: &str, len: u64, modified: u128) -> String {
    if len > MAX_HASHED {
        return modified.to_string();
    }
    let key = (path.to_string(), len, modified);
    if let Some(tag) = TAGS.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&key)) {
        return tag.clone();
    }
    let Some(hash) = hash_file(path) else { return modified.to_string() };
    let tag = format!("{hash:016x}");
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    if now.saturating_sub(modified) >= SETTLE_NANOS {
        let mut tags = TAGS.lock().unwrap_or_else(|e| e.into_inner());
        let map = tags.get_or_insert_with(HashMap::new);
        if map.len() >= MAX_ENTRIES {
            map.clear();
        }
        map.insert(key, tag.clone());
    }
    tag
}

fn hash_file(path: &str) -> Option<u64> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            return Some(hash);
        }
        for &b in &buf[..n] {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn same_bytes_same_tag_whatever_the_mtime() {
        let dir = std::env::temp_dir().join(format!("rws-etag-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b, c) = (dir.join("a"), dir.join("b"), dir.join("c"));
        std::fs::write(&a, b"hello replicas").unwrap();
        std::fs::write(&b, b"hello replicas").unwrap();
        std::fs::write(&c, b"hello replicaz").unwrap();
        let tag = |p: &std::path::Path, m| super::tag(p.to_str().unwrap(), 14, m);
        assert_eq!(tag(&a, 1), tag(&b, 2));
        assert_ne!(tag(&a, 1), tag(&c, 1));
        assert_eq!(tag(&a, 1).len(), 16);
        // Unreadable: the modification time.
        assert_eq!(tag(&dir.join("missing"), 7), "7");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
