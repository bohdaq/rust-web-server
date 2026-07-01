//! Composable middleware pipeline.
//!
//! [`Middleware`] is a single layer that wraps request dispatch. Implement it
//! to add cross-cutting behaviour — logging, authentication, rate limiting,
//! header injection — without editing the inner application.
//!
//! [`WithMiddleware`] stacks one or more middleware layers around any
//! [`Application`]. Layers run in registration order on the way in and in
//! reverse order on the way out.
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::middleware::{Middleware, WithMiddleware};
//! use rust_web_server::application::Application;
//! use rust_web_server::request::Request;
//! use rust_web_server::response::Response;
//! use rust_web_server::server::ConnectionInfo;
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//!
//! pub struct LoggingMiddleware;
//!
//! impl Middleware for LoggingMiddleware {
//!     fn handle(
//!         &self,
//!         request: &Request,
//!         connection: &ConnectionInfo,
//!         next: &dyn Application,
//!     ) -> Result<Response, String> {
//!         println!("{} {}", request.method, request.request_uri);
//!         let response = next.execute(request, connection)?;
//!         println!("  → {}", response.status_code);
//!         Ok(response)
//!     }
//! }
//!
//! let app = WithMiddleware::new(App::new())
//!     .wrap(LoggingMiddleware);
//! ```

#[cfg(test)]
mod tests;

use std::sync::Arc;

use crate::application::Application;
use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

/// Built-in middleware that enforces the process-wide rate limit
/// (configured via `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS` and
/// `RWS_CONFIG_RATE_LIMIT_WINDOW_SECS`).
///
/// Returns `429 Too Many Requests` when the sliding-window budget for the
/// client IP is exhausted; otherwise passes the request to the next layer.
///
/// # Example
///
/// ```rust,no_run
/// use rust_web_server::app::App;
/// use rust_web_server::middleware::{WithMiddleware, RateLimitLayer};
/// use rust_web_server::core::New;
///
/// let app = App::new().wrap(RateLimitLayer);
/// ```
pub struct RateLimitLayer;

impl Middleware for RateLimitLayer {
    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        use crate::error::{AppError, IntoResponse};
        if crate::rate_limit::global().check(&connection.client.ip) {
            next.execute(request, connection)
        } else {
            Ok(AppError::TooManyRequests.into_response())
        }
    }
}

/// A single middleware layer.
///
/// Receive the request, call `next.execute(request, connection)` to pass
/// control to the next layer (or the inner application), and optionally
/// inspect or transform the resulting response.
///
/// Short-circuit by returning a `Response` (or `Err`) without calling `next`.
pub trait Middleware: Send + Sync {
    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String>;
}

/// An [`Application`] that applies a stack of [`Middleware`] layers before
/// dispatching to an inner application.
///
/// Layers are applied in registration order: the first `.wrap()`ed middleware
/// runs first on the request path and last on the response path.
pub struct WithMiddleware<A> {
    inner: A,
    layers: Vec<Arc<dyn Middleware>>,
}

impl<A: Application> WithMiddleware<A> {
    /// Wrap `app` with no initial middleware.
    pub fn new(app: A) -> Self {
        WithMiddleware { inner: app, layers: Vec::new() }
    }

    /// Add a middleware layer. Layers run in registration order.
    pub fn wrap(mut self, layer: impl Middleware + 'static) -> Self {
        self.layers.push(Arc::new(layer));
        self
    }
}

impl<A: Clone> Clone for WithMiddleware<A> {
    fn clone(&self) -> Self {
        WithMiddleware { inner: self.inner.clone(), layers: self.layers.clone() }
    }
}

impl<A: Application> Application for WithMiddleware<A> {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        Chain { app: &self.inner, layers: &self.layers, index: 0 }.execute(request, connection)
    }
}

// Internal chain that carries the remaining layers and the final app.
struct Chain<'a> {
    app: &'a dyn Application,
    layers: &'a [Arc<dyn Middleware>],
    index: usize,
}

impl<'a> Application for Chain<'a> {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        if self.index < self.layers.len() {
            let next = Chain { app: self.app, layers: self.layers, index: self.index + 1 };
            self.layers[self.index].handle(request, connection, &next)
        } else {
            self.app.execute(request, connection)
        }
    }
}
