//! Postgres: the ORM routes (`todos`) and the job queue (`e2e_jobs` →
//! `job_runs`).
//!
//! The queue is this app's own: rws's `PersistentJobQueue` writes `?`
//! placeholders, which Postgres rejects (see README "Findings"). It follows
//! the same retry policy as rws's queues: 3 retries after the first attempt,
//! 500 ms initial backoff, doubled each time. A job is claimed with
//! `FOR UPDATE SKIP LOCKED`, and its `job_runs` row is written in the same
//! transaction that finishes the job, so with any number of replicas every
//! job runs (successfully) exactly once.

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use rust_web_server::model::{DbConfig, DbPool, Repository, Value};
use rust_web_server::Model;

use crate::common::{block, env, runtime};

pub const MAX_RETRIES: i64 = 3;
pub const INITIAL_BACKOFF_MS: i64 = 500;
pub const BACKOFF_MULTIPLIER: i64 = 2;
/// A job left `running` this long (its pod died) is handed out again.
pub const LEASE_SECS: i64 = 60;

#[derive(Model, Debug, Clone)]
#[table(name = "todos")]
pub struct Todo {
    #[primary_key(auto_increment)]
    pub id: i64,
    pub title: String,
}

#[derive(Clone)]
pub enum DbStatus {
    /// `RWS_DB_NAME` (or another required variable) is unset.
    Missing(String),
    Connecting,
    Ready(DbPool),
    Failed(String),
}

#[derive(Clone)]
pub struct Db {
    status: Arc<RwLock<DbStatus>>,
}

impl Db {
    pub fn missing(what: &str) -> Db {
        Db { status: Arc::new(RwLock::new(DbStatus::Missing(what.to_string()))) }
    }

    pub fn status(&self) -> DbStatus {
        self.status.read().unwrap().clone()
    }

    /// The pool, or the text of the `503` to answer with.
    pub fn pool(&self) -> Result<DbPool, String> {
        match self.status() {
            DbStatus::Ready(pool) => Ok(pool),
            DbStatus::Missing(what) => Err(format!("database not configured ({what} unset)")),
            DbStatus::Connecting => Err("database still connecting".to_string()),
            DbStatus::Failed(e) => Err(format!("database unavailable: {e}")),
        }
    }

    /// From `RWS_DB_*`: connects in the background (retrying for up to 60 s),
    /// runs the migrations in `migrations_dir`, then starts the job workers.
    pub fn start(migrations_dir: String, pod: String) -> Db {
        if env("RWS_DB_NAME").is_none() {
            return Db::missing("RWS_DB_NAME");
        }
        if std::env::var("RWS_DB_BACKEND").is_err() {
            std::env::set_var("RWS_DB_BACKEND", "postgres");
        }
        let db = Db { status: Arc::new(RwLock::new(DbStatus::Connecting)) };
        let status = Arc::clone(&db.status);
        runtime().spawn(async move {
            let result = connect_and_migrate(&migrations_dir).await;
            match result {
                Ok(pool) => {
                    eprintln!("[e2e] database ready, migrations applied from {migrations_dir}");
                    *status.write().unwrap() = DbStatus::Ready(pool.clone());
                    for worker in 0..2 {
                        runtime().spawn(job_worker(pool.clone(), format!("{pod}#{worker}"), pod.clone()));
                    }
                }
                Err(e) => {
                    eprintln!("[e2e] database unavailable: {e}");
                    *status.write().unwrap() = DbStatus::Failed(e);
                }
            }
        });
        db
    }
}

async fn connect_and_migrate(dir: &str) -> Result<DbPool, String> {
    let config = DbConfig::from_env().map_err(|e| e.to_string())?;
    // RWS_DB_SSL_*: a managed PostgreSQL requires TLS from its own CA.
    let tls = rust_web_server::model::DbTls::from_env().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(60);
    let pool = loop {
        match DbPool::new_with_tls(config.clone(), tls.clone()).await {
            Ok(pool) => break pool,
            Err(e) if Instant::now() < deadline => {
                eprintln!("[e2e] database connect failed, retrying: {e}");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => return Err(format!("connect: {e}")),
        }
    };
    // Replicas starting together race in rws's migration runner (no lock):
    // the loser's transaction fails, and the next attempt finds the
    // migration applied.
    let mut last = String::new();
    for _ in 0..10 {
        match pool.migrate(dir).await {
            Ok(()) => return Ok(pool),
            Err(e) => {
                last = e.to_string();
                eprintln!("[e2e] migrate failed, retrying: {last}");
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
    Err(format!("migrate: {last}"))
}

// ── todos ────────────────────────────────────────────────────────────────────

pub fn create_todo(pool: DbPool, title: String) -> Result<Todo, String> {
    block(async move { Todo::repository(&pool).save(&Todo { id: 0, title }).await.map_err(|e| e.to_string()) })
}

pub fn find_todo(pool: DbPool, id: i64) -> Result<Option<Todo>, String> {
    block(async move { Todo::repository(&pool).find_by_id(id).await.map_err(|e| e.to_string()) })
}

pub fn delete_todo(pool: DbPool, id: i64) -> Result<(), String> {
    block(async move { Todo::repository(&pool).delete_by_id(id).await.map_err(|e| e.to_string()) })
}

// ── jobs ─────────────────────────────────────────────────────────────────────

pub fn enqueue(pool: DbPool, key: String, fail_first: bool) -> Result<i64, String> {
    block(async move {
        let rows = pool
            .query_rows(
                "INSERT INTO e2e_jobs (job_key, fail_first, max_retries) VALUES ($1, $2, $3) RETURNING id",
                &[Value::Text(key), Value::Bool(fail_first), Value::Int(MAX_RETRIES)],
            )
            .await
            .map_err(|e| e.to_string())?;
        rows.first().ok_or("no id returned")?.get::<i64>("id").map_err(|e| e.to_string())
    })
}

/// `{"runs":[{"pod","attempts","at"}], "pending": n, "failed": n}` for `key`.
pub fn runs(pool: DbPool, key: String) -> Result<serde_json::Value, String> {
    block(async move {
        let rows = pool
            .query_rows(
                "SELECT pod, attempts, (extract(epoch from at) * 1000)::bigint AS at_ms FROM job_runs WHERE job_key = $1 ORDER BY id",
                &[Value::Text(key.clone())],
            )
            .await
            .map_err(|e| e.to_string())?;
        let mut runs = Vec::new();
        for row in rows {
            runs.push(serde_json::json!({
                "pod": row.get::<String>("pod").map_err(|e| e.to_string())?,
                "attempts": row.get::<i64>("attempts").map_err(|e| e.to_string())?,
                "at_ms": row.get::<i64>("at_ms").map_err(|e| e.to_string())?,
            }));
        }
        let counts = pool
            .query_rows(
                "SELECT count(*) FILTER (WHERE status IN ('pending','running')) AS pending, \
                        count(*) FILTER (WHERE status = 'failed') AS failed \
                 FROM e2e_jobs WHERE job_key = $1",
                &[Value::Text(key)],
            )
            .await
            .map_err(|e| e.to_string())?;
        let (pending, failed) = match counts.first() {
            Some(r) => (r.get::<i64>("pending").unwrap_or(0), r.get::<i64>("failed").unwrap_or(0)),
            None => (0, 0),
        };
        Ok(serde_json::json!({ "runs": runs, "pending": pending, "failed": failed }))
    })
}

/// What a job does: fail the first attempt when asked to, else record a run.
fn attempt_fails(fail_first: bool, attempt: i64) -> bool {
    fail_first && attempt == 1
}

pub fn backoff_ms(attempts_so_far: i64) -> i64 {
    INITIAL_BACKOFF_MS.saturating_mul(BACKOFF_MULTIPLIER.saturating_pow(attempts_so_far.max(0) as u32))
}

async fn job_worker(pool: DbPool, worker: String, pod: String) {
    loop {
        match tick(&pool, &worker, &pod).await {
            Ok(true) => {}
            Ok(false) => tokio::time::sleep(Duration::from_millis(200)).await,
            Err(e) => {
                eprintln!("[e2e] job worker {worker}: {e}");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

/// Claim one due job and run it. `Ok(false)` when none was due.
async fn tick(pool: &DbPool, worker: &str, pod: &str) -> Result<bool, String> {
    let e = |e: rust_web_server::model::DbError| e.to_string();
    let claimed = pool
        .query_rows(
            "UPDATE e2e_jobs SET status = 'running', locked_by = $1, locked_at = now(), attempts = attempts + 1 \
             WHERE id = (SELECT id FROM e2e_jobs \
                         WHERE (status = 'pending' AND next_run_at <= now()) \
                            OR (status = 'running' AND locked_at < now() - ($2::bigint * interval '1 second')) \
                         ORDER BY id LIMIT 1 FOR UPDATE SKIP LOCKED) \
             RETURNING id, job_key, fail_first, attempts, max_retries",
            &[Value::Text(worker.to_string()), Value::Int(LEASE_SECS)],
        )
        .await
        .map_err(e)?;
    let Some(row) = claimed.into_iter().next() else { return Ok(false) };
    let id: i64 = row.get("id").map_err(e)?;
    let key: String = row.get("job_key").map_err(e)?;
    let fail_first: bool = row.get("fail_first").map_err(e)?;
    let attempt: i64 = row.get("attempts").map_err(e)?;
    let max_retries: i64 = row.get("max_retries").map_err(e)?;

    if attempt_fails(fail_first, attempt) {
        let error = format!("attempt {attempt} fails on purpose (fail_first=1)");
        if attempt > max_retries {
            pool.execute(
                "UPDATE e2e_jobs SET status = 'failed', last_error = $1 WHERE id = $2 AND locked_by = $3",
                &[Value::Text(error), Value::Int(id), Value::Text(worker.to_string())],
            )
            .await
            .map_err(e)?;
        } else {
            let delay = backoff_ms(attempt - 1);
            eprintln!("[e2e] job {id} ({key}) failed attempt {attempt}/{}: retrying in {delay} ms", max_retries + 1);
            pool.execute(
                "UPDATE e2e_jobs SET status = 'pending', last_error = $1, \
                 next_run_at = now() + ($2::bigint * interval '1 millisecond') WHERE id = $3 AND locked_by = $4",
                &[Value::Text(error), Value::Int(delay), Value::Int(id), Value::Text(worker.to_string())],
            )
            .await
            .map_err(e)?;
        }
        return Ok(true);
    }

    // Success: the run row and the job's removal commit together, and only
    // while this worker still holds the job.
    let mut tx = pool.begin().await.map_err(e)?;
    let done = tx
        .execute(
            "DELETE FROM e2e_jobs WHERE id = $1 AND locked_by = $2 AND status = 'running'",
            &[Value::Int(id), Value::Text(worker.to_string())],
        )
        .await
        .map_err(e)?;
    if done == 1 {
        tx.execute(
            "INSERT INTO job_runs (job_key, pod, attempts) VALUES ($1, $2, $3)",
            &[Value::Text(key), Value::Text(pod.to_string()), Value::Int(attempt)],
        )
        .await
        .map_err(e)?;
        tx.commit().await.map_err(e)?;
    } else {
        tx.rollback().await.map_err(e)?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_policy_matches_rws_defaults() {
        assert_eq!(500, backoff_ms(0));
        assert_eq!(1000, backoff_ms(1));
        assert_eq!(2000, backoff_ms(2));
        assert!(attempt_fails(true, 1));
        assert!(!attempt_fails(true, 2));
        assert!(!attempt_fails(false, 1));
    }

    #[test]
    fn missing_database_names_the_variable() {
        let db = Db::missing("RWS_DB_NAME");
        assert!(db.pool().err().unwrap().contains("RWS_DB_NAME"));
    }
}
