//! Per-route request timeouts.
//!
//! A single global read timeout (30 s per connection, or `RWS_CONFIG_*`)
//! applies uniformly to every route today. A file-upload endpoint may
//! legitimately need 120 s while a health check must complete in 500 ms —
//! there's no way to express that difference without wrapping a handler
//! yourself. This module provides that wrapping.
//!
//! # Honest limitation: Rust cannot preempt a running synchronous call
//!
//! For synchronous handlers (`Router`, `AppWithState`, plain `Application`s),
//! there is no safe way to forcibly stop a thread that's already running.
//! [`with_timeout`], [`with_timeout_state`], and [`TimeoutLayer`] all run the
//! wrapped work on a background thread and bound how long they *wait* for
//! it — if the deadline passes, the caller gets a `504 Gateway Timeout`
//! response immediately, but the background thread keeps running to
//! completion (its result is simply discarded). This bounds the **client's**
//! wait time, not the handler's actual resource usage.
//!
//! For genuine cancellation, use [`with_timeout_async`] with
//! `AsyncAppWithState` (requires the `http2` feature): dropping a `Future`
//! that hasn't finished actually stops its execution at the next `.await`
//! point, backed by `tokio::time::timeout`.
//!
//! # Example — `Router`
//!
//! ```rust,no_run
//! use rust_web_server::router::Router;
//! use rust_web_server::timeout::with_timeout;
//! use rust_web_server::response::Response;
//! use rust_web_server::core::New;
//! use std::time::Duration;
//!
//! let router = Router::new()
//!     .get("/healthz", with_timeout(Duration::from_millis(500), |_req, _params, _conn| Response::new()))
//!     .post("/upload", with_timeout(Duration::from_secs(120), |_req, _params, _conn| Response::new()));
//! ```
//!
//! # Example — wrapping a whole `Application`
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::timeout::TimeoutLayer;
//! use std::time::Duration;
//!
//! let app = TimeoutLayer::new(App::new(), Duration::from_secs(5));
//! ```

#[cfg(test)]
mod tests;

use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::application::Application;
use crate::core::New;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::router::PathParams;
use crate::server::ConnectionInfo;

/// Runs `compute` on a detached background thread. Returns `Some(result)` if
/// it finishes within `duration`, `None` otherwise — in which case the
/// thread keeps running to completion; its result is dropped when it
/// eventually sends on the (by-then-abandoned) channel.
fn run_with_timeout<T, F>(duration: Duration, compute: F) -> Option<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(compute());
    });
    rx.recv_timeout(duration).ok()
}

fn timeout_response() -> Response {
    let cr = Range::get_content_range(
        b"504 Gateway Timeout".to_vec(),
        MimeType::TEXT_PLAIN.to_string(),
    );
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n504_gateway_timeout.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n504_gateway_timeout.reason_phrase.to_string();
    r.content_range_list = vec![cr];
    r
}

/// Wraps a stateless handler (the `Router` handler signature) so it must
/// complete within `duration` or the caller gets `504 Gateway Timeout`
/// instead of waiting further. See the [module docs](self) for the
/// cancellation caveat.
pub fn with_timeout<F>(
    duration: Duration,
    handler: F,
) -> impl Fn(&Request, &PathParams, &ConnectionInfo) -> Response + Send + Sync + 'static
where
    F: Fn(&Request, &PathParams, &ConnectionInfo) -> Response + Send + Sync + 'static,
{
    let handler = Arc::new(handler);
    move |req, params, conn| {
        let handler = Arc::clone(&handler);
        let req = req.clone();
        let params = params.clone();
        let conn = conn.clone();
        run_with_timeout(duration, move || handler(&req, &params, &conn))
            .unwrap_or_else(timeout_response)
    }
}

/// Wraps an `AppWithState<S>` handler (which additionally receives `&S`) so
/// it must complete within `duration` or the caller gets `504 Gateway
/// Timeout` instead of waiting further.
///
/// Requires `S: Clone` — the wrapped call runs on a background thread that
/// needs its own owned copy of the state, since the handler signature only
/// gives a borrowed `&S`. Most `AppWithState` state types hold their own
/// data behind `Arc` internally and are cheap to `#[derive(Clone)]`; if
/// yours isn't, wrap the whole app with [`TimeoutLayer`] instead, or switch
/// the route to `AsyncAppWithState` and use [`with_timeout_async`], which
/// needs no `Clone` bound at all.
pub fn with_timeout_state<S, F>(
    duration: Duration,
    handler: F,
) -> impl Fn(&Request, &PathParams, &ConnectionInfo, &S) -> Response + Send + Sync + 'static
where
    S: Clone + Send + Sync + 'static,
    F: Fn(&Request, &PathParams, &ConnectionInfo, &S) -> Response + Send + Sync + 'static,
{
    let handler = Arc::new(handler);
    move |req, params, conn, state: &S| {
        let handler = Arc::clone(&handler);
        let req = req.clone();
        let params = params.clone();
        let conn = conn.clone();
        let state = state.clone();
        run_with_timeout(duration, move || handler(&req, &params, &conn, &state))
            .unwrap_or_else(timeout_response)
    }
}

/// Wraps any owned [`Application`] (or a shared `Arc<dyn Application>`) so
/// every request through it must complete within `duration` or the client
/// gets `504 Gateway Timeout` instead of waiting further.
///
/// Use this to put one blanket timeout around an entire `App`/`AppWithState`/
/// custom `Application`. For different timeouts on different routes within
/// the same app, use [`with_timeout`] / [`with_timeout_state`] /
/// [`with_timeout_async`] on individual handlers instead.
pub struct TimeoutLayer<A: ?Sized> {
    inner: Arc<A>,
    duration: Duration,
}

impl<A: Application + Send + Sync + 'static> TimeoutLayer<A> {
    /// Wrap an owned application.
    pub fn new(inner: A, duration: Duration) -> Self {
        TimeoutLayer { inner: Arc::new(inner), duration }
    }
}

impl<A: Application + Send + Sync + ?Sized + 'static> TimeoutLayer<A> {
    /// Wrap an already-shared application (e.g. `Arc<dyn Application + Send + Sync>`).
    pub fn from_arc(inner: Arc<A>, duration: Duration) -> Self {
        TimeoutLayer { inner, duration }
    }
}

impl<A: Application + Send + Sync + ?Sized + 'static> Application for TimeoutLayer<A> {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        let inner = Arc::clone(&self.inner);
        let request = request.clone();
        let connection = connection.clone();
        match run_with_timeout(self.duration, move || inner.execute(&request, &connection)) {
            Some(result) => result,
            None => Ok(timeout_response()),
        }
    }
}

#[cfg(feature = "http2")]
mod async_timeout {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::time::Duration;

    use crate::request::Request;
    use crate::response::Response;
    use crate::router::PathParams;
    use crate::server::ConnectionInfo;

    type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

    /// Wraps an `AsyncAppWithState<S>` handler so its future is dropped —
    /// genuinely cancelled at its next `.await` point — if it doesn't
    /// resolve within `duration`. Backed by `tokio::time::timeout`; requires
    /// the `http2` feature. No `Clone` bound on `S`: `AsyncAppWithState`
    /// already passes state as an owned `Arc<S>`.
    pub fn with_timeout_async<S, F, Fut>(
        duration: Duration,
        handler: F,
    ) -> impl Fn(Request, PathParams, ConnectionInfo, Arc<S>) -> BoxFuture<Response> + Send + Sync + 'static
    where
        S: Send + Sync + 'static,
        F: Fn(Request, PathParams, ConnectionInfo, Arc<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        move |req, params, conn, state| {
            let fut = handler(req, params, conn, state);
            Box::pin(async move {
                match tokio::time::timeout(duration, fut).await {
                    Ok(response) => response,
                    Err(_) => super::timeout_response(),
                }
            })
        }
    }
}

#[cfg(feature = "http2")]
pub use async_timeout::with_timeout_async;
