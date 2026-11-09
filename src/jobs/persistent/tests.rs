//! The 18.35.0 queue features, on SQLite in memory (`model-sqlite`), and on
//! PostgreSQL / MySQL servers when their test URLs are set.
#![cfg(feature = "model-sqlite")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use super::*;

async fn queue() -> PersistentJobQueue {
    PersistentJobQueue::new(DbPool::memory().await.unwrap()).await.unwrap()
}

/// Registers `job_type` and records each payload it runs.
fn recorder(queue: &PersistentJobQueue, job_type: &str) -> Arc<Mutex<Vec<String>>> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s = Arc::clone(&seen);
    queue.register(job_type, move |payload| {
        s.lock().unwrap().push(payload.to_string());
        Ok(())
    });
    seen
}

#[tokio::test]
async fn future_jobs_wait_until_due() {
    let q = queue().await;
    let seen = recorder(&q, "t");
    q.enqueue_with("t", "later", EnqueueOptions::new().run_in(Duration::from_secs(3600))).await.unwrap();
    q.enqueue_with("t", "past", EnqueueOptions::new().run_at(SystemTime::now() - Duration::from_secs(10))).await.unwrap();
    assert!(q.tick().await.unwrap());
    assert!(!q.tick().await.unwrap());
    assert_eq!(vec!["past"], *seen.lock().unwrap());
    let pending = q.jobs(Some("pending"), 10).await.unwrap();
    assert_eq!(1, pending.len());
    assert!(pending[0].run_at > now_epoch() + 3000);
}

#[tokio::test]
async fn higher_priority_runs_first() {
    let q = queue().await;
    let seen = recorder(&q, "t");
    q.enqueue("t", "normal").await.unwrap();
    q.enqueue_with("t", "low", EnqueueOptions::new().priority(-5)).await.unwrap();
    q.enqueue_with("t", "urgent", EnqueueOptions::new().priority(10)).await.unwrap();
    while q.tick().await.unwrap() {}
    assert_eq!(vec!["urgent", "normal", "low"], *seen.lock().unwrap());
}

#[tokio::test]
async fn workers_take_only_their_queues() {
    let pool = DbPool::memory().await.unwrap();
    let mail = PersistentJobQueue::new(pool.clone()).await.unwrap().queues(&["mailers"]);
    let seen = recorder(&mail, "t");
    mail.enqueue("t", "default-queue").await.unwrap();
    mail.enqueue_with("t", "mail", EnqueueOptions::new().queue("mailers")).await.unwrap();
    while mail.tick().await.unwrap() {}
    assert_eq!(vec!["mail"], *seen.lock().unwrap());
    let rest = mail.jobs(None, 10).await.unwrap();
    assert_eq!(("default", "default-queue"), (rest[0].queue.as_str(), rest[0].payload.as_str()));
}

#[tokio::test]
async fn unique_jobs_are_enqueued_once_while_pending() {
    let q = queue().await;
    let seen = recorder(&q, "welcome");
    let a = q.enqueue_with("welcome", "1", EnqueueOptions::new().unique("user:1")).await.unwrap();
    let b = q.enqueue_with("welcome", "1 again", EnqueueOptions::new().unique("user:1")).await.unwrap();
    let other = q.enqueue_with("welcome", "2", EnqueueOptions::new().unique("user:2")).await.unwrap();
    assert_eq!(a, b);
    assert_ne!(a, other);
    while q.tick().await.unwrap() {}
    assert_eq!(vec!["1", "2"], *seen.lock().unwrap());
    // Done, so the key is free again.
    let c = q.enqueue_with("welcome", "1 later", EnqueueOptions::new().unique("user:1")).await.unwrap();
    assert_ne!(a, c);
}

#[tokio::test]
async fn exhausted_jobs_go_to_the_dead_letter_list() {
    let q = queue().await.backoff(Duration::from_millis(1), 1);
    let dead = Arc::new(Mutex::new(Vec::new()));
    let d = Arc::clone(&dead);
    q.on_dead(move |job| d.lock().unwrap().push((job.id.clone(), job.attempts, job.last_error.clone())));
    let fail = Arc::new(AtomicUsize::new(0));
    let f = Arc::clone(&fail);
    q.register("boom", move |_| {
        f.fetch_add(1, Ordering::SeqCst);
        Err("nope".into())
    });
    let id = q.enqueue_with("boom", "x", EnqueueOptions::new().max_retries(1)).await.unwrap();
    while q.tick().await.unwrap() {}
    assert_eq!(2, fail.load(Ordering::SeqCst));
    assert_eq!(vec![(id.clone(), 2, Some("nope".to_string()))], *dead.lock().unwrap());
    let failed = q.failed_jobs(10).await.unwrap();
    assert_eq!((id.as_str(), "failed", 2), (failed[0].id.as_str(), failed[0].status.as_str(), failed[0].attempts));

    // Retry puts it back with its attempts reset.
    assert!(q.retry(&id).await.unwrap());
    assert!(!q.retry(&id).await.unwrap(), "only failed jobs are retried");
    assert_eq!(0, q.jobs(Some("pending"), 10).await.unwrap()[0].attempts);
    assert!(q.delete(&id).await.unwrap());
    assert!(!q.delete(&id).await.unwrap());
    assert!(q.jobs(None, 10).await.unwrap().is_empty());
}

#[tokio::test]
async fn each_scheduled_time_is_claimed_by_one_replica() {
    let pool = DbPool::memory().await.unwrap();
    let a = PersistentJobQueue::new(pool.clone()).await.unwrap().schedule("report", "0 0 3 * * *", "report", "daily").unwrap();
    let b = PersistentJobQueue::new(pool.clone()).await.unwrap().schedule("report", "0 0 3 * * *", "report", "daily").unwrap();
    // 2026-11-10T12:34:56Z: the first sight records the next 03:00.
    let now = 1_794_314_096;
    assert_eq!(0, a.tick_schedules_at(now).await.unwrap());
    assert_eq!(0, b.tick_schedules_at(now).await.unwrap());
    let next_3am = now + (14 * 3600 + 25 * 60 + 4);
    assert_eq!(0, a.tick_schedules_at(next_3am - 1).await.unwrap());
    // Due: exactly one of the two enqueues it.
    let total = a.tick_schedules_at(next_3am).await.unwrap() + b.tick_schedules_at(next_3am).await.unwrap();
    assert_eq!(1, total);
    assert_eq!(1, a.jobs(Some("pending"), 10).await.unwrap().len());
    // A day later (and after missed runs) it fires once more.
    assert_eq!(1, b.tick_schedules_at(next_3am + 3 * 86_400).await.unwrap());
    assert_eq!(0, a.tick_schedules_at(next_3am + 3 * 86_400).await.unwrap());
    assert!(PersistentJobQueue::new(pool).await.unwrap().schedule("bad", "not cron", "x", "").is_err());
}

#[tokio::test]
async fn an_old_table_gets_the_new_columns() {
    let pool = DbPool::memory().await.unwrap();
    pool.execute(
        "CREATE TABLE rws_jobs (id TEXT PRIMARY KEY, job_type TEXT NOT NULL, payload TEXT NOT NULL DEFAULT '', \
         status TEXT NOT NULL DEFAULT 'pending', attempts INTEGER NOT NULL DEFAULT 0, max_retries INTEGER NOT NULL DEFAULT 3, \
         next_run_at INTEGER NOT NULL, created_at INTEGER NOT NULL, last_error TEXT)",
        &[],
    )
    .await
    .unwrap();
    pool.execute("INSERT INTO rws_jobs (id, job_type, payload, next_run_at, created_at) VALUES ('old', 't', 'from 18.34', 0, 0)", &[]).await.unwrap();
    let q = PersistentJobQueue::new(pool.clone()).await.unwrap();
    let seen = recorder(&q, "t");
    assert!(q.tick().await.unwrap());
    assert_eq!(vec!["from 18.34"], *seen.lock().unwrap());
    // And again: the columns are there now.
    PersistentJobQueue::new(pool).await.unwrap();
}

#[test]
fn placeholders_are_numbered_only_for_postgres() {
    assert_eq!("a = ? AND b = ?", portable(Backend::Sqlite, "a = ? AND b = ?"));
    #[cfg(feature = "model-postgres")]
    assert_eq!("a = $1 AND b = $2", portable(Backend::Postgres, "a = ? AND b = ?"));
}

/// The queue on PostgreSQL and MySQL when `RWS_TEST_POSTGRES_URL` /
/// `RWS_TEST_MYSQL_URL` name a server (CI's `model-databases` job); the
/// only test that uses those servers' `rws_jobs` tables, so it drops them
/// first and last.
#[cfg(any(feature = "model-postgres", feature = "model-mysql"))]
#[tokio::test]
async fn the_queue_on_real_servers() {
    let mut urls = Vec::new();
    #[cfg(feature = "model-postgres")]
    urls.push(("postgres", std::env::var("RWS_TEST_POSTGRES_URL").ok()));
    #[cfg(feature = "model-mysql")]
    urls.push(("mysql", std::env::var("RWS_TEST_MYSQL_URL").ok()));
    let required = std::env::var("RWS_TEST_REQUIRE_BACKENDS").unwrap_or_default();
    for (name, url) in urls {
        let Some(url) = url else {
            assert!(!required.contains(name), "RWS_TEST_REQUIRE_BACKENDS has {name} but its URL isn't set");
            eprintln!("skipped on {name}: no RWS_TEST_{}_URL", name.to_uppercase());
            continue;
        };
        let pool = DbPool::connect(&url).await.unwrap();
        for table in ["rws_jobs", "rws_job_schedules"] {
            pool.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]).await.unwrap();
        }
        let q = PersistentJobQueue::new(pool.clone()).await.unwrap().backoff(Duration::from_millis(1), 1).queues(&["default", "mail"]);
        let seen = recorder(&q, "t");
        q.enqueue("t", "a").await.unwrap();
        q.enqueue_with("t", "b", EnqueueOptions::new().priority(5).queue("mail")).await.unwrap();
        q.enqueue_with("t", "never", EnqueueOptions::new().queue("other")).await.unwrap();
        let u1 = q.enqueue_with("t", "u", EnqueueOptions::new().unique("k").run_in(Duration::from_secs(3600))).await.unwrap();
        assert_eq!(u1, q.enqueue_with("t", "u", EnqueueOptions::new().unique("k")).await.unwrap(), "{name}");
        while q.tick().await.unwrap() {}
        assert_eq!(vec!["b", "a"], *seen.lock().unwrap(), "{name}");
        q.register("boom", |_| Err("x".into()));
        let dead = q.enqueue_with("boom", "", EnqueueOptions::new().max_retries(0)).await.unwrap();
        while q.tick().await.unwrap() {}
        assert_eq!(dead, q.failed_jobs(5).await.unwrap()[0].id, "{name}");
        assert!(q.retry(&dead).await.unwrap() && q.delete(&dead).await.unwrap(), "{name}");
        let s = PersistentJobQueue::new(pool.clone()).await.unwrap().schedule("r", "0 0 3 * * *", "t", "s").unwrap();
        let now = 1_794_314_096;
        assert_eq!(0, s.tick_schedules_at(now).await.unwrap(), "{name}");
        assert_eq!(1, s.tick_schedules_at(now + 52_000).await.unwrap(), "{name}");
        assert_eq!(0, s.tick_schedules_at(now + 52_000).await.unwrap(), "{name}");
        for table in ["rws_jobs", "rws_job_schedules"] {
            pool.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]).await.unwrap();
        }
    }
}
