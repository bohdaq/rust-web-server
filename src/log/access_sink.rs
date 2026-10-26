//! Where access-log lines go: a buffer per thread, written to stdout by a
//! background thread every [`FLUSH_INTERVAL`] (or at once when a buffer
//! passes [`SHARD_LIMIT`]), when the thread exits, and at process exit.
//! `println!` per request took the process-wide stdout lock and made one
//! `write` syscall per line — a sixth of a small request's CPU.
//!
//! Lines from one thread stay in order; lines from different threads are
//! interleaved by flush, each with its own timestamp. A line can reach
//! stdout up to [`FLUSH_INTERVAL`] after its request.
//!
//! With `RWS_CONFIG_ACCESS_LOG` set (18.1.0), lines go to that file instead,
//! appended; past `RWS_CONFIG_ACCESS_LOG_MAX_SIZE` bytes it is rotated
//! (`access.log` → `access.log.1` …, keeping `RWS_CONFIG_ACCESS_LOG_KEEP`),
//! and `SIGUSR1` reopens it — what logrotate's `postrotate` sends. Read once,
//! at the first line.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// `RWS_CONFIG_ACCESS_LOG`: a file to append access-log lines to; empty or
/// unset keeps them on stdout.
pub const RWS_CONFIG_ACCESS_LOG: &str = "RWS_CONFIG_ACCESS_LOG";
/// `RWS_CONFIG_ACCESS_LOG_MAX_SIZE`: rotate the file past this many bytes
/// (`0`, the default: never — leave it to logrotate and `SIGUSR1`).
pub const RWS_CONFIG_ACCESS_LOG_MAX_SIZE: &str = "RWS_CONFIG_ACCESS_LOG_MAX_SIZE";
/// `RWS_CONFIG_ACCESS_LOG_KEEP`: rotated files kept (default 5).
pub const RWS_CONFIG_ACCESS_LOG_KEEP: &str = "RWS_CONFIG_ACCESS_LOG_KEEP";

/// Set by `SIGUSR1` (or [`reopen`]): the next write reopens the file.
static REOPEN: AtomicBool = AtomicBool::new(false);

/// Ask for the access-log file to be reopened before its next write — after
/// an external tool moved it away. Safe to call from a signal handler.
pub fn reopen() {
    REOPEN.store(true, Ordering::Relaxed);
}

/// Make `SIGUSR1` reopen the access-log file (Unix), when one is configured
/// — otherwise `SIGUSR1` keeps its default action. The handler only sets a
/// flag (async-signal-safe); the reopen happens on the next write.
pub fn install_reopen_signal() {
    if std::env::var(RWS_CONFIG_ACCESS_LOG).map_or(true, |p| p.trim().is_empty()) {
        return;
    }
    #[cfg(all(unix, any(feature = "http1", feature = "http2")))]
    {
        extern "C" fn on_sigusr1(_: libc::c_int) {
            REOPEN.store(true, Ordering::Relaxed);
        }
        // SAFETY: installs a handler that only stores to an atomic.
        unsafe {
            libc::signal(libc::SIGUSR1, on_sigusr1 as *const () as libc::sighandler_t);
        }
    }
}

/// An access-log file: appended to, rotated by size, reopened on request.
pub(crate) struct LogFile {
    path: PathBuf,
    file: File,
    size: u64,
    max_size: u64,
    keep: u32,
}

impl LogFile {
    pub(crate) fn open(path: &Path, max_size: u64, keep: u32) -> std::io::Result<LogFile> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let size = file.metadata()?.len();
        Ok(LogFile { path: path.to_path_buf(), file, size, max_size, keep })
    }

    /// Append `bytes`: reopen first if asked to, rotate first if they would
    /// take the file past its limit (an empty file takes them whatever their
    /// size).
    pub(crate) fn write(&mut self, bytes: &[u8], reopen: bool) -> std::io::Result<()> {
        if reopen {
            *self = LogFile::open(&self.path, self.max_size, self.keep)?;
        }
        if self.max_size > 0 && self.size > 0 && self.size + bytes.len() as u64 > self.max_size {
            self.rotate()?;
        }
        self.file.write_all(bytes)?;
        self.size += bytes.len() as u64;
        Ok(())
    }

    /// `access.log.{keep-1}` → `.{keep}` … `access.log` → `access.log.1`,
    /// then a new `access.log`. With `keep` 0 the file is just emptied.
    fn rotate(&mut self) -> std::io::Result<()> {
        let numbered = |n: u32| {
            let mut name = self.path.clone().into_os_string();
            name.push(format!(".{n}"));
            PathBuf::from(name)
        };
        if self.keep == 0 {
            self.file = OpenOptions::new().create(true).write(true).truncate(true).open(&self.path)?;
            self.file = OpenOptions::new().append(true).open(&self.path)?;
            self.size = 0;
            return Ok(());
        }
        let _ = std::fs::remove_file(numbered(self.keep));
        for n in (1..self.keep).rev() {
            let _ = std::fs::rename(numbered(n), numbered(n + 1));
        }
        std::fs::rename(&self.path, numbered(1))?;
        *self = LogFile::open(&self.path, self.max_size, self.keep)?;
        Ok(())
    }
}

/// Where flushed lines go, decided at the first line.
enum Destination {
    Stdout,
    File(Mutex<LogFile>),
}

fn destination() -> &'static Destination {
    static DESTINATION: OnceLock<Destination> = OnceLock::new();
    DESTINATION.get_or_init(|| {
        let path = std::env::var(RWS_CONFIG_ACCESS_LOG).unwrap_or_default();
        if path.trim().is_empty() {
            return Destination::Stdout;
        }
        let number = |key: &str, default: u64| std::env::var(key).ok().and_then(|v| parse_size(&v)).unwrap_or(default);
        let max_size = number(RWS_CONFIG_ACCESS_LOG_MAX_SIZE, 0);
        let keep = number(RWS_CONFIG_ACCESS_LOG_KEEP, 5).min(1000) as u32;
        match LogFile::open(Path::new(path.trim()), max_size, keep) {
            Ok(file) => Destination::File(Mutex::new(file)),
            Err(e) => {
                eprintln!("access log: can't open {}: {e}; writing to stdout", path.trim());
                Destination::Stdout
            }
        }
    })
}

/// A byte count: digits, optionally followed by `K`, `M` or `G` (×1024).
pub(crate) fn parse_size(value: &str) -> Option<u64> {
    let value = value.trim();
    let (digits, multiplier) = match value.as_bytes().last()? {
        b'k' | b'K' => (&value[..value.len() - 1], 1 << 10),
        b'm' | b'M' => (&value[..value.len() - 1], 1 << 20),
        b'g' | b'G' => (&value[..value.len() - 1], 1 << 30),
        _ => (value, 1),
    };
    digits.trim().parse::<u64>().ok()?.checked_mul(multiplier)
}

/// How often the background thread writes out every buffer.
pub(crate) const FLUSH_INTERVAL: Duration = Duration::from_millis(100);
/// A thread's buffer is written out at once past this size.
pub(crate) const SHARD_LIMIT: usize = 32 * 1024;

struct Shard(Mutex<Vec<u8>>);

fn registry() -> &'static Mutex<Vec<Arc<Shard>>> {
    static REGISTRY: OnceLock<Mutex<Vec<Arc<Shard>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let spawned = std::thread::Builder::new()
            .name("rws-access-log".to_string())
            .spawn(|| loop {
                std::thread::sleep(FLUSH_INTERVAL);
                flush();
            });
        if spawned.is_err() {
            eprintln!("access log: no flusher thread; lines are written when a buffer fills or its thread exits");
        }
        #[cfg(all(unix, any(feature = "http1", feature = "http2")))]
        {
            extern "C" fn flush_at_exit() {
                flush();
            }
            // SAFETY: registers a plain `extern "C"` function with no
            // arguments, as atexit requires; it only takes our locks.
            unsafe {
                libc::atexit(flush_at_exit);
            }
        }
        Mutex::new(Vec::new())
    })
}

/// This thread's shard; flushed and unregistered when the thread exits (the
/// blocking pool's threads come and go).
struct ShardHandle(Arc<Shard>);

impl Drop for ShardHandle {
    fn drop(&mut self) {
        let pending = std::mem::take(&mut *self.0 .0.lock().unwrap_or_else(|e| e.into_inner()));
        write_out(&pending);
        registry().lock().unwrap_or_else(|e| e.into_inner()).retain(|s| !Arc::ptr_eq(s, &self.0));
    }
}

thread_local! {
    static SHARD: ShardHandle = {
        let shard = Arc::new(Shard(Mutex::new(Vec::with_capacity(4096))));
        registry().lock().unwrap_or_else(|e| e.into_inner()).push(Arc::clone(&shard));
        ShardHandle(shard)
    };
}

/// Append one line (written by `write`, without the newline) to this
/// thread's buffer.
pub(crate) fn write_line(write: impl FnOnce(&mut Vec<u8>)) {
    let mut write = Some(write);
    let full = SHARD.try_with(|handle| {
        let mut buf = handle.0 .0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(write) = write.take() {
            write(&mut buf);
            buf.push(b'\n');
        }
        (buf.len() >= SHARD_LIMIT).then(|| std::mem::take(&mut *buf))
    });
    match full {
        Ok(Some(full)) => write_out(&full),
        Ok(None) => {}
        // The thread is exiting (its thread-locals are gone): write directly.
        Err(_) => {
            if let Some(write) = write.take() {
                let mut line = Vec::new();
                write(&mut line);
                line.push(b'\n');
                write_out(&line);
            }
        }
    }
}

/// Write out every thread's buffered lines now.
pub fn flush() {
    let shards: Vec<Arc<Shard>> = registry().lock().unwrap_or_else(|e| e.into_inner()).clone();
    for shard in shards {
        let pending = std::mem::take(&mut *shard.0.lock().unwrap_or_else(|e| e.into_inner()));
        write_out(&pending);
    }
}

fn write_out(bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    match destination() {
        Destination::Stdout => write_stdout(bytes),
        Destination::File(file) => {
            let mut file = file.lock().unwrap_or_else(|e| e.into_inner());
            if let Err(e) = file.write(bytes, REOPEN.swap(false, Ordering::Relaxed)) {
                eprintln!("access log: {e}");
            }
        }
    }
}

fn write_stdout(bytes: &[u8]) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(bytes);
    let _ = out.flush();
}
