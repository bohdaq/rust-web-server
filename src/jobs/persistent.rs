//! [`PersistentJobQueue`]: jobs in a database table (`model-*` features).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::model::{Backend, DbError, DbPool, ModelRow, Value};
use crate::scheduler::CronSchedule;

type Handler = Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;
type DeadHook = Arc<dyn Fn(&JobInfo) + Send + Sync>;

/// How to enqueue one job (18.35.0): its queue, priority, when to run it,
/// a uniqueness key and its retry count.
///
/// ```rust
/// use std::time::Duration;
/// use rust_web_server::jobs::EnqueueOptions;
///
/// let options = EnqueueOptions::new()
///     .queue("mailers")
///     .priority(10)
///     .run_in(Duration::from_secs(60))
///     .unique("welcome:42")
///     .max_retries(5);
/// ```
#[derive(Debug, Clone, Default)]
pub struct EnqueueOptions {
    queue: Option<String>,
    priority: i64,
    run_at: Option<SystemTime>,
    unique_key: Option<String>,
    max_retries: Option<u32>,
}

impl EnqueueOptions {
    pub fn new() -> Self {
        EnqueueOptions::default()
    }

    /// The named queue (default `default`); workers can be limited to some
    /// queues with [`PersistentJobQueue::queues`].
    pub fn queue(mut self, name: &str) -> Self {
        self.queue = Some(name.to_string());
        self
    }

    /// Higher runs first among due jobs (default 0; negative is fine).
    pub fn priority(mut self, priority: i64) -> Self {
        self.priority = priority;
        self
    }

    /// Not before `at` (second precision).
    pub fn run_at(mut self, at: SystemTime) -> Self {
        self.run_at = Some(at);
        self
    }

    /// Not before `delay` from now.
    pub fn run_in(mut self, delay: Duration) -> Self {
        self.run_at = Some(SystemTime::now() + delay);
        self
    }

    /// Skip the job when one with the same type and `key` is already
    /// pending or running ([`enqueue_with`](PersistentJobQueue::enqueue_with)
    /// then answers that job's id). Checked when enqueuing: two enqueues
    /// in the same instant from different processes can both insert.
    pub fn unique(mut self, key: &str) -> Self {
        self.unique_key = Some(key.to_string());
        self
    }

    /// Retries after the first attempt (default: the queue's, 3).
    pub fn max_retries(mut self, n: u32) -> Self {
        self.max_retries = Some(n);
        self
    }
}

/// A job's row: what [`PersistentJobQueue::jobs`] lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobInfo {
    pub id: String,
    pub job_type: String,
    pub payload: String,
    pub queue: String,
    pub priority: i64,
    /// `pending`, `running` or `failed` (the dead-letter list).
    pub status: String,
    /// Attempts that failed so far.
    pub attempts: i64,
    pub max_retries: i64,
    /// Unix seconds: when it's due (pending) or was last due.
    pub run_at: i64,
    pub created_at: i64,
    pub last_error: Option<String>,
    pub unique_key: Option<String>,
}

impl JobInfo {
    fn from_row(row: &ModelRow) -> Result<JobInfo, DbError> {
        Ok(JobInfo {
            id: row.get("id")?,
            job_type: row.get("job_type")?,
            payload: row.get("payload")?,
            queue: row.get("queue")?,
            priority: row.get("priority")?,
            status: row.get("status")?,
            attempts: row.get("attempts")?,
            max_retries: row.get("max_retries")?,
            run_at: row.get("next_run_at")?,
            created_at: row.get("created_at")?,
            last_error: row.get("last_error")?,
            unique_key: row.get("unique_key")?,
        })
    }
}

const COLUMNS: &str = "id, job_type, payload, queue, priority, status, attempts, max_retries, next_run_at, created_at, last_error, unique_key";

/// A recurring job: enqueued by whichever replica first claims its due
/// time (see [`PersistentJobQueue::schedule`]).
struct Schedule {
    name: String,
    cron: CronSchedule,
    job_type: String,
    payload: String,
    options: EnqueueOptions,
}

/// A job queue backed by the model layer (`model-sqlite` / `model-postgres` /
/// `model-mysql`): jobs are written to a `rws_jobs` table before being
/// acknowledged, so pending and retrying jobs survive a process crash or
/// restart, and several replicas can share one queue.
///
/// Unlike [`JobQueue`](super::JobQueue), a job here can't be an arbitrary
/// closure — a closure can't be serialized to disk. Instead the queue stores
/// `(job_type, payload)` as plain text and dispatches to a handler
/// registered for that `job_type` at execution time. Register every handler
/// you enqueue *before* calling [`start`](Self::start), including after a
/// restart — unprocessed rows from a previous run (and any row left
/// `running` when the process crashed) are picked up automatically.
///
/// Since 18.35.0 (#93) a job can wait until a time, go to a named queue
/// with a priority, be unique ([`EnqueueOptions`]); a job that exhausts its
/// retries stays in the table as `failed` — the dead-letter list
/// ([`failed_jobs`](Self::failed_jobs), [`retry`](Self::retry),
/// [`on_dead`](Self::on_dead)); and [`schedule`](Self::schedule) runs a job
/// on a cron schedule once per due time across every replica.
///
/// ```sql
/// CREATE TABLE IF NOT EXISTS rws_jobs (
///     id          VARCHAR(64) PRIMARY KEY,
///     job_type    TEXT    NOT NULL,
///     payload     TEXT    NOT NULL DEFAULT '',
///     status      TEXT    NOT NULL DEFAULT 'pending', -- pending | running | failed
///     attempts    INTEGER NOT NULL DEFAULT 0,
///     max_retries INTEGER NOT NULL DEFAULT 3,
///     next_run_at BIGINT  NOT NULL,
///     created_at  BIGINT  NOT NULL,
///     last_error  TEXT,
///     queue       VARCHAR(64) NOT NULL DEFAULT 'default',
///     priority    BIGINT  NOT NULL DEFAULT 0,
///     unique_key  VARCHAR(191)
/// )
/// ```
///
/// A table from before 18.35.0 gets the last three columns added. Completed
/// jobs are deleted.
pub struct PersistentJobQueue {
    pool: DbPool,
    handlers: RwLock<HashMap<String, Handler>>,
    default_max_retries: AtomicU32,
    poll_interval_ms: AtomicU64,
    initial_backoff_ms: AtomicU64,
    backoff_multiplier: AtomicU32,
    stop: Arc<AtomicBool>,
    id_counter: AtomicU64,
    queues: RwLock<Vec<String>>,
    schedules: RwLock<Vec<Arc<Schedule>>>,
    on_dead: RwLock<Vec<DeadHook>>,
}

/// `sql` with `?` placeholders numbered `$1`, `$2`, … for PostgreSQL.
fn portable(backend: Backend, sql: &str) -> String {
    #[cfg(feature = "model-postgres")]
    if backend == Backend::Postgres {
        let mut out = String::with_capacity(sql.len() + 8);
        let mut n = 0;
        for c in sql.chars() {
            if c == '?' {
                n += 1;
                out.push_str(&format!("${n}"));
            } else {
                out.push(c);
            }
        }
        return out;
    }
    let _ = backend;
    sql.to_string()
}

fn now_epoch() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn epoch_of(at: SystemTime) -> i64 {
    at.duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

impl PersistentJobQueue {
    /// Creates the queue and its backing `rws_jobs` table if absent (adding
    /// the 18.35.0 columns to an older one), then resets any row left
    /// `running` (interrupted by a previous crash) back to `pending` so it
    /// gets picked up again.
    pub async fn new(pool: DbPool) -> Result<Self, DbError> {
        let queue = PersistentJobQueue {
            pool,
            handlers: RwLock::new(HashMap::new()),
            default_max_retries: AtomicU32::new(3),
            poll_interval_ms: AtomicU64::new(500),
            initial_backoff_ms: AtomicU64::new(500),
            backoff_multiplier: AtomicU32::new(2),
            stop: Arc::new(AtomicBool::new(false)),
            id_counter: AtomicU64::new(0),
            queues: RwLock::new(Vec::new()),
            schedules: RwLock::new(Vec::new()),
            on_dead: RwLock::new(Vec::new()),
        };
        queue.ensure_tables().await?;
        queue.recover_interrupted().await?;
        Ok(queue)
    }

    fn sql(&self, sql: &str) -> String {
        portable(self.pool.backend(), sql)
    }

    async fn ensure_tables(&self) -> Result<(), DbError> {
        self.pool
            .execute(
                "CREATE TABLE IF NOT EXISTS rws_jobs \
                 (id VARCHAR(64) PRIMARY KEY, job_type TEXT NOT NULL, payload TEXT NOT NULL, \
                  status VARCHAR(16) NOT NULL DEFAULT 'pending', attempts INTEGER NOT NULL DEFAULT 0, \
                  max_retries INTEGER NOT NULL DEFAULT 3, next_run_at BIGINT NOT NULL, \
                  created_at BIGINT NOT NULL, last_error TEXT, \
                  queue VARCHAR(64) NOT NULL DEFAULT 'default', priority BIGINT NOT NULL DEFAULT 0, \
                  unique_key VARCHAR(191))",
                &[],
            )
            .await?;
        // A table from before 18.35.0 lacks these.
        for (column, definition) in [
            ("queue", "VARCHAR(64) NOT NULL DEFAULT 'default'"),
            ("priority", "BIGINT NOT NULL DEFAULT 0"),
            ("unique_key", "VARCHAR(191)"),
        ] {
            if self.pool.query_rows(&format!("SELECT {column} FROM rws_jobs WHERE 1 = 0"), &[]).await.is_err() {
                self.pool.execute(&format!("ALTER TABLE rws_jobs ADD COLUMN {column} {definition}"), &[]).await?;
            }
        }
        self.pool
            .execute(
                "CREATE TABLE IF NOT EXISTS rws_job_schedules (name VARCHAR(191) PRIMARY KEY, next_run_at BIGINT NOT NULL)",
                &[],
            )
            .await?;
        Ok(())
    }

    async fn recover_interrupted(&self) -> Result<(), DbError> {
        self.pool.execute("UPDATE rws_jobs SET status = 'pending' WHERE status = 'running'", &[]).await?;
        Ok(())
    }

    /// Override the default retry count applied to jobs enqueued via
    /// [`enqueue`](Self::enqueue) (default: 3). Use
    /// [`EnqueueOptions::max_retries`] for a per-job override.
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

    /// Only run jobs from these named queues (default: every queue) — one
    /// process for `mailers`, another for the rest.
    pub fn queues(self, names: &[&str]) -> Self {
        *self.queues.write().unwrap_or_else(|e| e.into_inner()) = names.iter().map(|n| n.to_string()).collect();
        self
    }

    /// Enqueue `job_type` with `payload` on the cron schedule `cron` (six
    /// fields, seconds first, UTC; see [`Scheduler`](crate::scheduler::Scheduler)).
    /// Every replica can declare the same schedule: each due time is
    /// claimed by exactly one of them (a compare-and-set on the
    /// `rws_job_schedules` row named `name`), which enqueues the job.
    pub fn schedule(self, name: &str, cron: &str, job_type: &str, payload: &str) -> Result<Self, String> {
        self.schedule_with(name, cron, job_type, payload, EnqueueOptions::new())
    }

    /// [`schedule`](Self::schedule) with options for the enqueued jobs.
    pub fn schedule_with(self, name: &str, cron: &str, job_type: &str, payload: &str, options: EnqueueOptions) -> Result<Self, String> {
        let cron = CronSchedule::parse(cron)?;
        self.schedules.write().unwrap_or_else(|e| e.into_inner()).push(Arc::new(Schedule {
            name: name.to_string(),
            cron,
            job_type: job_type.to_string(),
            payload: payload.to_string(),
            options,
        }));
        Ok(self)
    }

    /// Call `hook` with each job that exhausts its retries, as it moves to
    /// the dead-letter list (`status = 'failed'`).
    pub fn on_dead(&self, hook: impl Fn(&JobInfo) + Send + Sync + 'static) {
        self.on_dead.write().unwrap_or_else(|e| e.into_inner()).push(Arc::new(hook));
    }

    /// Register the function that executes jobs of `job_type`. Call before
    /// [`start`](Self::start) — jobs enqueued for an unregistered `job_type`
    /// fail immediately (and retry, same as any other handler error).
    pub fn register(&self, job_type: &str, handler: impl Fn(&str) -> Result<(), String> + Send + Sync + 'static) {
        self.handlers.write().unwrap_or_else(|e| e.into_inner()).insert(job_type.to_string(), Arc::new(handler));
    }

    fn generate_id(&self) -> String {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
        let count = self.id_counter.fetch_add(1, Ordering::Relaxed);
        format!("{:x}-{:x}", nanos, count)
    }

    /// Persist a new job using the queue's defaults. Returns the job id.
    pub async fn enqueue(&self, job_type: &str, payload: &str) -> Result<String, DbError> {
        self.enqueue_with(job_type, payload, EnqueueOptions::new()).await
    }

    /// Persist a new job with a per-job retry override.
    pub async fn enqueue_with_retries(&self, job_type: &str, payload: &str, max_retries: u32) -> Result<String, DbError> {
        self.enqueue_with(job_type, payload, EnqueueOptions::new().max_retries(max_retries)).await
    }

    /// Persist a new job with `options` (18.35.0). Returns its id — for a
    /// [unique](EnqueueOptions::unique) job already pending or running, that
    /// job's id, and nothing is inserted.
    pub async fn enqueue_with(&self, job_type: &str, payload: &str, options: EnqueueOptions) -> Result<String, DbError> {
        if let Some(key) = &options.unique_key {
            let rows = self
                .pool
                .query_rows(
                    &self.sql("SELECT id FROM rws_jobs WHERE job_type = ? AND unique_key = ? AND status IN ('pending', 'running')"),
                    &[Value::Text(job_type.to_string()), Value::Text(key.clone())],
                )
                .await?;
            if let Some(row) = rows.first() {
                return row.get("id");
            }
        }
        let id = self.generate_id();
        let now = now_epoch();
        let max_retries = options.max_retries.unwrap_or_else(|| self.default_max_retries.load(Ordering::Relaxed));
        self.pool
            .execute(
                &self.sql(
                    "INSERT INTO rws_jobs (id, job_type, payload, status, attempts, max_retries, next_run_at, created_at, queue, priority, unique_key) \
                     VALUES (?, ?, ?, 'pending', 0, ?, ?, ?, ?, ?, ?)",
                ),
                &[
                    Value::Text(id.clone()),
                    Value::Text(job_type.to_string()),
                    Value::Text(payload.to_string()),
                    Value::Int(max_retries as i64),
                    Value::Int(options.run_at.map(epoch_of).unwrap_or(now)),
                    Value::Int(now),
                    Value::Text(options.queue.clone().unwrap_or_else(|| "default".to_string())),
                    Value::Int(options.priority),
                    options.unique_key.clone().map(Value::Text).unwrap_or(Value::Null),
                ],
            )
            .await?;
        Ok(id)
    }

    /// Atomically claims the due job with the highest priority (then the
    /// oldest due), marking it `running`. `None` if no job is due. The
    /// `UPDATE ... WHERE status = 'pending'` guards against two workers
    /// claiming the same row.
    async fn claim_next(&self) -> Result<Option<JobInfo>, DbError> {
        let queues = self.queues.read().unwrap_or_else(|e| e.into_inner()).clone();
        let mut params = vec![Value::Int(now_epoch())];
        let mut filter = String::new();
        if !queues.is_empty() {
            filter = format!(" AND queue IN ({})", vec!["?"; queues.len()].join(", "));
            params.extend(queues.into_iter().map(Value::Text));
        }
        let rows = self
            .pool
            .query_rows(
                &self.sql(&format!(
                    "SELECT {COLUMNS} FROM rws_jobs WHERE status = 'pending' AND next_run_at <= ?{filter} \
                     ORDER BY priority DESC, next_run_at ASC, created_at ASC LIMIT 1"
                )),
                &params,
            )
            .await?;
        let Some(row) = rows.into_iter().next() else {
            return Ok(None);
        };
        let job = JobInfo::from_row(&row)?;
        let claimed = self
            .pool
            .execute(&self.sql("UPDATE rws_jobs SET status = 'running' WHERE id = ? AND status = 'pending'"), &[Value::Text(job.id.clone())])
            .await?;
        if claimed == 0 {
            return Ok(None); // another worker won the race
        }
        Ok(Some(job))
    }

    async fn mark_done(&self, id: &str) -> Result<(), DbError> {
        self.pool.execute(&self.sql("DELETE FROM rws_jobs WHERE id = ?"), &[Value::Text(id.to_string())]).await?;
        Ok(())
    }

    async fn mark_failed(&self, job: &JobInfo, attempts: i64, error: &str) -> Result<(), DbError> {
        self.pool
            .execute(
                &self.sql("UPDATE rws_jobs SET status = 'failed', attempts = ?, last_error = ? WHERE id = ?"),
                &[Value::Int(attempts), Value::Text(error.to_string()), Value::Text(job.id.clone())],
            )
            .await?;
        let dead = JobInfo { status: "failed".into(), attempts, last_error: Some(error.to_string()), ..job.clone() };
        let hooks = self.on_dead.read().unwrap_or_else(|e| e.into_inner()).clone();
        for hook in hooks {
            hook(&dead);
        }
        Ok(())
    }

    async fn reschedule(&self, id: &str, attempts: i64, error: &str, delay: Duration) -> Result<(), DbError> {
        let next_run_at = now_epoch() + delay.as_secs() as i64;
        self.pool
            .execute(
                &self.sql("UPDATE rws_jobs SET status = 'pending', attempts = ?, last_error = ?, next_run_at = ? WHERE id = ?"),
                &[Value::Int(attempts), Value::Text(error.to_string()), Value::Int(next_run_at), Value::Text(id.to_string())],
            )
            .await?;
        Ok(())
    }

    /// Run one poll-claim-execute cycle. Returns `Ok(true)` if a job was
    /// claimed (whether it succeeded, failed, or is retrying), `Ok(false)` if
    /// none was due. Exposed for tests and for embedding in a caller-owned
    /// loop; most applications should use [`start`](Self::start) instead.
    pub async fn tick(&self) -> Result<bool, DbError> {
        let Some(job) = self.claim_next().await? else {
            return Ok(false);
        };

        let handler = self.handlers.read().unwrap_or_else(|e| e.into_inner()).get(&job.job_type).cloned();
        let result = match handler {
            Some(h) => h(&job.payload),
            None => Err(format!("no handler registered for job_type '{}'", job.job_type)),
        };

        match result {
            Ok(()) => self.mark_done(&job.id).await?,
            Err(e) => {
                // `max_retries` counts retries *after* the first attempt, so
                // `max_retries + 1` total attempts are allowed before giving
                // up — matching `JobQueue`'s semantics.
                let next_attempts = job.attempts + 1;
                if next_attempts > job.max_retries {
                    eprintln!(
                        "[jobs] persistent job '{}' ({}) failed permanently after {} attempt(s): {}",
                        job.job_type, job.id, next_attempts, e
                    );
                    self.mark_failed(&job, next_attempts, &e).await?;
                } else {
                    let initial_ms = self.initial_backoff_ms.load(Ordering::Relaxed);
                    let multiplier = self.backoff_multiplier.load(Ordering::Relaxed).max(1) as u64;
                    let delay = Duration::from_millis(initial_ms.saturating_mul(multiplier.saturating_pow(job.attempts as u32)));
                    eprintln!(
                        "[jobs] persistent job '{}' ({}) failed (attempt {}/{}): {} — retrying in {:?}",
                        job.job_type,
                        job.id,
                        next_attempts,
                        job.max_retries + 1,
                        e,
                        delay
                    );
                    self.reschedule(&job.id, next_attempts, &e, delay).await?;
                }
            }
        }
        Ok(true)
    }

    /// Enqueue the scheduled jobs that are due, each due time once across
    /// every process sharing the database. Returns how many this call
    /// enqueued. [`start`](Self::start)'s first worker calls it about once a
    /// second; exposed for tests and caller-owned loops.
    pub async fn tick_schedules(&self) -> Result<usize, DbError> {
        self.tick_schedules_at(now_epoch()).await
    }

    async fn tick_schedules_at(&self, now: i64) -> Result<usize, DbError> {
        let schedules = self.schedules.read().unwrap_or_else(|e| e.into_inner()).clone();
        let mut enqueued = 0;
        for schedule in schedules {
            let next_after = |t: i64| schedule.cron.next_after(t.max(0) as u64).map(|n| n as i64);
            let rows = self
                .pool
                .query_rows(&self.sql("SELECT next_run_at FROM rws_job_schedules WHERE name = ?"), &[Value::Text(schedule.name.clone())])
                .await?;
            let Some(row) = rows.first() else {
                // First sight of this schedule: its next due time. Another
                // replica may insert it at the same moment; either row is right.
                if let Some(next) = next_after(now) {
                    let _ = self
                        .pool
                        .execute(&self.sql("INSERT INTO rws_job_schedules (name, next_run_at) VALUES (?, ?)"), &[Value::Text(schedule.name.clone()), Value::Int(next)])
                        .await;
                }
                continue;
            };
            let due: i64 = row.get("next_run_at")?;
            if due > now {
                continue;
            }
            // Missed runs (everything was down) collapse into this one.
            let Some(next) = next_after(now) else { continue };
            let claimed = self
                .pool
                .execute(
                    &self.sql("UPDATE rws_job_schedules SET next_run_at = ? WHERE name = ? AND next_run_at = ?"),
                    &[Value::Int(next), Value::Text(schedule.name.clone()), Value::Int(due)],
                )
                .await?;
            if claimed == 1 {
                self.enqueue_with(&schedule.job_type, &schedule.payload, schedule.options.clone()).await?;
                enqueued += 1;
            }
        }
        Ok(enqueued)
    }

    /// Jobs with `status` (`pending`, `running`, `failed`; `None`: all),
    /// highest priority and earliest due first, at most `limit`.
    pub async fn jobs(&self, status: Option<&str>, limit: usize) -> Result<Vec<JobInfo>, DbError> {
        let (filter, params) = match status {
            Some(status) => (" WHERE status = ?", vec![Value::Text(status.to_string())]),
            None => ("", Vec::new()),
        };
        let rows = self
            .pool
            .query_rows(&self.sql(&format!("SELECT {COLUMNS} FROM rws_jobs{filter} ORDER BY priority DESC, next_run_at ASC, created_at ASC LIMIT {limit}")), &params)
            .await?;
        rows.iter().map(JobInfo::from_row).collect()
    }

    /// The dead-letter list: jobs that exhausted their retries.
    pub async fn failed_jobs(&self, limit: usize) -> Result<Vec<JobInfo>, DbError> {
        self.jobs(Some("failed"), limit).await
    }

    /// Put a failed job back in the queue, due now, with its attempts reset.
    /// `false` when there's no failed job `id`.
    pub async fn retry(&self, id: &str) -> Result<bool, DbError> {
        let changed = self
            .pool
            .execute(
                &self.sql("UPDATE rws_jobs SET status = 'pending', attempts = 0, next_run_at = ? WHERE id = ? AND status = 'failed'"),
                &[Value::Int(now_epoch()), Value::Text(id.to_string())],
            )
            .await?;
        Ok(changed > 0)
    }

    /// Delete a job that isn't running. `false` when there's no such job.
    pub async fn delete(&self, id: &str) -> Result<bool, DbError> {
        let changed = self
            .pool
            .execute(&self.sql("DELETE FROM rws_jobs WHERE id = ? AND status <> 'running'"), &[Value::Text(id.to_string())])
            .await?;
        Ok(changed > 0)
    }

    /// Spawn `workers` background OS threads, each running its own
    /// single-threaded Tokio runtime, polling for due jobs at
    /// [`poll_interval`](Self::poll_interval) (default: 500 ms) whenever the
    /// queue is empty; the first also enqueues [scheduled](Self::schedule)
    /// jobs. Returns the thread handles; call [`stop`](Self::stop) then join
    /// them for a graceful shutdown.
    pub fn start(self: Arc<Self>, workers: usize) -> Vec<thread::JoinHandle<()>> {
        assert!(workers > 0, "PersistentJobQueue requires at least one worker");
        (0..workers)
            .map(|index| {
                let queue = Arc::clone(&self);
                thread::spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("failed to start tokio runtime for PersistentJobQueue worker");
                    rt.block_on(async {
                        let mut last_schedule_check = 0;
                        loop {
                            if queue.stop.load(Ordering::Relaxed) {
                                break;
                            }
                            if index == 0 && now_epoch() != last_schedule_check {
                                last_schedule_check = now_epoch();
                                if let Err(e) = queue.tick_schedules().await {
                                    eprintln!("[jobs] schedule check error: {}", e);
                                }
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

    /// The database the queue uses.
    pub fn pool(&self) -> &DbPool {
        &self.pool
    }
}

#[cfg(test)]
mod tests;
