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

use crate::compression::{Encoding, Level};

/// Most files held open at once — well under the common 256-descriptor
/// soft limit on macOS.
pub(crate) const CAPACITY: usize = 128;
const SHARDS: usize = 8;

/// Files up to this size also have their contents cached.
pub(crate) const MAX_CACHED_FILE: u64 = 64 * 1024;
/// Total cached contents across all entries.
pub(crate) const CONTENTS_BUDGET: usize = 8 * 1024 * 1024;
/// Bytes of contents currently cached (plain and compressed, together).
static CONTENTS_BYTES: AtomicUsize = AtomicUsize::new(0);
/// Files up to this size also have their compressed forms cached (and a
/// client that accepts gzip, br or zstd gets one even above the streaming
/// threshold); anything bigger goes out with `sendfile`, uncompressed.
pub(crate) const MAX_GZIPPED_FILE: u64 = 1024 * 1024;

/// An open regular file.
pub(crate) struct OpenFile {
    file: File,
    #[cfg(unix)]
    identity: (u64, u64), // (device, inode)
    /// For cache entries: the file's bytes as of `(len, modified)`, so a
    /// request for an unchanged small file costs no read at all.
    cache_contents: bool,
    contents: RwLock<Option<CachedContents>>,
    /// For cache entries with `RWS_CONFIG_OPEN_FILE_CACHE_STRICT=false`: the
    /// last `fstat` result and when it was taken, reused within [`valid_for`].
    last_state: RwLock<Option<(Instant, u64, u128)>>,
    /// For cache entries: the compressed bytes as of `(len, modified)`, one
    /// slot per [`Encoding`] — compressing a text file on every request cost
    /// ~80× serving it.
    encoded: [RwLock<Option<CachedEncoded>>; 3],
}

struct CachedContents {
    len: u64,
    modified: u128,
    bytes: Arc<[u8]>,
}

struct CachedEncoded {
    len: u64,
    modified: u128,
    /// `None`: the encoding doesn't make this file smaller, so it's sent as is.
    bytes: Option<Arc<[u8]>>,
}

/// A file in one content coding, from [`OpenFile::encoded`].
pub(crate) enum Encoded {
    /// Compressed bytes, smaller than the file.
    Smaller(Arc<[u8]>),
    /// The encoding doesn't make it smaller: send the file as is.
    NotSmaller,
}

fn slot(encoding: Encoding) -> usize {
    match encoding {
        Encoding::Gzip => 0,
        Encoding::Brotli => 1,
        Encoding::Zstd => 2,
    }
}

/// The file name suffix of a precompressed sibling (`app.js.br`).
fn sibling_suffix(encoding: Encoding) -> &'static str {
    match encoding {
        Encoding::Gzip => ".gz",
        Encoding::Brotli => ".br",
        Encoding::Zstd => ".zst",
    }
}

/// The bytes of `path` + `.gz`/`.br`/`.zst` when that is a regular file (not
/// a symlink, which could point outside the web root) at least as new as the
/// file itself (`modified`, nanoseconds), as nginx's `gzip_static` serves.
fn precompressed_sibling(path: &str, encoding: Encoding, modified: u128) -> Option<Arc<[u8]>> {
    let mut sibling = String::with_capacity(path.len() + 4);
    sibling.push_str(path);
    sibling.push_str(sibling_suffix(encoding));
    let md = std::fs::symlink_metadata(&sibling).ok()?;
    if !md.file_type().is_file() {
        return None;
    }
    let sibling_modified = md.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
    if sibling_modified < modified {
        return None;
    }
    std::fs::read(&sibling).ok().map(Arc::from)
}

impl Drop for OpenFile {
    fn drop(&mut self) {
        let held = self.contents.get_mut().map(|c| c.as_ref().map_or(0, |c| c.bytes.len())).unwrap_or(0)
            + self
                .encoded
                .iter_mut()
                .map(|slot| slot.get_mut().map(|g| g.as_ref().and_then(|g| g.bytes.as_ref()).map_or(0, |b| b.len())).unwrap_or(0))
                .sum::<usize>();
        CONTENTS_BYTES.fetch_sub(held, Ordering::Relaxed);
    }
}

/// Take `wanted` bytes of the contents budget, giving back `released`.
fn reserve(released: usize, wanted: usize) -> bool {
    CONTENTS_BYTES
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |held| {
            let after = held - released + wanted;
            (after <= CONTENTS_BUDGET).then_some(after)
        })
        .is_ok()
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
            last_state: RwLock::new(None),
            encoded: [RwLock::new(None), RwLock::new(None), RwLock::new(None)],
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
            // Reserve within the budget, counting what this entry gives back.
            if reserve(released, body.len()) {
                *slot = Some(CachedContents { len: state.len, modified: state.modified, bytes: Arc::from(body.as_slice()) });
            }
        }
        Ok(body)
    }

    /// The file in `encoding` as of `state`: a precompressed sibling
    /// (`<path>.br`, `.zst`, `.gz`) at least as new as the file when there is
    /// one, else the file compressed at [`Level::Static`]. Cached per
    /// `(len, modified)` like [`Self::contents`] (files up to
    /// [`MAX_GZIPPED_FILE`], settled, within the budget), so an unchanged file
    /// costs no compression and no sibling lookup after the first request.
    pub(crate) fn encoded(&self, state: &FileState, encoding: Encoding, path: &str) -> std::io::Result<Encoded> {
        let slot = &self.encoded[slot(encoding)];
        if let Some(cached) = slot.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
            if cached.len == state.len && cached.modified == state.modified {
                return Ok(cached.bytes.as_ref().map_or(Encoded::NotSmaller, |b| Encoded::Smaller(Arc::clone(b))));
            }
        }
        let compressed = match precompressed_sibling(path, encoding, state.modified) {
            Some(bytes) => Some(bytes),
            None => {
                let plain = self.contents(state)?;
                crate::compression::encode(encoding, &plain, Level::Static)
                    .filter(|encoded| encoded.len() < plain.len())
                    .map(|encoded| Arc::<[u8]>::from(encoded.as_slice()))
            }
        };
        if self.cache_contents && state.len <= MAX_GZIPPED_FILE && settled(state.modified) {
            let mut slot = slot.write().unwrap_or_else(|e| e.into_inner());
            let released = slot.as_ref().and_then(|g| g.bytes.as_ref()).map_or(0, |b| b.len());
            let wanted = compressed.as_ref().map_or(0, |b| b.len());
            if reserve(released, wanted) {
                *slot = Some(CachedEncoded { len: state.len, modified: state.modified, bytes: compressed.clone() });
            }
        }
        Ok(compressed.map_or(Encoded::NotSmaller, Encoded::Smaller))
    }

    /// The open file (for `sendfile`).
    #[cfg_attr(not(feature = "http2"), allow(dead_code))]
    pub(crate) fn file(&self) -> &File {
        &self.file
    }

    /// Current size and modification time: one `fstat` — or, with
    /// `RWS_CONFIG_OPEN_FILE_CACHE_STRICT=false`, the last one if it is
    /// younger than [`valid_for`] (nginx's `open_file_cache_valid`: an edit in
    /// place may then be served stale for up to that long).
    pub(crate) fn state(&self) -> Option<FileState> {
        self.state_with(strict(), valid_for())
    }

    fn state_with(&self, strict: bool, valid_for: Duration) -> Option<FileState> {
        if strict || !self.cache_contents {
            return self.fstat();
        }
        if let Some((at, len, modified)) = *self.last_state.read().unwrap_or_else(|e| e.into_inner()) {
            if at.elapsed() < valid_for {
                return Some(FileState { len, modified });
            }
        }
        let state = self.fstat()?;
        *self.last_state.write().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), state.len, state.modified));
        Some(state)
    }

    fn fstat(&self) -> Option<FileState> {
        #[cfg(all(target_os = "linux", any(feature = "http1", feature = "http2")))]
        {
            fstat_state(&self.file)
        }
        #[cfg(not(all(target_os = "linux", any(feature = "http1", feature = "http2"))))]
        {
            let md = self.file.metadata().ok()?;
            Some(FileState { len: md.len(), modified: modified_nanos(&md) })
        }
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

/// [`OpenFile::state`] with one plain `fstat`: `File::metadata` uses
/// `statx` on Linux (for the birth time, which this doesn't need) — a
/// heavier syscall, made on every static request. Same values as
/// `md.len()` and [`modified_nanos`] (a time before the epoch is 0).
#[cfg(all(target_os = "linux", any(feature = "http1", feature = "http2")))]
fn fstat_state(file: &File) -> Option<FileState> {
    use std::os::unix::io::AsRawFd;
    let mut st = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: fstat writes a full `stat` into `st` on success; the fd is
    // owned by `file`, which outlives the call.
    if unsafe { libc::fstat(file.as_raw_fd(), st.as_mut_ptr()) } != 0 {
        return None;
    }
    // SAFETY: fstat returned 0, so `st` is initialized.
    let st = unsafe { st.assume_init() };
    let modified = if st.st_mtime < 0 {
        0
    } else {
        st.st_mtime as u128 * 1_000_000_000 + st.st_mtime_nsec as u128
    };
    Some(FileState { len: st.st_size as u64, modified })
}

#[cfg_attr(all(target_os = "linux", any(feature = "http1", feature = "http2")), allow(dead_code))]
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

/// `false` reuses a cached file's last size and modification time for
/// [`valid_for`] instead of an `fstat` per request. Read once from
/// `RWS_CONFIG_OPEN_FILE_CACHE_STRICT` (default `true`).
pub(crate) fn strict() -> bool {
    static STRICT: OnceLock<bool> = OnceLock::new();
    *STRICT.get_or_init(|| {
        !std::env::var("RWS_CONFIG_OPEN_FILE_CACHE_STRICT").is_ok_and(|v| v.trim().eq_ignore_ascii_case("false"))
    })
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
    // Nothing outside the web root, even through a symlink (#43); checked
    // here, once per open, rather than per request.
    if !super::inside_root(path) {
        return None;
    }
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
        let path = TempPath::new_in_cwd("ofc_hit.txt");
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
        let path = TempPath::new_in_cwd("ofc_edit.txt");
        std::fs::write(path.as_str(), b"one").unwrap();
        let cached = open_with(path.as_str(), true, LONG).unwrap();
        std::fs::write(path.as_str(), b"three!").unwrap();
        let state = cached.state().unwrap();
        assert_eq!(state.len, 6, "size comes from fstat, not the cache");
        assert_eq!(cached.read_all(state.len).unwrap(), b"three!");
    }

    #[test]
    fn a_replaced_file_is_reopened_after_revalidation() {
        let path = TempPath::new_in_cwd("ofc_replaced.txt");
        let tmp = TempPath::new_in_cwd("ofc_replaced.tmp");
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
        let path = TempPath::new_in_cwd("ofc_deleted.txt");
        std::fs::write(path.as_str(), b"x").unwrap();
        assert!(open_with(path.as_str(), true, Duration::from_millis(1)).is_some());
        std::fs::remove_file(path.as_str()).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        assert!(open_with(path.as_str(), true, Duration::from_millis(1)).is_none());
    }

    #[test]
    fn directories_and_missing_files_are_not_opened() {
        let dir = TempPath::new_in_cwd("ofc_dir");
        std::fs::create_dir_all(&*dir).unwrap();
        assert!(open_with(dir.as_str(), true, LONG).is_none());
        std::fs::remove_dir(&*dir).unwrap();
        assert!(open_with("/nonexistent/ofc", true, LONG).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_refused_without_follow() {
        let target = TempPath::new_in_cwd("ofc_target.txt");
        let link = TempPath::new_in_cwd("ofc_link");
        std::fs::write(target.as_str(), b"t").unwrap();
        std::os::unix::fs::symlink(target.as_str(), link.as_str()).unwrap();
        assert!(open_with(link.as_str(), false, LONG).is_none());
        assert!(open_with(link.as_str(), true, LONG).is_some());
    }

    #[test]
    fn zero_validity_disables_caching() {
        let path = TempPath::new_in_cwd("ofc_off.txt");
        std::fs::write(path.as_str(), b"x").unwrap();
        let a = open_with(path.as_str(), true, Duration::ZERO).unwrap();
        let b = open_with(path.as_str(), true, Duration::ZERO).unwrap();
        assert!(!Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn small_file_contents_are_cached_until_the_file_changes() {
        let path = TempPath::new_in_cwd("ofc_contents.txt");
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
        let path = TempPath::new_in_cwd("ofc_fresh.txt");
        std::fs::write(path.as_str(), b"new").unwrap();
        let file = open_with(path.as_str(), true, LONG).unwrap();
        let state = file.state().unwrap();
        assert_eq!(file.contents(&state).unwrap(), b"new");
        assert!(file.contents.read().unwrap().is_none(), "modified < 2 s ago: read every time");
    }

    #[test]
    fn large_files_and_uncached_handles_keep_no_contents() {
        let path = TempPath::new_in_cwd("ofc_large.bin");
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
        let paths: Vec<TempPath> = (0..CAPACITY * 2).map(|i| TempPath::new_in_cwd(&format!("ofc_cap_{i}.txt"))).collect();
        for p in &paths {
            std::fs::write(p.as_str(), b"x").unwrap();
            open_with(p.as_str(), true, LONG).unwrap();
        }
        let total: usize = shards().iter().map(|s| s.read().unwrap().len()).sum();
        assert!(total <= CAPACITY, "{total} entries");
    }

    #[test]
    fn state_matches_std_metadata_including_nanoseconds_and_pre_epoch_times() {
        let path = TempPath::new_in_cwd("ofc_state.txt");
        std::fs::write(path.as_str(), b"12345").unwrap();
        let file = File::open(path.as_str()).unwrap();
        for mtime in [
            std::time::UNIX_EPOCH + Duration::new(1_767_222_000, 123_456_789),
            std::time::UNIX_EPOCH + Duration::from_secs(1),
            std::time::UNIX_EPOCH - Duration::from_secs(5),
        ] {
            file.set_modified(mtime).unwrap();
            let opened = OpenFile::from_file(File::open(path.as_str()).unwrap()).unwrap();
            let state = opened.state().unwrap();
            let md = std::fs::metadata(path.as_str()).unwrap();
            assert_eq!(md.len(), state.len);
            assert_eq!(modified_nanos(&md), state.modified, "{mtime:?}");
        }
    }

    fn old_text_file(name: &str, text: &str) -> TempPath {
        let path = TempPath::new_in_cwd(name);
        std::fs::write(path.as_str(), text).unwrap();
        File::options().write(true).open(path.as_str()).unwrap()
            .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(1_767_222_000)).unwrap();
        path
    }

    #[test]
    fn gzipped_is_compressed_once_per_file_version() {
        let path = old_text_file("ofc_gzip.js", &"var a = 1; ".repeat(200));
        let file = open_with(path.as_str(), true, LONG).unwrap();
        let state = file.state().unwrap();
        let Encoded::Smaller(first) = file.encoded(&state, Encoding::Gzip, path.as_str()).unwrap() else { panic!("text compresses") };
        assert_eq!(crate::compression::gzip(&file.contents(&state).unwrap()).unwrap(), first.to_vec());
        let Encoded::Smaller(second) = file.encoded(&state, Encoding::Gzip, path.as_str()).unwrap() else { panic!() };
        assert!(Arc::ptr_eq(&first, &second), "a hit reuses the cached bytes");

        // An edit in place (new size and time) is compressed afresh at once.
        std::fs::write(path.as_str(), "let b = 2; ".repeat(300)).unwrap();
        File::options().write(true).open(path.as_str()).unwrap()
            .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(1_767_223_000)).unwrap();
        let state = file.state().unwrap();
        let Encoded::Smaller(third) = file.encoded(&state, Encoding::Gzip, path.as_str()).unwrap() else { panic!() };
        assert!(!Arc::ptr_eq(&first, &third));
        assert_eq!(crate::compression::gzip("let b = 2; ".repeat(300).as_bytes()).unwrap(), third.to_vec());
    }

    #[test]
    fn gzipped_reports_a_file_gzip_does_not_shrink() {
        let path = TempPath::new_in_cwd("ofc_gzip_random.txt");
        let noise: Vec<u8> = (0..4096u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
        std::fs::write(path.as_str(), &noise).unwrap();
        File::options().write(true).open(path.as_str()).unwrap()
            .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(1_767_222_000)).unwrap();
        let file = open_with(path.as_str(), true, LONG).unwrap();
        let state = file.state().unwrap();
        let shrinks = crate::compression::gzip(&noise).unwrap().len() < noise.len();
        match file.encoded(&state, Encoding::Gzip, path.as_str()).unwrap() {
            Encoded::Smaller(_) => assert!(shrinks),
            Encoded::NotSmaller => assert!(!shrinks),
        }
    }

    #[test]
    fn each_encoding_is_cached_in_its_own_slot() {
        let path = old_text_file("ofc_encodings.css", &".a { color: red; }\n".repeat(300));
        let file = open_with(path.as_str(), true, LONG).unwrap();
        let state = file.state().unwrap();
        let plain = file.contents(&state).unwrap();
        for encoding in [Encoding::Gzip, Encoding::Brotli, Encoding::Zstd].into_iter().filter(|e| e.available()) {
            let Encoded::Smaller(first) = file.encoded(&state, encoding, path.as_str()).unwrap() else { panic!("{encoding:?} compresses") };
            assert_eq!(crate::compression::encode(encoding, &plain, Level::Static).unwrap(), first.to_vec(), "{encoding:?}");
            let Encoded::Smaller(again) = file.encoded(&state, encoding, path.as_str()).unwrap() else { panic!() };
            assert!(Arc::ptr_eq(&first, &again), "{encoding:?}: a hit reuses the cached bytes");
        }
    }

    /// A sibling file next to `file_name`, with its modification time.
    fn sibling(file_name: &str, suffix: &str, bytes: &[u8], mtime_secs: u64) -> TempPath {
        let path = TempPath::new_in_cwd(&format!("{file_name}{suffix}"));
        std::fs::write(path.as_str(), bytes).unwrap();
        File::options().write(true).open(path.as_str()).unwrap()
            .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(mtime_secs)).unwrap();
        path
    }

    #[test]
    fn a_precompressed_sibling_at_least_as_new_is_served() {
        let path = old_text_file("ofc_sibling_new.js", &"var s = 1; ".repeat(200));
        let _sibling = sibling("ofc_sibling_new.js", ".gz", b"precompressed bytes", 1_767_222_000);
        let file = open_with(path.as_str(), true, LONG).unwrap();
        let state = file.state().unwrap();
        let Encoded::Smaller(bytes) = file.encoded(&state, Encoding::Gzip, path.as_str()).unwrap() else { panic!() };
        assert_eq!(b"precompressed bytes", &bytes[..]);
    }

    #[test]
    fn an_older_or_symlinked_sibling_is_ignored() {
        let path = old_text_file("ofc_sibling_old.js", &"var s = 1; ".repeat(200));
        let _old = sibling("ofc_sibling_old.js", ".gz", b"precompressed bytes", 1_767_221_000);
        let file = open_with(path.as_str(), true, LONG).unwrap();
        let state = file.state().unwrap();
        let Encoded::Smaller(compressed) = file.encoded(&state, Encoding::Gzip, path.as_str()).unwrap() else { panic!() };
        assert_eq!(crate::compression::gzip(&file.contents(&state).unwrap()).unwrap(), compressed.to_vec());

        // A symlink sibling is never followed (it could point outside the root).
        #[cfg(unix)]
        if Encoding::Zstd.available() {
            let target = sibling("ofc_sibling_old.js", ".target", b"precompressed bytes", 1_767_223_000);
            let link = TempPath::new_in_cwd("ofc_sibling_old.js.zst");
            std::os::unix::fs::symlink(target.as_str(), link.as_str()).unwrap();
            let Encoded::Smaller(bytes) = file.encoded(&state, Encoding::Zstd, path.as_str()).unwrap() else { panic!() };
            assert_ne!(b"precompressed bytes", &bytes[..]);
        }
    }

    #[test]
    fn non_strict_state_reuses_the_last_fstat_within_the_window() {
        let path = old_text_file("ofc_trust.txt", "12345");
        let file = open_with(path.as_str(), true, LONG).unwrap();
        let first = file.state_with(false, LONG).unwrap();
        assert_eq!(5, first.len);
        std::fs::write(path.as_str(), "1234567890").unwrap();
        // Trusted: the cached size, without an fstat.
        assert_eq!(5, file.state_with(false, LONG).unwrap().len);
        // Strict (the default) sees the edit at once; so does an expired window.
        assert_eq!(10, file.state_with(true, LONG).unwrap().len);
        assert_eq!(10, file.state_with(false, Duration::ZERO).unwrap().len);
    }
}
