//! Sync-to-async bridging for code that must expose a synchronous interface
//! (the [`crate::middleware::Middleware`] / [`crate::application::Application`]
//! traits) while doing async I/O internally — e.g. an `h2` client call or an
//! `async fn` handler.
//!
//! Requires the `http2` feature (tokio).

#[cfg(test)]
mod tests;

use std::future::Future;

/// Runs the future produced by `f` to completion, regardless of whether the
/// calling thread is already inside a tokio runtime.
///
/// - **Already inside a runtime** (HTTP/2 / HTTP/3 async server): spawns a
///   scoped OS thread with its own dedicated single-threaded runtime and
///   blocks *that* thread instead of the current one. This works correctly
///   on both the `current_thread` and `multi_thread` schedulers — unlike
///   [`tokio::task::block_in_place`], which requires `multi_thread` and
///   panics under `current_thread` — and it doesn't pull a worker thread out
///   of a shared pool the way `block_in_place` does.
/// - **Not inside any runtime** (the HTTP/1.1 thread pool): builds a
///   temporary single-threaded runtime directly on the current thread.
///
/// `f` is a closure rather than a bare future so the future itself is
/// constructed on whichever thread ends up polling it — it never needs to
/// cross a thread boundary, so it doesn't need to be `Send`. Only the
/// closure (and whatever it captures) and the future's output value
/// (`T`, which is sent back across the scoped-thread join) need to be.
pub(crate) fn block_on_isolated<F, Fut, T>(f: F) -> T
where
    F: FnOnce() -> Fut + Send,
    Fut: Future<Output = T>,
    T: Send,
{
    match tokio::runtime::Handle::try_current() {
        Ok(_) => std::thread::scope(|s| {
            s.spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(f())
            })
            .join()
            .unwrap()
        }),
        Err(_) => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(f()),
    }
}
