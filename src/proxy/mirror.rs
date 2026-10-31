//! Request mirroring (18.11.0, #39): send a copy of a share of requests to
//! other backends — a new version under real traffic, a shadow service —
//! and throw the answers away.
//!
//! Fire-and-forget: the client's request never waits for, or depends on, a
//! mirror. Copies go through a bounded queue to a few worker threads (not a
//! thread per copy), so a slow or dead mirror can't use up threads or
//! memory; when the queue is full the copy is dropped and counted. Only
//! requests whose body is in memory and at most [`MAX_MIRRORED_BODY`] are
//! mirrored: a streamed upload is read once, by the real backend.
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::proxy::mirror::{Mirror, MirrorLayer};
//!
//! let shadow = Mirror::new("v2-shadow", vec!["http://10.0.0.9:8080".into()]).percent(10.0);
//! let app = App::new().wrap(MirrorLayer::new(shadow));
//! ```
//!
//! Counts: `rws_mirror_requests_total{mirror,outcome}` with outcome `sent`
//! (the mirror answered), `failed` (no answer), `dropped` (queue full) and
//! `skipped` (body streamed or too large).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::Duration;

use crate::application::Application;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

/// Copies waiting for a worker; past this, copies are dropped.
pub const QUEUE: usize = 1024;
/// Worker threads sending copies (shared by every mirror).
pub const WORKERS: usize = 4;
/// Larger request bodies aren't mirrored.
pub const MAX_MIRRORED_BODY: usize = 64 * 1024;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const READ_TIMEOUT: Duration = Duration::from_secs(10);

/// One mirror target: backends (round-robin) and the share of requests
/// copied to it. Cheap to clone; clones share their state.
#[derive(Clone)]
pub struct Mirror {
    inner: Arc<Inner>,
}

struct Inner {
    name: String,
    /// `http://` / `https://` URLs or `host:port`; a live list (health
    /// checks may change it).
    backends: Arc<RwLock<Vec<String>>>,
    /// Share of requests copied, 0–100.
    percent: f64,
    seen: AtomicU64,
    next: AtomicU64,
}

impl std::fmt::Debug for Mirror {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mirror").field("name", &self.inner.name).field("percent", &self.inner.percent).finish_non_exhaustive()
    }
}

impl Mirror {
    /// Copy every request to `backends` (round-robin).
    pub fn new(name: impl Into<String>, backends: Vec<String>) -> Mirror {
        Mirror::with_live_backends(name, Arc::new(RwLock::new(backends)))
    }

    /// Like [`Mirror::new`], with a backend list another component updates
    /// (a config-file upstream's health-checked list).
    pub fn with_live_backends(name: impl Into<String>, backends: Arc<RwLock<Vec<String>>>) -> Mirror {
        Mirror { inner: Arc::new(Inner { name: name.into(), backends, percent: 100.0, seen: AtomicU64::new(0), next: AtomicU64::new(0) }) }
    }

    /// Copy only this share of requests (0–100), spread evenly: 25 copies
    /// every 4th request, not 25 in a row.
    pub fn percent(self, percent: f64) -> Mirror {
        let percent = if percent.is_finite() { percent.clamp(0.0, 100.0) } else { 0.0 };
        let inner = &self.inner;
        Mirror {
            inner: Arc::new(Inner {
                name: inner.name.clone(),
                backends: Arc::clone(&inner.backends),
                percent,
                seen: AtomicU64::new(0),
                next: AtomicU64::new(0),
            }),
        }
    }

    pub fn name(&self) -> &str {
        &self.inner.name
    }

    /// Whether the `n`th request (from 0) is in the sampled share.
    fn sampled(&self) -> bool {
        let p = self.inner.percent;
        if p >= 100.0 {
            return true;
        }
        if p <= 0.0 {
            return false;
        }
        let n = self.inner.seen.fetch_add(1, Ordering::Relaxed) as f64;
        ((n + 1.0) * p / 100.0).floor() > (n * p / 100.0).floor()
    }

    /// Queue a copy of `request` if it's in the sampled share; never blocks.
    /// `true` when a copy was queued.
    pub fn send(&self, request: &Request, connection: &ConnectionInfo) -> bool {
        if !self.sampled() {
            return false;
        }
        if request.body.is_stream() || request.body.len() > MAX_MIRRORED_BODY {
            count(&self.inner.name, "skipped");
            return false;
        }
        let backend = {
            let backends = self.inner.backends.read().unwrap_or_else(|e| e.into_inner());
            if backends.is_empty() {
                count(&self.inner.name, "failed");
                return false;
            }
            let i = self.inner.next.fetch_add(1, Ordering::Relaxed) as usize % backends.len();
            backends[i].clone()
        };
        let job = Job { mirror: self.inner.name.clone(), backend, request: request.clone(), client_ip: connection.client.ip.clone(), tls: connection.tls };
        match queue().try_send(job) {
            Ok(()) => true,
            Err(TrySendError::Full(job)) | Err(TrySendError::Disconnected(job)) => {
                count(&job.mirror, "dropped");
                false
            }
        }
    }
}

struct Job {
    mirror: String,
    backend: String,
    request: Request,
    client_ip: String,
    tls: bool,
}

fn queue() -> &'static SyncSender<Job> {
    static QUEUE_TX: OnceLock<SyncSender<Job>> = OnceLock::new();
    QUEUE_TX.get_or_init(|| {
        let (tx, rx) = sync_channel::<Job>(QUEUE);
        let rx = Arc::new(Mutex::new(rx));
        for i in 0..WORKERS {
            let rx = Arc::clone(&rx);
            let _ = std::thread::Builder::new().name(format!("rws-mirror-{i}")).spawn(move || loop {
                let job = match rx.lock().unwrap_or_else(|e| e.into_inner()).recv() {
                    Ok(job) => job,
                    Err(_) => return,
                };
                let outcome = if deliver(&job) { "sent" } else { "failed" };
                count(&job.mirror, outcome);
            });
        }
        tx
    })
}

/// Send one copy and wait for (and drop) the answer.
fn deliver(job: &Job) -> bool {
    let Some((host, port, tls)) = crate::proxy_config::health::parse_backend_url(&job.backend) else { return false };
    crate::proxy::forward_upstream(
        crate::proxy::shared_pool(),
        &job.request,
        crate::proxy::Peer { ip: &job.client_ip, tls: job.tls },
        &host,
        port,
        tls,
        CONNECT_TIMEOUT,
        READ_TIMEOUT,
    )
    .map(|response| {
        // A streamed answer is read to the end so the connection isn't
        // left half-used; then it's dropped.
        if let Some(mut pipe) = response.stream_pipe {
            let _ = std::io::copy(&mut pipe, &mut std::io::sink());
        }
    })
    .is_ok()
}

fn counts() -> &'static Mutex<BTreeMap<(String, &'static str), u64>> {
    static COUNTS: OnceLock<Mutex<BTreeMap<(String, &'static str), u64>>> = OnceLock::new();
    COUNTS.get_or_init(Mutex::default)
}

fn count(mirror: &str, outcome: &'static str) {
    *counts().lock().unwrap_or_else(|e| e.into_inner()).entry((mirror.to_string(), outcome)).or_default() += 1;
}

/// The `rws_mirror_requests_total` series; empty until a mirror is used.
pub(crate) fn prometheus_text() -> String {
    let counts = counts().lock().unwrap_or_else(|e| e.into_inner());
    if counts.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "# HELP rws_mirror_requests_total Request copies sent to mirrors, by outcome: sent, failed, dropped (queue full), skipped (body streamed or too large)\n# TYPE rws_mirror_requests_total counter\n",
    );
    for ((mirror, outcome), n) in counts.iter() {
        let mirror = mirror.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
        out.push_str(&format!("rws_mirror_requests_total{{mirror=\"{mirror}\",outcome=\"{outcome}\"}} {n}\n"));
    }
    out
}

/// A [`Mirror`] as middleware: copies the request, then hands it on.
#[derive(Clone, Debug)]
pub struct MirrorLayer {
    mirror: Mirror,
}

impl MirrorLayer {
    pub fn new(mirror: Mirror) -> MirrorLayer {
        MirrorLayer { mirror }
    }
}

impl Middleware for MirrorLayer {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        self.mirror.send(request, connection);
        next.execute(request, connection)
    }

    /// Queueing a copy never blocks.
    fn runs_inline(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }

    /// A streamed body passes through untouched (and isn't mirrored).
    fn accepts_streamed_body(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }
}

#[cfg(test)]
mod tests;
