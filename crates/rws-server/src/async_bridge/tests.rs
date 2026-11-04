use super::{block_on_isolated, block_on_shared, run_async};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn thread_name() -> String {
    std::thread::current().name().unwrap_or("").to_string()
}

/// Polls `flag` for up to five seconds.
fn eventually(flag: &AtomicBool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if flag.load(Ordering::SeqCst) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

/// A future that spawns a task finishing after it has returned — what sqlx
/// does to give a pool connection back.
fn leaves_a_task_behind(done: Arc<AtomicBool>) -> impl std::future::Future<Output = ()> + Send + 'static {
    async move {
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            done.store(true, Ordering::SeqCst);
        });
    }
}

// The bug: `block_on_isolated` drops its runtime, and the task with it.
#[test]
fn tasks_left_behind_by_block_on_shared_still_run() {
    let done = Arc::new(AtomicBool::new(false));
    block_on_shared(leaves_a_task_behind(Arc::clone(&done)));
    assert!(eventually(&done), "the spawned task never ran");
}

#[test]
fn block_on_isolated_drops_tasks_left_behind() {
    let done = Arc::new(AtomicBool::new(false));
    let task = Arc::clone(&done);
    block_on_isolated(move || leaves_a_task_behind(task));
    std::thread::sleep(Duration::from_millis(100));
    assert!(!done.load(Ordering::SeqCst), "documented: the runtime is gone, and its tasks with it");
}

#[test]
fn block_on_shared_from_a_plain_thread() {
    assert!(tokio::runtime::Handle::try_current().is_err());
    let name = block_on_shared(async {
        tokio::time::sleep(Duration::from_millis(1)).await;
        thread_name()
    });
    assert_eq!("rws-async", name);
}

#[tokio::test]
async fn block_on_shared_inside_a_current_thread_runtime() {
    assert_eq!("rws-async", block_on_shared(async { thread_name() }));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn block_on_shared_on_a_multi_thread_worker() {
    let name = tokio::spawn(async { block_on_shared(async { thread_name() }) }).await.unwrap();
    assert_eq!("rws-async", name);
}

#[test]
fn block_on_shared_nested_on_its_own_workers() {
    // A handler that calls another async app synchronously: every worker
    // could be waiting, so each hands its worker role on while it waits.
    let threads = super::runtime().metrics().num_workers() * 2;
    let handles: Vec<_> = (0..threads)
        .map(|i| std::thread::spawn(move || block_on_shared(async move { block_on_shared(async move { i * 2 }) })))
        .collect();
    let sum: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
    assert_eq!((0..threads).map(|i| i * 2).sum::<usize>(), sum);
}

#[test]
fn block_on_shared_resumes_a_panic() {
    let caught = std::panic::catch_unwind(|| block_on_shared(async { panic!("handler failed") }));
    let message = caught.err().and_then(|p| p.downcast_ref::<&str>().map(|s| s.to_string()));
    assert_eq!(Some("handler failed".to_string()), message);
    // The runtime is still there.
    assert_eq!(3, block_on_shared(async { 3 }));
}

#[tokio::test]
async fn run_async_on_a_current_thread_runtime_uses_the_shared_one() {
    // The test's runtime stays free meanwhile: a task on it makes progress.
    let local = tokio::spawn(async { 7 });
    assert_eq!("rws-async", run_async(async { thread_name() }).await);
    assert_eq!(7, local.await.unwrap());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_async_on_a_multi_thread_runtime_stays_there() {
    let name = tokio::spawn(run_async(async { thread_name() })).await.unwrap();
    assert_ne!("rws-async", name);
}

#[tokio::test]
async fn run_async_resumes_a_panic() {
    let result = tokio::spawn(run_async(async { panic!("async handler failed") })).await;
    let panic = result.err().expect("a panic").into_panic();
    assert_eq!(Some(&"async handler failed"), panic.downcast_ref::<&str>());
}

#[test]
fn works_with_no_runtime_active() {
    // Called from a plain #[test] fn — no tokio runtime is running at all,
    // matching the HTTP/1.1 thread-pool calling context.
    assert!(tokio::runtime::Handle::try_current().is_err(), "test setup: no runtime should be active here");
    let result = block_on_isolated(|| async { 1 + 1 });
    assert_eq!(2, result);
}

// The critical regression test: `tokio::task::block_in_place` (the mechanism
// this replaces) *panics* on a `current_thread` runtime. `#[tokio::test]`
// defaults to exactly that flavor, so this test alone would have failed
// under the old implementation — it's the direct proof of the fix.
#[tokio::test]
async fn works_inside_current_thread_runtime() {
    assert!(tokio::runtime::Handle::try_current().is_ok(), "test setup: a runtime should be active here");
    let result = block_on_isolated(|| async { 1 + 1 });
    assert_eq!(2, result);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn works_inside_multi_thread_runtime() {
    let result = block_on_isolated(|| async { 1 + 1 });
    assert_eq!(2, result);
}

#[tokio::test]
async fn propagates_the_future_output_by_value() {
    let result = block_on_isolated(|| async { "hello".to_string() });
    assert_eq!("hello", result);
}

#[tokio::test]
async fn closure_can_borrow_from_the_calling_scope() {
    let input = [1, 2, 3];
    let result = block_on_isolated(|| async { input.iter().sum::<i32>() });
    assert_eq!(6, result);
}

#[tokio::test]
async fn inner_future_can_actually_await_async_io() {
    // Not just an immediately-ready future — proves the spawned runtime can
    // drive real async work (a timer) to completion.
    let result = block_on_isolated(|| async {
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        42
    });
    assert_eq!(42, result);
}
