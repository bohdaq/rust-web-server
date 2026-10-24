//! Open-file cache for the static-file fast path, like nginx's
//! `open_file_cache`: a regular file's open handle is kept per path for
//! [`valid_for`] (default 1 s, `RWS_CONFIG_OPEN_FILE_CACHE_SECS`, `0` turns it
//! off) and read with `pread`, so a hit costs one `fstat` — no `open`,
//! `close`, `stat` or `lstat`. Opening the file was the largest remaining
//! cost of serving a small file.
//!
//! Size and modification time come from that `fstat` on every request, so an
//! edit in place is served correctly at once. After `valid_for`, one `stat`
//! of the path checks it still names the same file (device and inode): a file
//! replaced by a new one (a rename, as editors and deploy tools do) or deleted
//! is noticed within `valid_for`. At most [`CAPACITY`] files are held open.
//! Unix only; elsewhere every request opens the file.

use std::fs::File;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, Instant};

/// Most files held open at once — well under the common 256-descriptor
/// soft limit on macOS.
pub(crate) const CAPACITY: usize = 128;
const SHARDS: usize = 8;

/// Files up to this size also have their contents cached.
pub(crate) const MAX_CACHED_FILE: u64 = 64 * 1024;
/// Total cached contents across all entries.
pub(crate) const CONTENTS_BUDGET: usize = 8 * 1024 * 1024;
/// Bytes of contents currently cached.
static CONTENTS_BYTES: AtomicUsize = AtomicUsize::new(0);

/// An open regular file.
pub(crate) struct OpenFile {
    file: File,
    #[cfg(unix)]
    identity: (u64, u64), // (device, inode)
    /// For cache entries: the file's bytes as of `(len, modified)`, so a
    /// request for an unchanged small file costs no read at all.
    cache_contents: bool,
    contents: RwLock<Option<CachedContents>>,
}

struct CachedContents {
    len: u64,
    modified: u128,
    bytes: Arc<[u8]>,
}

impl Drop for OpenFile {
    fn drop(&mut self) {
        let held = self.contents.get_mut().map(|c| c.as_ref().map_or(0, |c| c.bytes.len())).unwrap_or(0);
        CONTENTS_BYTES.fetch_sub(held, Ordering::Relaxed);
    }
}

/// Size and modification time of an [`OpenFile`] right now.
pub(crate) struct FileState {
    pub(crate) len: u64,
    /// Modification time, nanoseconds since the Unix epoch.
    pub(crate) modified: u128,
}

impl OpenFile {
    fn from_file(file: File) -> Option<OpenFile> {
        let md = file.metadata().ok()?;
        if !md.is_file() {
            return None;
        }
        Some(OpenFile {
            #[cfg(unix)]
            identity: identity(&md),
            file,
            cache_contents: false,
            contents: RwLock::new(None),
        })
    }

    /// The whole file as of `state` (from [`OpenFile::state`]): the cached
    /// bytes when the size and modification time still match, otherwise a
    /// fresh read — which is cached for next time if the file is small and
    /// the budget allows. An edit is therefore served at once.
    pub(crate) fn contents(&self, state: &FileState) -> std::io::Result<Vec<u8>> {
        if let Some(cached) = self.contents.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
            if cached.len == state.len && cached.modified == state.modified {
                return Ok(cached.bytes.to_vec());
            }
        }
        let body = self.read_all(state.len)?;
        if self.cache_contents && state.len <= MAX_CACHED_FILE && settled(state.modified) {
            let mut slot = self.contents.write().unwrap_or_else(|e| e.into_inner());
            let released = slot.as_ref().map_or(0, |c| c.bytes.len());
            let wanted = body.len();
            // Reserve within the budget, counting what this entry gives back.
            let reserved = CONTENTS_BYTES
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |held| {
                    let after = held - released + wanted;
                    (after <= CONTENTS_BUDGET).then_some(after)
                })
                .is_ok();
            if reserved {
                *slot = Some(CachedContents { len: state.len, modified: state.modified, bytes: Arc::from(body.as_slice()) });
            }
        }
        Ok(body)
    }

    /// The open file (for `sendfile`).
    #[cfg_attr(not(feature = "http2"), allow(dead_code))]
    pub(crate) fn file(&self) -> &File {
        &self.file
    }

    /// Current size and modification time (`fstat`).
    pub(crate) fn state(&self) -> Option<FileState> {
        let md = self.file.metadata().ok()?;
        Some(FileState { len: md.len(), modified: modified_nanos(&md) })
    }

    /// `len` bytes from the start of the file, read with positional reads —
    /// safe while other requests read the same handle.
    pub(crate) fn read_all(&self, len: u64) -> std::io::Result<Vec<u8>> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileExt;
            let mut body = vec![0u8; len as usize];
            self.file.read_exact_at(&mut body, 0)?;
            Ok(body)
        }
        #[cfg(not(unix))]
        {
            use std::io::Read;
            let mut body = Vec::with_capacity(len as usize);
            (&self.file).take(len).read_to_end(&mut body)?;
            Ok(body)
        }
    }
}

/// Contents are cached only for a file last modified at least this long ago:
/// on a filesystem with coarse timestamps, a same-size edit within the same
/// tick wouldn't change `(len, modified)`, so a freshly written file is read
/// on every request until it settles (git's "racy" index entries, same fix).
const SETTLE: std::time::Duration = std::time::Duration::from_secs(2);

fn settled(modified_nanos: u128) -> bool {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    now.saturating_sub(modified_nanos) >= SETTLE.as_nanos()
}

fn modified_nanos(md: &std::fs::Metadata) -> u128 {
    md.modified()
        .ok()
        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos())
}

#[cfg(unix)]
fn identity(md: &std::fs::Metadata) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (md.dev(), md.ino())
}

struct Entry {
    file: Arc<OpenFile>,
    checked: Instant,
}

type Shard = RwLock<Vec<(String, Entry)>>;

fn shards() -> &'static [Shard; SHARDS] {
    static SHARDS_CELL: OnceLock<[Shard; SHARDS]> = OnceLock::new();
    SHARDS_CELL.get_or_init(|| std::array::from_fn(|_| RwLock::new(Vec::new())))
}

fn shard_for(path: &str) -> &'static Shard {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    &shards()[(hasher.finish() as usize) % SHARDS]
}

/// How long an entry is trusted before it's revalidated; zero = cache off.
/// Read once from `RWS_CONFIG_OPEN_FILE_CACHE_SECS` (default 1).
pub(crate) fn valid_for() -> Duration {
    static VALID: OnceLock<Duration> = OnceLock::new();
    *VALID.get_or_init(|| {
        if !cfg!(unix) {
            return Duration::ZERO;
        }
        let secs = std::env::var("RWS_CONFIG_OPEN_FILE_CACHE_SECS")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or(1);
        Duration::from_secs(secs)
    })
}

/// The regular file at `path`, from the cache when possible. With
/// `follow_symlinks = false` a symlink isn't opened (`O_NOFOLLOW`) and `None`
/// is returned, as for a missing file or a directory.
pub(crate) fn open(path: &str, follow_symlinks: bool) -> Option<Arc<OpenFile>> {
    open_with(path, follow_symlinks, valid_for())
}

fn open_with(path: &str, follow_symlinks: bool, valid_for: Duration) -> Option<Arc<OpenFile>> {
    if valid_for.is_zero() {
        return open_uncached(path, follow_symlinks).map(Arc::new);
    }
    let shard = shard_for(path);
    let now = Instant::now();
    let stale = {
        let entries = shard.read().unwrap_or_else(|e| e.into_inner());
        match entries.iter().find(|(p, _)| p == path) {
            Some((_, entry)) if now.duration_since(entry.checked) < valid_for => return Some(Arc::clone(&entry.file)),
            Some((_, entry)) => Some(Arc::clone(&entry.file)),
            None => None,
        }
    };

    // Revalidate a stale entry with one stat before reopening.
    if let Some(cached) = stale {
        if still_current(path, &cached, follow_symlinks) {
            let mut entries = shard.write().unwrap_or_else(|e| e.into_inner());
            if let Some((_, entry)) = entries.iter_mut().find(|(p, _)| p == path) {
                entry.checked = now;
            }
            return Some(cached);
        }
        shard.write().unwrap_or_else(|e| e.into_inner()).retain(|(p, _)| p != path);
    }

    let mut opened = open_uncached(path, follow_symlinks)?;
    opened.cache_contents = true;
    let fresh = Arc::new(opened);
    let mut entries = shard.write().unwrap_or_else(|e| e.into_inner());
    entries.retain(|(p, _)| p != path);
    if entries.len() >= CAPACITY / SHARDS {
        // Evict the entry checked longest ago.
        if let Some(oldest) = entries.iter().enumerate().min_by_key(|(_, (_, e))| e.checked).map(|(i, _)| i) {
            entries.swap_remove(oldest);
        }
    }
    entries.push((path.to_string(), Entry { file: Arc::clone(&fresh), checked: now }));
    Some(fresh)
}

#[cfg(unix)]
fn still_current(path: &str, cached: &OpenFile, follow_symlinks: bool) -> bool {
    let md = if follow_symlinks { std::fs::metadata(path) } else { std::fs::symlink_metadata(path) };
    match md {
        Ok(md) => md.is_file() && identity(&md) == cached.identity,
        Err(_) => false,
    }
}

#[cfg(not(unix))]
fn still_current(_path: &str, _cached: &OpenFile, _follow_symlinks: bool) -> bool {
    false
}

fn open_uncached(path: &str, follow_symlinks: bool) -> Option<OpenFile> {
    let file = if follow_symlinks { File::open(path).ok()? } else { open_no_follow(path)? };
    OpenFile::from_file(file)
}

/// Open `path` unless it is a symlink — one `open` with `O_NOFOLLOW` instead
/// of an extra `lstat`.
#[cfg(all(unix, any(feature = "http1", feature = "http2")))]
fn open_no_follow(path: &str) -> Option<File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW).open(path).ok()
}

#[cfg(not(all(unix, any(feature = "http1", feature = "http2"))))]
fn open_no_follow(path: &str) -> Option<File> {
    let md = std::fs::symlink_metadata(path).ok()?;
    if md.file_type().is_symlink() {
        return None;
    }
    File::open(path).ok()
}

/// `true` if `path` has a cache entry that is still trusted — lets
/// `is_matching` skip its `stat` on a hit.
pub(crate) fn is_cached_file(path: &str) -> bool {
    let valid_for = valid_for();
    if valid_for.is_zero() {
        return false;
    }
    let entries = shard_for(path).read().unwrap_or_else(|e| e.into_inner());
    entries
        .iter()
        .any(|(p, e)| p == path && e.checked.elapsed() < valid_for)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env::TempPath;

    const LONG: Duration = Duration::from_secs(3600);

    #[test]
    fn a_hit_reuses_the_open_handle() {
        let path = TempPath::new("ofc_hit.txt");
        std::fs::write(path.as_str(), b"hello").unwrap();
        let first = open_with(path.as_str(), true, LONG).unwrap();
        let second = open_with(path.as_str(), true, LONG).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        let state = second.state().unwrap();
        assert_eq!(state.len, 5);
        assert_eq!(second.read_all(state.len).unwrap(), b"hello");
    }

    #[test]
    fn an_edit_in_place_is_read_immediately_even_if_the_size_changes() {
        let path = TempPath::new("ofc_edit.txt");
        std::fs::write(path.as_str(), b"one").unwrap();
        let cached = open_with(path.as_str(), true, LONG).unwrap();
        std::fs::write(path.as_str(), b"three!").unwrap();
        let state = cached.state().unwrap();
        assert_eq!(state.len, 6, "size comes from fstat, not the cache");
        assert_eq!(cached.read_all(state.len).unwrap(), b"three!");
    }

    #[test]
    fn a_replaced_file_is_reopened_after_revalidation() {
        let path = TempPath::new("ofc_replaced.txt");
        let tmp = TempPath::new("ofc_replaced.tmp");
        std::fs::write(path.as_str(), b"old").unwrap();
        let old = open_with(path.as_str(), true, Duration::from_millis(1)).unwrap();
        // Replace by rename: a new inode at the same path.
        std::fs::write(tmp.as_str(), b"new!").unwrap();
        std::fs::rename(tmp.as_str(), path.as_str()).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        let new = open_with(path.as_str(), true, Duration::from_millis(1)).unwrap();
        assert!(!Arc::ptr_eq(&old, &new));
        assert_eq!(new.read_all(new.state().unwrap().len).unwrap(), b"new!");
    }

    #[test]
    fn a_deleted_file_is_dropped_after_revalidation() {
        let path = TempPath::new("ofc_deleted.txt");
        std::fs::write(path.as_str(), b"x").unwrap();
        assert!(open_with(path.as_str(), true, Duration::from_millis(1)).is_some());
        std::fs::remove_file(path.as_str()).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        assert!(open_with(path.as_str(), true, Duration::from_millis(1)).is_none());
    }

    #[test]
    fn directories_and_missing_files_are_not_opened() {
        let dir = TempPath::new("ofc_dir");
        std::fs::create_dir_all(&*dir).unwrap();
        assert!(open_with(dir.as_str(), true, LONG).is_none());
        std::fs::remove_dir(&*dir).unwrap();
        assert!(open_with("/nonexistent/ofc", true, LONG).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_refused_without_follow() {
        let target = TempPath::new("ofc_target.txt");
        let link = TempPath::new("ofc_link");
        std::fs::write(target.as_str(), b"t").unwrap();
        std::os::unix::fs::symlink(target.as_str(), link.as_str()).unwrap();
        assert!(open_with(link.as_str(), false, LONG).is_none());
        assert!(open_with(link.as_str(), true, LONG).is_some());
    }

    #[test]
    fn zero_validity_disables_caching() {
        let path = TempPath::new("ofc_off.txt");
        std::fs::write(path.as_str(), b"x").unwrap();
        let a = open_with(path.as_str(), true, Duration::ZERO).unwrap();
        let b = open_with(path.as_str(), true, Duration::ZERO).unwrap();
        assert!(!Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn small_file_contents_are_cached_until_the_file_changes() {
        let path = TempPath::new("ofc_contents.txt");
        std::fs::write(path.as_str(), b"v1").unwrap();
        // Backdate it: contents are cached only for settled files.
        let old = std::time::SystemTime::now() - Duration::from_secs(60);
        std::fs::File::options().write(true).open(path.as_str()).unwrap().set_modified(old).unwrap();
        let file = open_with(path.as_str(), true, LONG).unwrap();
        let state = file.state().unwrap();
        assert_eq!(file.contents(&state).unwrap(), b"v1");
        assert!(file.contents.read().unwrap().is_some(), "cached after the first read");
        assert_eq!(file.contents(&state).unwrap(), b"v1");

        // Edit in place: size and mtime change, so the next request rereads.
        std::fs::write(path.as_str(), b"version 2").unwrap();
        let state = file.state().unwrap();
        assert_eq!(file.contents(&state).unwrap(), b"version 2");
    }

    #[test]
    fn freshly_written_files_are_not_cached_yet() {
        let path = TempPath::new("ofc_fresh.txt");
        std::fs::write(path.as_str(), b"new").unwrap();
        let file = open_with(path.as_str(), true, LONG).unwrap();
        let state = file.state().unwrap();
        assert_eq!(file.contents(&state).unwrap(), b"new");
        assert!(file.contents.read().unwrap().is_none(), "modified < 2 s ago: read every time");
    }

    #[test]
    fn large_files_and_uncached_handles_keep_no_contents() {
        let path = TempPath::new("ofc_large.bin");
        std::fs::write(path.as_str(), vec![7u8; MAX_CACHED_FILE as usize + 1]).unwrap();
        let file = open_with(path.as_str(), true, LONG).unwrap();
        let state = file.state().unwrap();
        assert_eq!(file.contents(&state).unwrap().len(), MAX_CACHED_FILE as usize + 1);
        assert!(file.contents.read().unwrap().is_none());

        let small = OpenFile::from_file(File::open(path.as_str()).unwrap()).unwrap();
        assert!(!small.cache_contents, "only cache entries keep contents");
    }

    #[test]
    fn capacity_is_bounded() {
        let paths: Vec<TempPath> = (0..CAPACITY * 2).map(|i| TempPath::new(&format!("ofc_cap_{i}.txt"))).collect();
        for p in &paths {
            std::fs::write(p.as_str(), b"x").unwrap();
            open_with(p.as_str(), true, LONG).unwrap();
        }
        let total: usize = shards().iter().map(|s| s.read().unwrap().len()).sum();
        assert!(total <= CAPACITY, "{total} entries");
    }
}
