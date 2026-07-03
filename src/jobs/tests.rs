//! Unit tests for `JobQueue`. `PersistentJobQueue` tests additionally require
//! `model-sqlite`:
//! ```bash
//! cargo test --no-default-features --features jobs,model-sqlite -- jobs
//! ```

use crate::jobs::JobQueue;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[test]
fn job_runs_successfully() {
    let queue = JobQueue::new(2);
    let count = Arc::new(AtomicUsize::new(0));

    let count2 = Arc::clone(&count);
    queue.submit(move || {
        count2.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });

    queue.join();
    assert_eq!(1, count.load(Ordering::SeqCst));
}

#[test]
fn job_retries_until_success() {
    let queue = JobQueue::new(1).backoff(Duration::from_millis(1), 1);
    let attempts = Arc::new(AtomicUsize::new(0));

    let attempts2 = Arc::clone(&attempts);
    queue.submit(move || {
        let n = attempts2.fetch_add(1, Ordering::SeqCst) + 1;
        if n < 3 {
            Err(format!("attempt {n} failed"))
        } else {
            Ok(())
        }
    });

    queue.join();
    assert_eq!(3, attempts.load(Ordering::SeqCst));
}

#[test]
fn job_stops_after_max_retries() {
    let queue = JobQueue::new(1).max_retries(2).backoff(Duration::from_millis(1), 1);
    let attempts = Arc::new(AtomicUsize::new(0));

    let attempts2 = Arc::clone(&attempts);
    queue.submit(move || {
        attempts2.fetch_add(1, Ordering::SeqCst);
        Err("always fails".to_string())
    });

    queue.join();
    // 1 initial attempt + 2 retries = 3 total, then it gives up.
    assert_eq!(3, attempts.load(Ordering::SeqCst));
}

#[test]
fn multiple_jobs_all_run() {
    let queue = JobQueue::new(4);
    let count = Arc::new(AtomicUsize::new(0));

    for _ in 0..20 {
        let count2 = Arc::clone(&count);
        queue.submit(move || {
            count2.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
    }

    queue.join();
    assert_eq!(20, count.load(Ordering::SeqCst));
}

#[test]
fn named_job_struct_implements_job_trait() {
    use crate::jobs::Job;

    struct RecordingJob {
        log: Arc<Mutex<Vec<String>>>,
    }

    impl Job for RecordingJob {
        fn run(&self) -> Result<(), String> {
            self.log.lock().unwrap().push("ran".to_string());
            Ok(())
        }

        fn name(&self) -> &str {
            "recording-job"
        }
    }

    let queue = JobQueue::new(1);
    let log = Arc::new(Mutex::new(Vec::new()));
    queue.submit(RecordingJob { log: Arc::clone(&log) });
    queue.join();

    assert_eq!(vec!["ran".to_string()], *log.lock().unwrap());
}

// ── PersistentJobQueue ───────────────────────────────────────────────────────

#[cfg(all(test, feature = "model-sqlite"))]
mod persistent_tests {
    use crate::jobs::PersistentJobQueue;
    use crate::model::DbPool;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    async fn test_queue() -> PersistentJobQueue {
        let pool = DbPool::memory().await.expect("open :memory: pool");
        PersistentJobQueue::new(pool).await.expect("create persistent job queue")
    }

    #[tokio::test]
    async fn tick_returns_false_when_empty() {
        let queue = test_queue().await;
        assert_eq!(false, queue.tick().await.unwrap());
    }

    #[tokio::test]
    async fn enqueue_and_tick_runs_registered_handler() {
        let queue = test_queue().await;
        let received = Arc::new(Mutex::new(None));

        let received2 = Arc::clone(&received);
        queue.register("send_email", move |payload| {
            *received2.lock().unwrap() = Some(payload.to_string());
            Ok(())
        });

        queue.enqueue("send_email", "user@example.com").await.unwrap();

        assert_eq!(true, queue.tick().await.unwrap());
        assert_eq!(Some("user@example.com".to_string()), received.lock().unwrap().clone());

        // Completed jobs are deleted.
        assert_eq!(false, queue.tick().await.unwrap());
    }

    #[tokio::test]
    async fn unregistered_job_type_fails_and_retries() {
        let queue = test_queue().await.backoff(Duration::from_millis(1), 1);
        queue.enqueue_with_retries("no_such_handler", "payload", 2).await.unwrap();

        // Attempt 1: fails (no handler), rescheduled with a 1ms backoff.
        assert_eq!(true, queue.tick().await.unwrap());
        tokio::time::sleep(Duration::from_millis(5)).await;
        // Attempt 2: fails again, rescheduled again.
        assert_eq!(true, queue.tick().await.unwrap());
        tokio::time::sleep(Duration::from_millis(5)).await;
        // Attempt 3 (2 retries exhausted): marked failed, no longer claimable.
        assert_eq!(true, queue.tick().await.unwrap());
        assert_eq!(false, queue.tick().await.unwrap());
    }

    #[tokio::test]
    async fn job_retries_then_succeeds() {
        let queue = test_queue().await.backoff(Duration::from_millis(1), 1);
        let attempts = Arc::new(AtomicUsize::new(0));

        let attempts2 = Arc::clone(&attempts);
        queue.register("flaky", move |_payload| {
            let n = attempts2.fetch_add(1, Ordering::SeqCst) + 1;
            if n < 3 {
                Err(format!("attempt {n} failed"))
            } else {
                Ok(())
            }
        });

        queue.enqueue_with_retries("flaky", "", 5).await.unwrap();

        for _ in 0..3 {
            queue.tick().await.unwrap();
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        assert_eq!(3, attempts.load(Ordering::SeqCst));
        // Succeeded on the 3rd attempt -> row deleted.
        assert_eq!(false, queue.tick().await.unwrap());
    }

    #[tokio::test]
    async fn recovers_interrupted_running_jobs_on_restart() {
        let pool = DbPool::memory().await.expect("open :memory: pool");

        // Simulate a job left `running` by a process that crashed mid-execution.
        {
            let queue = PersistentJobQueue::new(pool.clone()).await.unwrap();
            queue.enqueue("cleanup", "payload").await.unwrap();
            // Claim it (moves it to `running`) but never finish it.
            let claimed = pool
                .execute("UPDATE rws_jobs SET status = 'running' WHERE job_type = 'cleanup'", &[])
                .await
                .unwrap();
            assert_eq!(1, claimed);
        }

        // "Restart": open a new queue against the same pool.
        let queue = PersistentJobQueue::new(pool).await.unwrap();
        let ran = Arc::new(AtomicUsize::new(0));
        let ran2 = Arc::clone(&ran);
        queue.register("cleanup", move |_| {
            ran2.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        assert_eq!(true, queue.tick().await.unwrap());
        assert_eq!(1, ran.load(Ordering::SeqCst));
    }
}
