//! Background task scheduler — a `@Scheduled`-style runner for fixed-rate,
//! fixed-delay, and cron-expression tasks.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::time::Duration;
//! use rust_web_server::scheduler::Scheduler;
//!
//! Scheduler::new()
//!     // Runs every 60 s (interval measured from task start).
//!     .every(Duration::from_secs(60), || println!("tick"))
//!     // Waits 30 s after each run completes before starting the next.
//!     .after(Duration::from_secs(30), || println!("heartbeat"))
//!     // Fires at second 0 of every minute.
//!     .cron("0 * * * * *", || println!("every minute")).unwrap()
//!     // 10 s pause before the first run of the most recently added task.
//!     .initial_delay(Duration::from_secs(10))
//!     .start();
//! ```

pub mod cron;
pub use cron::CronSchedule;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::{Duration, Instant};

/// A `@Scheduled`-style background task runner.
///
/// Register tasks with `.every()`, `.after()`, or `.cron()`, then call
/// `.start()` to spawn one dedicated background thread per task.
pub struct Scheduler {
    tasks: Vec<Task>,
}

struct Task {
    kind: TaskKind,
    initial_delay: Duration,
    f: Arc<dyn Fn() + Send + Sync + 'static>,
}

enum TaskKind {
    /// Fixed rate — interval measured from the *start* of the previous run.
    FixedRate(Duration),
    /// Fixed delay — interval measured from the *end* of the previous run.
    FixedDelay(Duration),
    /// Cron — fires whenever the wall clock matches the parsed expression.
    Cron(CronSchedule),
}

impl Scheduler {
    pub fn new() -> Self {
        Scheduler { tasks: Vec::new() }
    }

    /// Run `task` every `interval`, measured from the start of the previous run.
    /// If the task takes longer than `interval`, the next run starts immediately.
    pub fn every(mut self, interval: Duration, task: impl Fn() + Send + Sync + 'static) -> Self {
        self.tasks.push(Task {
            kind: TaskKind::FixedRate(interval),
            initial_delay: Duration::ZERO,
            f: Arc::new(task),
        });
        self
    }

    /// Run `task` with `delay` between the end of one run and the start of the next.
    pub fn after(mut self, delay: Duration, task: impl Fn() + Send + Sync + 'static) -> Self {
        self.tasks.push(Task {
            kind: TaskKind::FixedDelay(delay),
            initial_delay: Duration::ZERO,
            f: Arc::new(task),
        });
        self
    }

    /// Run `task` according to a 6-field cron expression.
    ///
    /// Format: `"second minute hour day-of-month month day-of-week"` (UTC).
    ///
    /// Each field supports `*`, an exact value, `*/step`, an `N-M` range, and
    /// comma-separated combinations, e.g. `"0,30 * * * * *"` fires at seconds 0 and 30.
    ///
    /// Day-of-week: 0 = Sunday, 6 = Saturday.
    pub fn cron(
        mut self,
        expr: &str,
        task: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self, String> {
        let schedule = CronSchedule::parse(expr)?;
        self.tasks.push(Task {
            kind: TaskKind::Cron(schedule),
            initial_delay: Duration::ZERO,
            f: Arc::new(task),
        });
        Ok(self)
    }

    /// Add an initial delay before the first run of the most recently registered task.
    pub fn initial_delay(mut self, delay: Duration) -> Self {
        if let Some(t) = self.tasks.last_mut() {
            t.initial_delay = delay;
        }
        self
    }

    /// Spawn one background thread per registered task and return immediately.
    /// Threads run for the lifetime of the process.
    pub fn start(self) {
        for task in self.tasks {
            let f = task.f.clone();
            let initial_delay = task.initial_delay;
            std::thread::spawn(move || {
                if !initial_delay.is_zero() {
                    std::thread::sleep(initial_delay);
                }
                match task.kind {
                    TaskKind::FixedRate(interval) => loop {
                        let start = Instant::now();
                        f();
                        let elapsed = start.elapsed();
                        if elapsed < interval {
                            std::thread::sleep(interval - elapsed);
                        }
                    },
                    TaskKind::FixedDelay(delay) => loop {
                        f();
                        std::thread::sleep(delay);
                    },
                    TaskKind::Cron(ref schedule) => {
                        // Poll at 200 ms resolution; track last-fired second to avoid
                        // double-firing when the task finishes within the same second.
                        let mut last_fired_secs: u64 = 0;
                        loop {
                            let now_secs = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            if now_secs != last_fired_secs
                                && schedule.matches_epoch(now_secs)
                            {
                                last_fired_secs = now_secs;
                                f();
                            }
                            std::thread::sleep(Duration::from_millis(200));
                        }
                    }
                }
            });
        }
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Scheduler::new()
    }
}
