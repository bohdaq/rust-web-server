//! Sync-to-async bridging for code that must expose a synchronous interface
//! (the [`crate::middleware::Middleware`] / [`crate::application::Application`]
//! traits) while doing async I/O internally — e.g. an `async fn` handler
//! ([`crate::async_state::AsyncAppWithState`], `McpServer::async_tool`).
//!
//! [`block_on_shared`] and [`run_async`] run the future on one long-lived,
//! process-wide runtime (threads `rws-async`) rather than a runtime made for
//! the call: tasks the future leaves behind keep running after it returns.
//! sqlx returns a pool connection to its pool from such a task, so on a
//! runtime dropped after the call the connection was closed instead — a
//! reconnect per request for PostgreSQL/MySQL and an emptied in-memory
//! SQLite database.
//!
//! Requires the `http2` feature (tokio).

#[cfg(test)]
mod tests;

use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};

use tokio::runtime::{Handle, Runtime, RuntimeFlavor};

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
/// The runtime is made for the call and dropped when the future is done, so
/// tasks the future spawns (sqlx returning a pool connection, a background
/// write) are dropped with it. Use [`block_on_shared`] for anything that can
/// leave tasks behind; this stays for futures that can't be sent to another
/// thread.
///
/// `f` is a closure rather than a bare future so the future itself is
/// constructed on whichever thread ends up polling it — it never needs to
/// cross a thread boundary, so it doesn't need to be `Send`. Only the
/// closure (and whatever it captures) and the future's output value
/// (`T`, which is sent back across the scoped-thread join) need to be.
#[doc(hidden)]
pub fn block_on_isolated<F, Fut, T>(f: F) -> T
where
    F: FnOnce() -> Fut + Send,
    Fut: Future<Output = T>,
    T: Send,
{
    match tokio::runtime::Handle::try_current() {
        Ok(_) => std::thread::scope(|s| {
            // The caller's span (an `OtelLayer` request) stays the parent of
            // spans started on the isolated thread.
            let context = crate::otel::Context::current();
            // …and so does the caller (`Identity::current()` in the handler).
            let identity = crate::authz::Identity::current();
            s.spawn(move || {
                let _context = context.attach();
                let _identity = identity.map(crate::authz::identity::enter);
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

// ── the shared runtime ────────────────────────────────────────────────────────

thread_local! {
    /// Set on the shared runtime's own threads.
    static ON_SHARED: Cell<bool> = const { Cell::new(false) };
}

/// The process-wide runtime async handlers run on when the caller can't
/// await them on a multi-thread runtime of its own: worker threads
/// `rws-async`, one per CPU (or `TOKIO_WORKER_THREADS`), made on first use
/// and never dropped, so tasks handlers leave behind (sqlx returning a pool
/// connection) keep running and the I/O resources made on it stay usable.
#[doc(hidden)]
pub fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .thread_name("rws-async")
            .on_thread_start(|| ON_SHARED.with(|on| on.set(true)))
            .enable_all()
            .build()
            .expect("the rws-async runtime")
    })
}

/// Runs `future` on the shared [`runtime`] and waits for it — from any
/// thread: a plain one (the HTTP/1.1 thread pool), tokio's blocking pool, a
/// `current_thread` or `multi_thread` runtime, or one of the shared
/// runtime's own workers (it then hands its worker role on with
/// `block_in_place` while it waits). A panic in `future` is resumed here.
///
/// The caller's span ([`crate::otel::Context`]), [`crate::authz::Identity`]
/// and early-hints destination stay current inside `future`.
///
/// Waiting blocks the calling thread: on a `current_thread` runtime nothing
/// else runs there meanwhile, so `future` must not need that runtime (an
/// I/O resource made on it) — from async code use [`run_async`].
#[doc(hidden)]
pub fn block_on_shared<T, Fut>(future: Fut) -> T
where
    Fut: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let carried = CatchPanic(Box::pin(Carried::capture(future)));
    runtime().spawn(async move {
        let _ = tx.send(carried.await);
    });
    let wait = move || rx.recv();
    let received = if ON_SHARED.with(Cell::get) { tokio::task::block_in_place(wait) } else { wait() };
    match received {
        Ok(Ok(output)) => output,
        Ok(Err(panic)) => std::panic::resume_unwind(panic),
        // The task was dropped without finishing: only at process exit.
        Err(_) => panic!("the rws-async runtime dropped the task"),
    }
}

/// Awaits `future` on the caller's runtime when that's a `multi_thread` one
/// (the async servers' shared runtime: no hand-off), otherwise on the shared
/// [`runtime`] — a `current_thread` caller (per-core runtimes, a
/// `#[tokio::test]`) stays free to run its own tasks meanwhile, and nothing
/// the future leaves behind is tied to a runtime that may go away. Context
/// is carried as in [`block_on_shared`]; a panic is resumed in the caller.
#[doc(hidden)]
pub async fn run_async<T, Fut>(future: Fut) -> T
where
    Fut: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let here = matches!(Handle::try_current().map(|h| h.runtime_flavor()), Ok(RuntimeFlavor::MultiThread));
    if here {
        return future.await;
    }
    match runtime().spawn(Carried::capture(future)).await {
        Ok(output) => output,
        Err(error) if error.is_panic() => std::panic::resume_unwind(error.into_panic()),
        Err(error) => panic!("the rws-async runtime dropped the task: {error}"),
    }
}

/// A future with the caller's thread-local request context — span, identity,
/// early-hints sink — made current around every poll, wherever it's polled.
struct Carried<F> {
    future: Pin<Box<F>>,
    context: crate::otel::Context,
    identity: Option<crate::authz::Identity>,
    hints: Option<Arc<crate::early_hints::Sink>>,
}

impl<F: Future> Carried<F> {
    fn capture(future: F) -> Carried<F> {
        Carried {
            future: Box::pin(future),
            context: crate::otel::Context::current(),
            identity: crate::authz::Identity::current(),
            hints: crate::early_hints::current(),
        }
    }
}

impl<F: Future> Future for Carried<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
        let this = self.get_mut();
        let _context = (!this.context.is_empty()).then(|| this.context.attach());
        let _identity = this.identity.clone().map(crate::authz::identity::enter);
        let future = &mut this.future;
        match &this.hints {
            Some(sink) => crate::early_hints::with_sink(Some(Arc::clone(sink)), || future.as_mut().poll(cx)),
            None => future.as_mut().poll(cx),
        }
    }
}

/// Polls a boxed future, catching a panic as its payload.
struct CatchPanic<F>(Pin<Box<F>>);

impl<F: Future> Future for CatchPanic<F> {
    type Output = Result<F::Output, Box<dyn std::any::Any + Send>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let future = &mut self.0;
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| future.as_mut().poll(cx))) {
            Ok(Poll::Ready(output)) => Poll::Ready(Ok(output)),
            Ok(Poll::Pending) => Poll::Pending,
            Err(panic) => Poll::Ready(Err(panic)),
        }
    }
}
