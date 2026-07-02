---
title: Background Scheduler
description: Run background tasks on a fixed rate, fixed delay, or 6-field cron schedule without an external crate.
---

## Quick start

```rust
use std::time::Duration;
use rust_web_server::scheduler::Scheduler;

Scheduler::new()
    .every(Duration::from_secs(60), || purge_expired_sessions())
    .after(Duration::from_secs(30), || emit_heartbeat())
    .cron("0 * * * * *", || rotate_logs()).unwrap()
    .initial_delay(Duration::from_secs(10))
    .start();
```

`.start()` spawns one OS thread per registered task and returns immediately. Threads run for the lifetime of the process.

## Task types

### `.every(interval, task)` — fixed rate

The interval is measured from the **start** of the previous run. If the task takes longer than the interval, the next run starts immediately (no backlog accumulates).

```rust
// Purge sessions every 60 seconds, measured from task start
.every(Duration::from_secs(60), || {
    let removed = session_store.purge_expired();
    println!("purged {} sessions", removed);
})
```

### `.after(delay, task)` — fixed delay

The delay is measured from the **end** of the previous run. Use this when you want a guaranteed pause between runs regardless of how long the task takes.

```rust
// Wait 30 seconds after each heartbeat before sending the next
.after(Duration::from_secs(30), || {
    health_check::emit_heartbeat();
})
```

### `.cron(expr, task)` — cron schedule

Fires whenever the wall-clock time (UTC) matches the 6-field cron expression. Returns `Result<Self, String>` — use `.unwrap()` or propagate the error at startup.

```rust
// Every day at midnight UTC
.cron("0 0 0 * * *", || daily_report::generate()).unwrap()

// Every hour on the hour
.cron("0 0 * * * *", || cache::warm()).unwrap()

// Every 15 seconds
.cron("*/15 * * * * *", || metrics::flush()).unwrap()
```

## `.initial_delay(duration)`

Adds a one-time startup delay before the **most recently registered task** runs for the first time. Chain it immediately after the task it applies to.

```rust
Scheduler::new()
    .every(Duration::from_secs(60), || purge_sessions())
    .initial_delay(Duration::from_secs(10))   // wait 10 s before first purge
    .after(Duration::from_secs(30), || heartbeat())
    // no initial_delay here — heartbeat starts immediately
    .start();
```

## 6-field cron syntax

Format: `"second minute hour day-of-month month day-of-week"` (UTC)

| Field | Range | Examples |
|---|---|---|
| second | 0–59 | `0`, `*/15`, `0,30` |
| minute | 0–59 | `*`, `0`, `5-10` |
| hour | 0–23 | `*`, `0`, `9-17` |
| day-of-month | 1–31 | `*`, `1`, `15` |
| month | 1–12 | `*`, `1`, `6-8` |
| day-of-week | 0–6 (0=Sun) | `*`, `1-5`, `0,6` |

### Supported field syntax

| Syntax | Meaning |
|---|---|
| `*` | Every value |
| `N` | Exact value `N` |
| `*/step` | Every `step`-th value starting from minimum |
| `N-M` | Range from `N` to `M` inclusive |
| `a,b,c` | Comma-separated list of any of the above |

### Examples

```
0 0 * * * *      every minute at second 0
0 */5 * * * *    every 5 minutes
0 0 2 * * *      daily at 02:00 UTC
0 0 9-17 * * *   top of every hour, 09:00–17:00 UTC
0 0 0 1 * *      first day of every month at midnight
0 0 0 * * 1      every Monday at midnight (1 = Monday)
0,30 * * * * *   every 30 seconds
```

## Complete example: session cleanup + cache warm

```rust
use std::time::Duration;
use rust_web_server::scheduler::Scheduler;

fn start_background_tasks(db: Arc<Db>, cache: Arc<Cache>) {
    let db1 = db.clone();
    let cache1 = cache.clone();

    Scheduler::new()
        // Purge expired sessions every hour
        .cron("0 0 * * * *", move || {
            db1.execute("DELETE FROM sessions WHERE expires_at < NOW()").ok();
        })
        .unwrap()
        // Warm the cache 5 seconds after startup, then every 10 minutes
        .every(Duration::from_secs(600), move || {
            cache1.warm_from_db(&db);
        })
        .initial_delay(Duration::from_secs(5))
        .start();
}
```

:::note[Thread per task]
Each registered task gets its own dedicated OS thread. The thread runs for the lifetime of the process. Use `Arc` to share state across tasks; do not capture `&mut` references.
:::

:::note[Cron resolution]
Cron tasks poll at 200 ms resolution. A task is guaranteed to fire at most once per second — if the task finishes within the same second it started, the next poll skips that second to prevent double-firing.
:::
