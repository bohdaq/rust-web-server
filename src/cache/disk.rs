//! The disk store behind [`CacheLayer::disk`](super::CacheLayer::disk)
//! (18.11.0, #39): one file per entry, an in-memory index, a bound on total
//! size. Entries survive a restart.
//!
//! An entry file is a header line, the cache key, a newline, then the
//! response as it would be written to a client (`Response::generate`):
//!
//! ```text
//! RWSCACHE1 <stored-at, ms since the epoch> <ttl, s> <key length>\n
//! <key>\n
//! HTTP/1.1 200 OK\r\n…
//! ```
//!
//! It is written to a temporary file and renamed, so a reader never sees a
//! partial entry, and read back with the same parser as upstream responses.
//! File I/O happens outside the index lock. Files that can't be read are
//! removed. Two stores (e.g. from overlapping config reloads) may share a
//! directory; renames keep that safe, at worst an entry is written twice.

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::response::Response;

const MAGIC: &str = "RWSCACHE1";
const SUFFIX: &str = ".entry";
/// The longest key accepted (a request target plus varied headers).
const MAX_KEY: usize = 16 * 1024;

#[derive(Debug, Clone)]
struct Meta {
    file: PathBuf,
    size: u64,
    stored_ms: u64,
    ttl: Duration,
}

impl Meta {
    fn age(&self) -> Duration {
        Duration::from_millis(now_ms().saturating_sub(self.stored_ms))
    }

    fn fresh(&self) -> bool {
        self.age() < self.ttl
    }
}

#[derive(Default)]
struct Index {
    entries: HashMap<String, Meta>,
    /// Oldest first.
    order: VecDeque<String>,
    total: u64,
}

impl Index {
    fn remove(&mut self, key: &str) -> Option<Meta> {
        let meta = self.entries.remove(key)?;
        self.order.retain(|k| k != key);
        self.total = self.total.saturating_sub(meta.size);
        Some(meta)
    }
}

pub(crate) struct DiskStore {
    dir: PathBuf,
    max_bytes: u64,
    index: Mutex<Index>,
    /// Entry count, readable without the lock.
    count: AtomicUsize,
    seq: AtomicU64,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn file_name(key: &str) -> String {
    let hash = crate::core::sha256::sha256(key.as_bytes());
    let hex: String = hash[..16].iter().map(|b| format!("{b:02x}")).collect();
    format!("{hex}{SUFFIX}")
}

/// The header line and key of an entry file: `(stored-at ms, ttl, key,
/// offset of the response)`. `None` for anything else. Public to the crate
/// for the fuzz target.
pub(crate) fn parse_entry_header(bytes: &[u8]) -> Option<(u64, Duration, String, usize)> {
    let line_end = memchr::memchr(b'\n', &bytes[..bytes.len().min(128)])?;
    let line = std::str::from_utf8(&bytes[..line_end]).ok()?;
    let mut parts = line.split(' ');
    if parts.next()? != MAGIC {
        return None;
    }
    let stored_ms: u64 = parts.next()?.parse().ok()?;
    let ttl: u64 = parts.next()?.parse().ok()?;
    let key_len: usize = parts.next()?.parse().ok()?;
    if parts.next().is_some() || key_len > MAX_KEY {
        return None;
    }
    let key_start = line_end + 1;
    let key_end = key_start.checked_add(key_len)?;
    if bytes.get(key_end) != Some(&b'\n') {
        return None;
    }
    let key = std::str::from_utf8(&bytes[key_start..key_end]).ok()?.to_string();
    Some((stored_ms, Duration::from_secs(ttl), key, key_end + 1))
}

impl DiskStore {
    /// Open `dir` (created if missing) and index the entries in it; expired
    /// and unreadable files, and leftover temporary files, are removed.
    pub(crate) fn open(dir: impl Into<PathBuf>, max_bytes: u64) -> std::io::Result<DiskStore> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        let mut found: Vec<(String, Meta)> = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.contains(".tmp-") {
                let _ = std::fs::remove_file(&path);
                continue;
            }
            if !name.ends_with(SUFFIX) {
                continue;
            }
            match read_header(&path) {
                Some((stored_ms, ttl, key, size)) => {
                    let meta = Meta { file: path.clone(), size, stored_ms, ttl };
                    if meta.fresh() {
                        found.push((key, meta));
                    } else {
                        let _ = std::fs::remove_file(&path);
                    }
                }
                None => {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        found.sort_by_key(|(_, m)| m.stored_ms);
        let mut index = Index::default();
        for (key, meta) in found {
            index.total += meta.size;
            index.order.push_back(key.clone());
            index.entries.insert(key, meta);
        }
        let store = DiskStore { dir, max_bytes, count: AtomicUsize::new(index.entries.len()), index: Mutex::new(index), seq: AtomicU64::new(0) };
        let evicted = store.evict_over_limit();
        remove_files(evicted);
        Ok(store)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Index> {
        self.index.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.count.load(Ordering::Relaxed) == 0
    }

    pub(crate) fn len(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }

    pub(crate) fn bytes(&self) -> u64 {
        self.lock().total
    }

    /// The fresh entry for `key` and its age, or `None`. A stale or
    /// unreadable entry is removed.
    pub(crate) fn get(&self, key: &str) -> Option<(Response, Duration)> {
        let meta = {
            let mut index = self.lock();
            let meta = index.entries.get(key)?.clone();
            if !meta.fresh() {
                index.remove(key);
                self.count.store(index.entries.len(), Ordering::Relaxed);
                drop(index);
                let _ = std::fs::remove_file(&meta.file);
                return None;
            }
            meta
        };
        match read_entry(&meta.file, key) {
            Some(response) => Some((response, meta.age())),
            None => {
                let mut index = self.lock();
                if index.entries.get(key).is_some_and(|m| m.stored_ms == meta.stored_ms) {
                    index.remove(key);
                    self.count.store(index.entries.len(), Ordering::Relaxed);
                }
                drop(index);
                let _ = std::fs::remove_file(&meta.file);
                None
            }
        }
    }

    /// Store `response` under `key` for `ttl`. An entry larger than the
    /// whole store isn't kept. Errors (disk full, permissions) are logged and
    /// the response just isn't cached.
    pub(crate) fn put(&self, key: &str, response: &Response, ttl: Duration) {
        if key.len() > MAX_KEY {
            return;
        }
        let stored_ms = now_ms();
        let mut bytes = format!("{MAGIC} {stored_ms} {} {}\n{key}\n", ttl.as_secs(), key.len()).into_bytes();
        bytes.extend_from_slice(&response.clone().generate());
        let size = bytes.len() as u64;
        if size > self.max_bytes {
            return;
        }
        let file = self.dir.join(file_name(key));
        let tmp = self.dir.join(format!("{}.tmp-{}-{}", file_name(key), std::process::id(), self.seq.fetch_add(1, Ordering::Relaxed)));
        let written = std::fs::File::create(&tmp).and_then(|mut f| f.write_all(&bytes)).and_then(|_| std::fs::rename(&tmp, &file));
        if let Err(e) = written {
            let _ = std::fs::remove_file(&tmp);
            eprintln!("cache: can't write {}: {e}", file.display());
            return;
        }
        let evicted = {
            let mut index = self.lock();
            index.remove(key);
            index.total += size;
            index.order.push_back(key.to_string());
            index.entries.insert(key.to_string(), Meta { file, size, stored_ms, ttl });
            drop(index);
            self.evict_over_limit()
        };
        remove_files(evicted);
    }

    /// Drop expired entries, then the oldest, until under `max_bytes`;
    /// returns the files to delete (outside the lock).
    fn evict_over_limit(&self) -> Vec<PathBuf> {
        let mut index = self.lock();
        let mut files = Vec::new();
        let stale: Vec<String> = index.entries.iter().filter(|(_, m)| !m.fresh()).map(|(k, _)| k.clone()).collect();
        for key in stale {
            if let Some(m) = index.remove(&key) {
                files.push(m.file);
            }
        }
        while index.total > self.max_bytes {
            let Some(oldest) = index.order.front().cloned() else { break };
            if let Some(m) = index.remove(&oldest) {
                files.push(m.file);
            }
        }
        self.count.store(index.entries.len(), Ordering::Relaxed);
        files
    }

    /// Remove the entries whose key `matches`; how many.
    pub(crate) fn remove_where(&self, matches: impl Fn(&str) -> bool) -> usize {
        let files: Vec<PathBuf> = {
            let mut index = self.lock();
            let keys: Vec<String> = index.entries.keys().filter(|k| matches(k)).cloned().collect();
            let files = keys.iter().filter_map(|k| index.remove(k)).map(|m| m.file).collect();
            self.count.store(index.entries.len(), Ordering::Relaxed);
            files
        };
        let n = files.len();
        remove_files(files);
        n
    }
}

fn remove_files(files: Vec<PathBuf>) {
    for file in files {
        let _ = std::fs::remove_file(file);
    }
}

/// The header of the entry at `path` without reading the response:
/// `(stored-at, ttl, key, file size)`.
fn read_header(path: &Path) -> Option<(u64, Duration, String, u64)> {
    let file = std::fs::File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    let mut reader = std::io::BufReader::new(file);
    let mut line = Vec::new();
    reader.by_ref().take(128).read_until(b'\n', &mut line).ok()?;
    let key_len: usize = std::str::from_utf8(&line).ok()?.trim_end().rsplit(' ').next()?.parse().ok()?;
    if key_len > MAX_KEY {
        return None;
    }
    let mut rest = vec![0; key_len + 1];
    reader.read_exact(&mut rest).ok()?;
    line.extend_from_slice(&rest);
    let (stored_ms, ttl, key, _) = parse_entry_header(&line)?;
    Some((stored_ms, ttl, key, size))
}

/// The response stored at `path`, if the file is an entry for `key`.
fn read_entry(path: &Path, key: &str) -> Option<Response> {
    let bytes = std::fs::read(path).ok()?;
    let (_, _, stored_key, offset) = parse_entry_header(&bytes)?;
    if stored_key != key {
        return None;
    }
    Response::parse_upstream(&bytes[offset..], crate::proxy::REGENERATED_OR_HOP_BY_HOP).ok()
}
