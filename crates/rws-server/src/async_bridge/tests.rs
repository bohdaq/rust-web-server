use super::block_on_isolated;

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
