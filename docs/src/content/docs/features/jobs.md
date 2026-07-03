---
title: Background Job Queue
description: Run one-shot, fire-and-forget work from request handlers, with retry-with-backoff and optional crash-safe persistence.
---

Requires the `jobs` feature:

```toml
[dependencies]
rust-web-server = { version = "17", features = ["jobs"] }
```

There are two queue types:

- **`JobQueue`** — in-memory worker pool. Simple, no extra dependencies. Queued and retrying jobs are lost if the process crashes or restarts.
- **`PersistentJobQueue`** — backed by the model layer (`model-sqlite` / `model-postgres` / `model-mysql`). Jobs are written to a `rws_jobs` table before being acknowledged, so pending and retrying jobs survive a restart.

## `JobQueue` quick start

```rust
use rust_web_server::jobs::JobQueue;

let queue = JobQueue::new(4); // 4 worker threads

// In a request handler, after committing the signup:
let to = "new-user@example.com".to_string();
queue.submit(move || {
    // send_welcome_email(&to) — return Err(msg) to trigger a retry
    Ok(())
});
```

`submit` returns immediately; the job runs on one of the queue's worker threads.

### Retry with backoff

A job that returns `Err` is retried on the same worker thread with exponential
backoff. Configure the policy with `.max_retries()` / `.backoff()` (defaults:
3 retries, 500 ms initial backoff, 2x multiplier):

```rust
use rust_web_server::jobs::JobQueue;
use std::time::Duration;

let queue = JobQueue::new(4)
    .max_retries(5)
    .backoff(Duration::from_millis(200), 2);
```

`max_retries` counts retries **after** the first attempt — `max_retries(2)`
allows 3 total attempts (1 initial + 2 retries) before giving up. A job that
exhausts its retries is logged and dropped.

:::note[Backoff blocks the worker]
Retries sleep on the same worker thread that picked up the job — a job that
retries repeatedly occupies one worker for the duration of its backoff. Size
the pool with that in mind, or keep retry counts low for jobs expected to fail
often.
:::

### Named jobs

Implement the `Job` trait directly for jobs that need state or a stable name
in log lines, instead of a closure:

```rust
use rust_web_server::jobs::Job;

struct SendWelcomeEmail {
    to: String,
}

impl Job for SendWelcomeEmail {
    fn run(&self) -> Result<(), String> {
        // send_welcome_email(&self.to)
        Ok(())
    }

    fn name(&self) -> &str {
        "send_welcome_email" // used in retry/failure log lines; defaults to "job"
    }
}

queue.submit(SendWelcomeEmail { to: "new-user@example.com".to_string() });
```

### Shutdown

```rust
queue.join(); // stop accepting new jobs; wait for in-flight jobs (and their retries) to finish
```

`join()` is mainly useful in tests and at process shutdown — most long-running
servers simply let the queue live for the process lifetime.

## `PersistentJobQueue`

Additionally requires a `model-*` feature:

```toml
[dependencies]
rust-web-server = { version = "17", features = ["jobs", "model-sqlite"] }
```

Because a closure can't be serialized to disk, persisted jobs are `(job_type,
payload)` string pairs, dispatched at execution time to a handler registered
by name. Register every `job_type` your process enqueues **before** starting
workers — this includes the process that comes back up after a crash, since
recovery works by replaying rows from the `rws_jobs` table, not by replaying
in-memory closures.

```rust,no_run
# async fn example() -> Result<(), rust_web_server::model::DbError> {
use rust_web_server::jobs::PersistentJobQueue;
use rust_web_server::model::DbPool;
use std::sync::Arc;

let pool = DbPool::from_env().await?;
let queue = Arc::new(PersistentJobQueue::new(pool).await?);

queue.register("send_welcome_email", |payload| {
    // send_welcome_email(payload) — payload is whatever string you enqueued
    Ok(())
});

let _worker_handles = Arc::clone(&queue).start(4); // 4 polling worker threads

// From a request handler:
queue.enqueue("send_welcome_email", "new-user@example.com").await?;
# Ok(())
# }
```

### Crash safety

`PersistentJobQueue::new()` creates the `rws_jobs` table if it doesn't exist,
then resets any row left `status = 'running'` back to `pending` — those rows
were being processed when the previous instance crashed or was killed, so they
get picked up again on the next poll.

Completed jobs are deleted from the table. A job that exhausts its retries is
left with `status = 'failed'` and its last error in `last_error`, for
inspection, instead of being deleted.

### Configuration

```rust,no_run
# async fn example() -> Result<(), rust_web_server::model::DbError> {
use rust_web_server::jobs::PersistentJobQueue;
use rust_web_server::model::DbPool;
use std::time::Duration;

let pool = DbPool::from_env().await?;
let queue = PersistentJobQueue::new(pool).await?
    .max_retries(5)                                  // default: 3 (retries after the first attempt)
    .backoff(Duration::from_millis(500), 2)           // default: 500ms initial, 2x multiplier
    .poll_interval(Duration::from_millis(250));       // how often idle workers check for due jobs (default: 500ms)
# Ok(())
# }
```

Use `enqueue_with_retries(job_type, payload, max_retries)` to override the
retry count for a single job instead of the queue-wide default.

### Testing without a background poll loop

`tick()` runs a single poll-claim-execute cycle and returns whether a job was
claimed — useful in tests, or for embedding the queue in a caller-owned loop
instead of `start()`'s dedicated worker threads:

```rust,no_run
# async fn example() -> Result<(), rust_web_server::model::DbError> {
use rust_web_server::jobs::PersistentJobQueue;
use rust_web_server::model::DbPool;

let pool = DbPool::memory().await?;
let queue = PersistentJobQueue::new(pool).await?;
queue.register("noop", |_payload| Ok(()));
queue.enqueue("noop", "").await?;

assert_eq!(true, queue.tick().await?);  // ran the job
assert_eq!(false, queue.tick().await?); // nothing left to do
# Ok(())
# }
```

:::note[Concurrency]
`claim_next` claims a row with `UPDATE rws_jobs SET status = 'running' WHERE id = ? AND status = 'pending'`
— if two workers (in the same process or different processes sharing the
same database) race to claim the same row, only one `UPDATE` succeeds, so the
other worker simply moves on. This is safe under SQLite, PostgreSQL, and MySQL.
:::
