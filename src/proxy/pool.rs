use std::collections::{HashMap, VecDeque};
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const DEFAULT_MAX_IDLE: usize = 8;
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// Per-backend HTTP/1.1 connection pool.
///
/// Holds idle `TcpStream` connections keyed by `"host:port"`.  When a
/// backend responds with `Connection: keep-alive` (or the HTTP/1.1 default),
/// the stream is returned here and reused for the next request to the same
/// backend, eliminating the TCP-handshake cost and reducing ephemeral-port
/// exhaustion under load.
///
/// # Thread safety
///
/// All methods take `&self` and are safe to call from multiple threads.
/// The inner map is protected by a `Mutex`.
///
/// # Example
///
/// ```rust,ignore
/// use std::sync::Arc;
/// use std::time::Duration;
/// use rust_web_server::proxy::ConnPool;
/// use rust_web_server::proxy::ReverseProxy;
///
/// let pool = Arc::new(ConnPool::new(16, Duration::from_secs(30)));
/// let _proxy = ReverseProxy::new(["http://backend:8080"])
///     .with_pool(Arc::clone(&pool));
/// ```
pub struct ConnPool {
    inner: Mutex<HashMap<String, VecDeque<PoolEntry>>>,
    max_idle: usize,
    idle_timeout: Duration,
}

struct PoolEntry {
    stream: TcpStream,
    added: Instant,
}

impl ConnPool {
    /// Create a pool with the given per-backend idle limit and idle timeout.
    pub fn new(max_idle: usize, idle_timeout: Duration) -> Self {
        ConnPool {
            inner: Mutex::new(HashMap::new()),
            max_idle,
            idle_timeout,
        }
    }

    /// Create a pool with defaults: 8 idle connections per backend, 60-second timeout.
    pub fn new_default() -> Self {
        Self::new(DEFAULT_MAX_IDLE, DEFAULT_IDLE_TIMEOUT)
    }

    /// Try to acquire an idle connection for `key = "host:port"`.
    ///
    /// Stale entries (older than `idle_timeout`) are discarded automatically.
    /// Returns `None` if no usable connection is available.
    pub fn acquire(&self, key: &str) -> Option<TcpStream> {
        let mut map = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let queue = map.get_mut(key)?;
        let now = Instant::now();
        while let Some(entry) = queue.pop_front() {
            if now.duration_since(entry.added) < self.idle_timeout {
                return Some(entry.stream);
            }
            // stale — drop, which closes the TCP connection
        }
        None
    }

    /// Return a keep-alive connection to the pool.
    ///
    /// If the backend slot is already at `max_idle`, the stream is dropped
    /// (the TCP connection closes) rather than exceeding the limit.
    pub fn release(&self, key: &str, stream: TcpStream) {
        let mut map = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let queue = map.entry(key.to_string()).or_default();
        if queue.len() < self.max_idle {
            queue.push_back(PoolEntry { stream, added: Instant::now() });
        }
        // over limit — stream dropped here, closing the connection
    }

    /// Total idle connections across all backends (useful for testing/metrics).
    pub fn idle_count(&self) -> usize {
        let map = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        map.values().map(|q| q.len()).sum()
    }
}
