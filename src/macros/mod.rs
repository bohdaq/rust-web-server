//! Declarative routing macro.
//!
//! The [`routes!`] macro builds an [`crate::state::AppWithState`],
//! [`crate::async_state::AsyncAppWithState`], or any other builder that
//! exposes `.get()`, `.post()`, `.put()`, `.patch()`, and `.delete()` methods.

#[cfg(test)]
mod tests;

/// Build a routing app from a declarative table.
///
/// Syntax:
/// ```text
/// routes! {
///     <builder>,
///     METHOD "path" => handler,
///     ...
/// }
/// ```
///
/// `METHOD` must be one of `GET`, `POST`, `PUT`, `PATCH`, or `DELETE` (all
/// caps). The builder receives one `.method(path, handler)` call per entry,
/// chained in declaration order.
///
/// Handlers may be named functions or closures — anything accepted by the
/// corresponding builder method. A trailing comma after the last route is
/// optional.
///
/// # Example — stateful sync app
///
/// ```rust
/// use rust_web_server::app::App;
/// use rust_web_server::core::New;
/// use rust_web_server::routes;
/// use rust_web_server::request::Request;
/// use rust_web_server::router::PathParams;
/// use rust_web_server::server::ConnectionInfo;
/// use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
///
/// struct Db;
///
/// // AppWithState<S> passes &S (not &Arc<S>) to the handler.
/// fn list_users(
///     _req: &Request,
///     _params: &PathParams,
///     _conn: &ConnectionInfo,
///     _state: &Db,
/// ) -> Response {
///     let mut r = Response::new();
///     r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
///     r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
///     r
/// }
///
/// fn create_user(
///     _req: &Request,
///     _params: &PathParams,
///     _conn: &ConnectionInfo,
///     _state: &Db,
/// ) -> Response {
///     let mut r = Response::new();
///     r.status_code = *STATUS_CODE_REASON_PHRASE.n201_created.status_code;
///     r.reason_phrase = STATUS_CODE_REASON_PHRASE.n201_created.reason_phrase.to_string();
///     r
/// }
///
/// let app = routes! {
///     App::with_state(Db),
///     GET  "/users" => list_users,
///     POST "/users" => create_user,
/// };
/// ```
///
/// # Example — inline closures
///
/// ```rust
/// use rust_web_server::app::App;
/// use rust_web_server::core::New;
/// use rust_web_server::routes;
/// use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
///
/// let app = routes! {
///     App::with_state(42u32),
///     GET "/ping" => |_req, _params, _conn, _state: &u32| {
///         let mut r = Response::new();
///         r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
///         r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
///         r
///     },
/// };
/// ```
#[macro_export]
macro_rules! routes {
    // Base case — no routes left (handles optional trailing comma).
    ($app:expr $(,)?) => { $app };

    ($app:expr, GET $path:literal => $handler:expr $(, $($tail:tt)*)?) => {
        $crate::routes!($app.get($path, $handler) $(, $($tail)*)?)
    };
    ($app:expr, POST $path:literal => $handler:expr $(, $($tail:tt)*)?) => {
        $crate::routes!($app.post($path, $handler) $(, $($tail)*)?)
    };
    ($app:expr, PUT $path:literal => $handler:expr $(, $($tail:tt)*)?) => {
        $crate::routes!($app.put($path, $handler) $(, $($tail)*)?)
    };
    ($app:expr, PATCH $path:literal => $handler:expr $(, $($tail:tt)*)?) => {
        $crate::routes!($app.patch($path, $handler) $(, $($tail)*)?)
    };
    ($app:expr, DELETE $path:literal => $handler:expr $(, $($tail:tt)*)?) => {
        $crate::routes!($app.delete($path, $handler) $(, $($tail)*)?)
    };
}
