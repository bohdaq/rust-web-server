//! Background job queue for one-shot, fire-and-forget work triggered from
//! request handlers — e.g. "send a confirmation email after signup" — without
//! blocking the response.
//!
//! Two flavors:
//! - [`JobQueue`] — an in-memory worker pool. Simple and dependency-free, but
//!   queued and retrying jobs are lost if the process crashes or restarts.
//! - [`PersistentJobQueue`] (requires a `model-sqlite` / `model-postgres` /
//!   `model-mysql` feature) — persists jobs via the model layer so pending
//!   and retrying jobs survive a restart.
//!
//! # Example
//!
//! ```rust
//! use rust_web_server::jobs::JobQueue;
//!
//! let queue = JobQueue::new(4); // 4 worker threads
//! queue.submit(|| {
//!     println!("sending welcome email...");
//!     Ok(())
//! });
//! queue.join(); // wait for in-flight jobs to finish (tests/shutdown only)
//! ```

#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// ── Job ──────────────────────────────────────────────────────────────────────

/// A unit of background work.
///
/// Implement `run` directly for named, stateful jobs, or rely on the blanket
/// implementation below to submit a plain closure.
pub trait Job: Send {
    /// Perform the work. Return `Err(message)` to trigger a retry (subject to
    /// the queue's retry policy).
    fn run(&self) -> Result<(), String>;

    /// Human-readable name used in retry/failure log lines. Defaults to `"job"`.
    fn name(&self) -> &str {
        "job"
    }
}

impl<F> Job for F
where
    F: Fn() -> Result<(), String> + Send,
{
    fn run(&self) -> Result<(), String> {
        self()
    }
}

struct Envelope {
    job: Box<dyn Job>,
    attempt: u32,
}

// ── JobQueue ─────────────────────────────────────────────────────────────────

/// An in-memory background job queue backed by a fixed-size worker pool.
///
/// Jobs that return `Err` are retried with exponential backoff (default: 3
/// retries, 500 ms initial backoff, 2x multiplier) — the same worker thread
/// that picked up the job sleeps between attempts, so a job that retries
/// repeatedly occupies one worker for the duration of its backoff; size the
/// pool with that in mind. Queued and retrying jobs are lost if the process
/// exits; see [`PersistentJobQueue`] for crash-safe persistence.
pub struct JobQueue {
    sender: Sender<Envelope>,
    workers: Vec<thread::JoinHandle<()>>,
    max_retries: Arc<AtomicU32>,
    initial_backoff_ms: Arc<AtomicU64>,
    backoff_multiplier: Arc<AtomicU32>,
}

impl JobQueue {
    /// Create a queue backed by `workers` worker threads.
    ///
    /// Default retry policy: 3 retries, 500 ms initial backoff, 2x multiplier.
    /// Adjust it with [`max_retries`](Self::max_retries) / [`backoff`](Self::backoff)
    /// before submitting jobs.
    pub fn new(workers: usize) -> Self {
        assert!(workers > 0, "JobQueue requires at least one worker");

        let (sender, receiver) = mpsc::channel::<Envelope>();
        let receiver = Arc::new(std::sync::Mutex::new(receiver));

        let max_retries = Arc::new(AtomicU32::new(3));
        let initial_backoff_ms = Arc::new(AtomicU64::new(500));
        let backoff_multiplier = Arc::new(AtomicU32::new(2));

        let mut handles = Vec::with_capacity(workers);
        for _ in 0..workers {
            let receiver = Arc::clone(&receiver);
            let max_retries = Arc::clone(&max_retries);
            let initial_backoff_ms = Arc::clone(&initial_backoff_ms);
            let backoff_multiplier = Arc::clone(&backoff_multiplier);
            handles.push(thread::spawn(move || {
                worker_loop(receiver, max_retries, initial_backoff_ms, backoff_multiplier);
            }));
        }

        JobQueue {
            sender,
            workers: handles,
            max_retries,
            initial_backoff_ms,
            backoff_multiplier,
        }
    }

    /// Override the number of retries after the initial attempt (default: 3).
    pub fn max_retries(self, n: u32) -> Self {
        self.max_retries.store(n, Ordering::Relaxed);
        self
    }

    /// Override the backoff policy: `initial` delay before the first retry,
    /// doubled (or scaled by `multiplier`) on each subsequent attempt.
    pub fn backoff(self, initial: Duration, multiplier: u32) -> Self {
        self.initial_backoff_ms.store(initial.as_millis() as u64, Ordering::Relaxed);
        self.backoff_multiplier.store(multiplier.max(1), Ordering::Relaxed);
        self
    }

    /// Submit a job for background execution. Returns immediately.
    pub fn submit(&self, job: impl Job + 'static) {
        let _ = self.sender.send(Envelope { job: Box::new(job), attempt: 0 });
    }

    /// Stop accepting new jobs and wait for every worker to finish its
    /// current job (including any retries already in progress). Consumes
    /// the queue.
    pub fn join(self) {
        let JobQueue { sender, workers, .. } = self;
        drop(sender);
        for handle in workers {
            let _ = handle.join();
        }
    }
}

fn worker_loop(
    receiver: Arc<std::sync::Mutex<mpsc::Receiver<Envelope>>>,
    max_retries: Arc<AtomicU32>,
    initial_backoff_ms: Arc<AtomicU64>,
    backoff_multiplier: Arc<AtomicU32>,
) {
    loop {
        let received = {
            let lock = receiver.lock().unwrap();
            lock.recv()
        };
        let mut envelope = match received {
            Ok(envelope) => envelope,
            Err(_) => break, // sender dropped -> queue shut down
        };

        loop {
            match envelope.job.run() {
                Ok(()) => break,
                Err(e) => {
                    let max = max_retries.load(Ordering::Relaxed);
                    if envelope.attempt >= max {
                        eprintln!(
                            "[jobs] '{}' failed permanently after {} attempt(s): {}",
                            envelope.job.name(),
                            envelope.attempt + 1,
                            e
                        );
                        break;
                    }

                    let initial_ms = initial_backoff_ms.load(Ordering::Relaxed);
                    let multiplier = backoff_multiplier.load(Ordering::Relaxed).max(1) as u64;
                    let delay = Duration::from_millis(
                        initial_ms.saturating_mul(multiplier.saturating_pow(envelope.attempt)),
                    );
                    eprintln!(
                        "[jobs] '{}' failed (attempt {}/{}): {} — retrying in {:?}",
                        envelope.job.name(),
                        envelope.attempt + 1,
                        max + 1,
                        e,
                        delay
                    );
                    thread::sleep(delay);
                    envelope.attempt += 1;
                }
            }
        }
    }
}

// ── PersistentJobQueue ───────────────────────────────────────────────────────

/// A job queue backed by the model layer (`model-sqlite` / `model-postgres` /
/// `model-mysql`): jobs are written to a `rws_jobs` table before being
/// acknowledged, so pending and retrying jobs survive a process crash or
/// restart.
///
/// Unlike [`JobQueue`], a job here can't be an arbitrary closure — a closure
/// can't be serialized to disk. Instead the queue stores `(job_type,
/// payload)` as plain text and dispatches to a handler registered for that
/// `job_type` at execution time. Register every handler you enqueue *before*
/// calling [`start`](Self::start), including after a restart — unprocessed
/// rows from a previous run (and any row left `running` when the process
/// crashed) are picked up automatically.
///
/// ```sql
/// CREATE TABLE IF NOT EXISTS rws_jobs (
///     id          TEXT    PRIMARY KEY,
///     job_type    TEXT    NOT NULL,
///     payload     TEXT    NOT NULL DEFAULT '',
///     status      TEXT    NOT NULL DEFAULT 'pending', -- pending | running | failed
///     attempts    INTEGER NOT NULL DEFAULT 0,
///     max_retries INTEGER NOT NULL DEFAULT 3,
///     next_run_at INTEGER NOT NULL,
///     created_at  INTEGER NOT NULL,
///     last_error  TEXT
/// )
/// ```
///
/// Completed jobs are deleted; a job that exhausts its retries is left with
/// `status = 'failed'` for inspection instead of being deleted.
#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
pub struct PersistentJobQueue {
    pool: crate::model::DbPool,
    handlers: std::sync::RwLock<std::collections::HashMap<String, Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>>>,
    default_max_retries: AtomicU32,
    poll_interval_ms: AtomicU64,
    initial_backoff_ms: AtomicU64,
    backoff_multiplier: AtomicU32,
    stop: Arc<std::sync::atomic::AtomicBool>,
    id_counter: AtomicU64,
}

#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
impl PersistentJobQueue {
    /// Creates the queue and its backing `rws_jobs` table if absent, then
    /// resets any row left `running` (interrupted by a previous crash) back
    /// to `pending` so it gets picked up again.
    pub async fn new(pool: crate::model::DbPool) -> Result<Self, crate::model::DbError> {
        let queue = PersistentJobQueue {
            pool,
            handlers: std::sync::RwLock::new(std::collections::HashMap::new()),
            default_max_retries: AtomicU32::new(3),
            poll_interval_ms: AtomicU64::new(500),
            initial_backoff_ms: AtomicU64::new(500),
            backoff_multiplier: AtomicU32::new(2),
            stop: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            id_counter: AtomicU64::new(0),
        };
        queue.ensure_table().await?;
        queue.recover_interrupted().await?;
        Ok(queue)
    }

    async fn ensure_table(&self) -> Result<(), crate::model::DbError> {
        self.pool
            .execute(
                "CREATE TABLE IF NOT EXISTS rws_jobs \
                 (id TEXT PRIMARY KEY, job_type TEXT NOT NULL, payload TEXT NOT NULL DEFAULT '', \
                  status TEXT NOT NULL DEFAULT 'pending', attempts INTEGER NOT NULL DEFAULT 0, \
                  max_retries INTEGER NOT NULL DEFAULT 3, next_run_at INTEGER NOT NULL, \
                  created_at INTEGER NOT NULL, last_error TEXT)",
                &[],
            )
            .await?;
        Ok(())
    }

    async fn recover_interrupted(&self) -> Result<(), crate::model::DbError> {
        self.pool
            .execute("UPDATE rws_jobs SET status = 'pending' WHERE status = 'running'", &[])
            .await?;
        Ok(())
    }

    /// Override the default retry count applied to jobs enqueued via
    /// [`enqueue`](Self::enqueue) (default: 3). Use
    /// [`enqueue_with_retries`](Self::enqueue_with_retries) for a per-job override.
    pub fn max_retries(self, n: u32) -> Self {
        self.default_max_retries.store(n, Ordering::Relaxed);
        self
    }

    /// Override how often idle workers poll for due jobs (default: 500 ms).
    pub fn poll_interval(self, interval: Duration) -> Self {
        self.poll_interval_ms.store(interval.as_millis() as u64, Ordering::Relaxed);
        self
    }

    /// Override the backoff policy applied between retries: `initial` delay
    /// before the first retry, doubled (or scaled by `multiplier`) on each
    /// subsequent attempt. Default: 500 ms initial, 2x multiplier.
    pub fn backoff(self, initial: Duration, multiplier: u32) -> Self {
        self.initial_backoff_ms.store(initial.as_millis() as u64, Ordering::Relaxed);
        self.backoff_multiplier.store(multiplier.max(1), Ordering::Relaxed);
        self
    }

    /// Register the function that executes jobs of `job_type`. Call before
    /// [`start`](Self::start) — jobs enqueued for an unregistered `job_type`
    /// fail immediately (and retry, same as any other handler error).
    pub fn register(&self, job_type: &str, handler: impl Fn(&str) -> Result<(), String> + Send + Sync + 'static) {
        self.handlers.write().unwrap().insert(job_type.to_string(), Arc::new(handler));
    }

    fn generate_id(&self) -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let count = self.id_counter.fetch_add(1, Ordering::Relaxed);
        format!("{:x}-{:x}", nanos, count)
    }

    fn now_epoch() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    /// Persist a new job using the queue's default retry count. Returns the
    /// generated job id.
    pub async fn enqueue(&self, job_type: &str, payload: &str) -> Result<String, crate::model::DbError> {
        let max_retries = self.default_max_retries.load(Ordering::Relaxed);
        self.enqueue_with_retries(job_type, payload, max_retries).await
    }

    /// Persist a new job with a per-job retry override.
    pub async fn enqueue_with_retries(
        &self,
        job_type: &str,
        payload: &str,
        max_retries: u32,
    ) -> Result<String, crate::model::DbError> {
        use crate::model::Value;
        let id = self.generate_id();
        let now = Self::now_epoch();
        self.pool
            .execute(
                "INSERT INTO rws_jobs (id, job_type, payload, status, attempts, max_retries, next_run_at, created_at) \
                 VALUES (?, ?, ?, 'pending', 0, ?, ?, ?)",
                &[
                    Value::Text(id.clone()),
                    Value::Text(job_type.to_string()),
                    Value::Text(payload.to_string()),
                    Value::Int(max_retries as i64),
                    Value::Int(now),
                    Value::Int(now),
                ],
            )
            .await?;
        Ok(id)
    }

    /// Atomically claims the oldest due job (`status = 'pending'` and
    /// `next_run_at <= now`), marking it `running`. Returns `None` if no job
    /// is due. The `UPDATE ... WHERE status = 'pending'` guards against two
    /// workers claiming the same row.
    async fn claim_next(&self) -> Result<Option<(String, String, String, i64, i64)>, crate::model::DbError> {
        use crate::model::Value;
        let now = Self::now_epoch();
        let rows = self
            .pool
            .query_rows(
                "SELECT id, job_type, payload, attempts, max_retries FROM rws_jobs \
                 WHERE status = 'pending' AND next_run_at <= ? ORDER BY next_run_at ASC LIMIT 1",
                &[Value::Int(now)],
            )
            .await?;
        let row = match rows.into_iter().next() {
            Some(r) => r,
            None => return Ok(None),
        };
        let id: String = row.get("id")?;
        let claimed = self
            .pool
            .execute(
                "UPDATE rws_jobs SET status = 'running' WHERE id = ? AND status = 'pending'",
                &[Value::Text(id.clone())],
            )
            .await?;
        if claimed == 0 {
            return Ok(None); // another worker won the race
        }
        Ok(Some((id, row.get("job_type")?, row.get("payload")?, row.get("attempts")?, row.get("max_retries")?)))
    }

    async fn mark_done(&self, id: &str) -> Result<(), crate::model::DbError> {
        use crate::model::Value;
        self.pool.execute("DELETE FROM rws_jobs WHERE id = ?", &[Value::Text(id.to_string())]).await?;
        Ok(())
    }

    async fn mark_failed(&self, id: &str, error: &str) -> Result<(), crate::model::DbError> {
        use crate::model::Value;
        self.pool
            .execute(
                "UPDATE rws_jobs SET status = 'failed', last_error = ? WHERE id = ?",
                &[Value::Text(error.to_string()), Value::Text(id.to_string())],
            )
            .await?;
        Ok(())
    }

    async fn reschedule(&self, id: &str, attempts: i64, error: &str, delay: Duration) -> Result<(), crate::model::DbError> {
        use crate::model::Value;
        let next_run_at = Self::now_epoch() + delay.as_secs() as i64;
        self.pool
            .execute(
                "UPDATE rws_jobs SET status = 'pending', attempts = ?, last_error = ?, next_run_at = ? WHERE id = ?",
                &[Value::Int(attempts), Value::Text(error.to_string()), Value::Int(next_run_at), Value::Text(id.to_string())],
            )
            .await?;
        Ok(())
    }

    /// Run one poll-claim-execute cycle. Returns `Ok(true)` if a job was
    /// claimed (whether it succeeded, failed, or is retrying), `Ok(false)` if
    /// none was due. Exposed for tests and for embedding in a caller-owned
    /// loop; most applications should use [`start`](Self::start) instead.
    pub async fn tick(&self) -> Result<bool, crate::model::DbError> {
        let (id, job_type, payload, attempts, max_retries) = match self.claim_next().await? {
            Some(claimed) => claimed,
            None => return Ok(false),
        };

        let handler = self.handlers.read().unwrap().get(&job_type).cloned();
        let result = match handler {
            Some(h) => h(&payload),
            None => Err(format!("no handler registered for job_type '{}'", job_type)),
        };

        match result {
            Ok(()) => self.mark_done(&id).await?,
            Err(e) => {
                // `max_retries` counts retries *after* the first attempt, so
                // `max_retries + 1` total attempts are allowed before giving
                // up — matching `JobQueue`'s semantics.
                let next_attempts = attempts + 1;
                if next_attempts > max_retries {
                    eprintln!(
                        "[jobs] persistent job '{}' ({}) failed permanently after {} attempt(s): {}",
                        job_type, id, next_attempts, e
                    );
                    self.mark_failed(&id, &e).await?;
                } else {
                    let initial_ms = self.initial_backoff_ms.load(Ordering::Relaxed);
                    let multiplier = self.backoff_multiplier.load(Ordering::Relaxed).max(1) as u64;
                    let delay = Duration::from_millis(
                        initial_ms.saturating_mul(multiplier.saturating_pow(attempts as u32)),
                    );
                    eprintln!(
                        "[jobs] persistent job '{}' ({}) failed (attempt {}/{}): {} — retrying in {:?}",
                        job_type, id, next_attempts, max_retries + 1, e, delay
                    );
                    self.reschedule(&id, next_attempts, &e, delay).await?;
                }
            }
        }
        Ok(true)
    }

    /// Spawn `workers` background OS threads, each running its own
    /// single-threaded Tokio runtime, polling for due jobs at
    /// [`poll_interval`](Self::poll_interval) (default: 500 ms) whenever the
    /// queue is empty. Returns the thread handles; call
    /// [`stop`](Self::stop) then join them for a graceful shutdown.
    pub fn start(self: Arc<Self>, workers: usize) -> Vec<thread::JoinHandle<()>> {
        assert!(workers > 0, "PersistentJobQueue requires at least one worker");
        (0..workers)
            .map(|_| {
                let queue = Arc::clone(&self);
                thread::spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("failed to start tokio runtime for PersistentJobQueue worker");
                    rt.block_on(async {
                        loop {
                            if queue.stop.load(Ordering::Relaxed) {
                                break;
                            }
                            let poll_interval = Duration::from_millis(queue.poll_interval_ms.load(Ordering::Relaxed));
                            match queue.tick().await {
                                Ok(true) => {}
                                Ok(false) => tokio::time::sleep(poll_interval).await,
                                Err(e) => {
                                    eprintln!("[jobs] persistent queue poll error: {}", e);
                                    tokio::time::sleep(poll_interval).await;
                                }
                            }
                        }
                    });
                })
            })
            .collect()
    }

    /// Signal all worker threads spawned by [`start`](Self::start) to stop
    /// after their current tick. Does not itself join the returned thread
    /// handles — join them separately to wait for a clean shutdown.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
