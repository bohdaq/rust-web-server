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

#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
mod persistent;
#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
pub use persistent::{EnqueueOptions, JobInfo, PersistentJobQueue};
