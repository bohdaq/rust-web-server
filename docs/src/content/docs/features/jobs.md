---
title: Background Job Queue
description: Run one-shot, fire-and-forget work from request handlers, with retry-with-backoff and optional crash-safe persistence.
---

Requires the `jobs` feature:

```toml
[dependencies]
rust-web-server = { version = "18", features = ["jobs"] }
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
rust-web-server = { version = "18", features = ["jobs", "model-sqlite"] }
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

`queue.stop()` tells the workers started by `start()` to exit after their
current poll; join the returned handles to wait for them.

### Crash safety

`PersistentJobQueue::new()` creates the `rws_jobs` table if it doesn't exist
(adding the `queue`, `priority` and `unique_key` columns to one from before
18.35.0), then resets any row left `status = 'running'` back to `pending` — those rows
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

### When, where and how often

`enqueue_with(job_type, payload, options)` takes `EnqueueOptions` (18.35.0):

```rust,no_run
# async fn example(queue: &rust_web_server::jobs::PersistentJobQueue) -> Result<(), rust_web_server::model::DbError> {
use std::time::Duration;
use rust_web_server::jobs::EnqueueOptions;

queue.enqueue_with("send_reminder", "user:42", EnqueueOptions::new()
    .run_in(Duration::from_secs(24 * 3600))   // or .run_at(SystemTime)
    .queue("mailers")                          // default "default"
    .priority(10)                              // higher first among due jobs; default 0
    .unique("reminder:42")                     // skipped while one with this key is pending or running
    .max_retries(5)).await?;
# Ok(())
# }
```

| Option | Effect |
|---|---|
| `run_at(time)` / `run_in(delay)` | Not claimed before that time (second precision) |
| `queue(name)` | A named queue; `PersistentJobQueue::queues(&["mailers"])` makes a process take only those |
| `priority(n)` | Due jobs run highest priority first, then oldest due |
| `unique(key)` | While a job of the same type with that key is pending or running, `enqueue_with` returns its id and inserts nothing. Checked when enqueuing, so two enqueues at the same instant from different processes can both insert |
| `max_retries(n)` | Overrides the queue's default |

### The dead-letter list

A job that exhausts its retries stays in `rws_jobs` with `status = 'failed'` and its `last_error`. `failed_jobs(limit)` lists them, `retry(id)` puts one back in the queue (due now, attempts reset), `delete(id)` removes it, and `on_dead(|job| …)` is called as each one fails for good — to alert someone, say. `jobs(status, limit)` lists any status as `JobInfo` rows.

### Recurring jobs across replicas

`schedule(name, cron, job_type, payload)` enqueues a job on a six-field cron schedule (seconds first, UTC, as [`Scheduler`](/features/scheduler/)). Every replica can declare the same schedule: each due time is claimed by exactly one of them with a compare-and-set on the schedule's row in `rws_job_schedules`, and only that one enqueues the job. Runs missed while every replica was down collapse into one.

```rust,no_run
# async fn example(pool: rust_web_server::model::DbPool) -> Result<(), Box<dyn std::error::Error>> {
use std::sync::Arc;
use rust_web_server::jobs::PersistentJobQueue;

let queue = Arc::new(PersistentJobQueue::new(pool).await?
    .schedule("nightly-report", "0 0 3 * * *", "build_report", "")?);
queue.register("build_report", |_| Ok(()));
Arc::clone(&queue).start(2);   // the first worker checks the schedules about once a second
# Ok(())
# }
```

`schedule_with(…, options)` gives the enqueued jobs `EnqueueOptions`; `tick_schedules()` runs one check in a loop of your own.

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
other worker simply moves on. This is safe under SQLite, PostgreSQL, and MySQL; CI runs the queue's tests on
all three. (Before 18.35.0 the queue's SQL only worked on SQLite.)
:::
