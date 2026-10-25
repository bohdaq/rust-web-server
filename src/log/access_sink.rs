//! Where access-log lines go: a buffer per thread, written to stdout by a
//! background thread every [`FLUSH_INTERVAL`] (or at once when a buffer
//! passes [`SHARD_LIMIT`]), when the thread exits, and at process exit.
//! `println!` per request took the process-wide stdout lock and made one
//! `write` syscall per line — a sixth of a small request's CPU.
//!
//! Lines from one thread stay in order; lines from different threads are
//! interleaved by flush, each with its own timestamp. A line can reach
//! stdout up to [`FLUSH_INTERVAL`] after its request.

use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

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
        write_stdout(&pending);
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
        Ok(Some(full)) => write_stdout(&full),
        Ok(None) => {}
        // The thread is exiting (its thread-locals are gone): write directly.
        Err(_) => {
            if let Some(write) = write.take() {
                let mut line = Vec::new();
                write(&mut line);
                line.push(b'\n');
                write_stdout(&line);
            }
        }
    }
}

/// Write out every thread's buffered lines now.
pub fn flush() {
    let shards: Vec<Arc<Shard>> = registry().lock().unwrap_or_else(|e| e.into_inner()).clone();
    for shard in shards {
        let pending = std::mem::take(&mut *shard.0.lock().unwrap_or_else(|e| e.into_inner()));
        write_stdout(&pending);
    }
}

fn write_stdout(bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(bytes);
    let _ = out.flush();
}
