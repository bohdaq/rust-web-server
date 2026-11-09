//! Route groups, guards and resources (18.35.0, #94).
//!
//! A group registers routes under a shared path prefix and runs the same
//! guards — and, on [`AppWithState`](crate::state::AppWithState), the same
//! middleware — in front of each:
//!
//! ```rust
//! # extern crate rws_server as rust_web_server;
//! use rust_web_server::app::App;
//! use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
//!
//! fn signed_in(request: &rust_web_server::request::Request, _: &rust_web_server::server::ConnectionInfo, _: &()) -> Result<(), Response> {
//!     match request.header("authorization") {
//!         Some(_) => Ok(()),
//!         None => Err(Response::text(STATUS_CODE_REASON_PHRASE.n401_unauthorized, "sign in")),
//!     }
//! }
//!
//! let app = App::with_state(()).group("/admin", |admin| {
//!     admin
//!         .guard(signed_in)
//!         .get("/", |_, _, _, _| Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "dashboard"))
//!         .resource("/posts", |posts| {
//!             posts
//!                 .index(|_, _, _, _| Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "all posts"))
//!                 .show(|_, params, _, _| Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, params.get("id").unwrap_or("")))
//!         })
//! });
//! ```
//!
//! Guards (`before_action`) are checked in the order added, outer groups
//! first, and apply to every route of the group whichever order they were
//! added in; the first `Err(response)` answers the request. Groups nest.
//!
//! [`Resource`] registers the conventional routes for a collection:
//!
//! | Method | Path | Builder | Name |
//! |---|---|---|---|
//! | GET | `/posts` | `index` | `posts.index` |
//! | GET | `/posts/new` | `new_form` | `posts.new` |
//! | POST | `/posts` | `create` | `posts.create` |
//! | GET | `/posts/{id}` | `show` | `posts.show` |
//! | GET | `/posts/{id}/edit` | `edit` | `posts.edit` |
//! | PUT, PATCH | `/posts/{id}` | `update` | `posts.update` |
//! | DELETE | `/posts/{id}` | `destroy` | `posts.destroy` |
//!
//! `new` comes before `{id}`, so `/posts/new` is never taken as an id. The
//! names (the resource path's last segment, then the action) work with
//! [`url_for`](crate::router::url_for); `.name(…)` on the resource changes
//! the prefix.

#[cfg(test)]
mod tests;

use std::sync::Arc;

use crate::application::Application;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::Response;
use crate::router::PathParams;
use crate::server::ConnectionInfo;

/// A route guard: `Ok(())` lets the request through, `Err(response)`
/// answers it (a redirect to the login page, a `401`, a `403`).
pub type Guard<S> = Arc<dyn Fn(&Request, &ConnectionInfo, &S) -> Result<(), Response> + Send + Sync>;

/// A sync handler as groups store it.
pub(crate) type SyncHandler<S> = Arc<dyn Fn(&Request, &PathParams, &ConnectionInfo, &Arc<S>) -> Response + Send + Sync>;

/// A route a group collected, before the app registers it.
pub(crate) struct GroupRoute<H> {
    pub(crate) method: &'static str,
    pub(crate) pattern: String,
    pub(crate) name: Option<String>,
    pub(crate) handler: H,
}

/// Join a group prefix and a route pattern: `/admin` + `/` = `/admin`,
/// `/admin` + `/posts` = `/admin/posts`, `` + `` = `/`.
pub fn join_path(prefix: &str, pattern: &str) -> String {
    let prefix = prefix.trim_end_matches('/');
    let pattern = pattern.trim_start_matches('/');
    let joined = match (prefix.is_empty(), pattern.is_empty()) {
        (_, true) => prefix.to_string(),
        (true, false) => format!("/{pattern}"),
        (false, false) => format!("{prefix}/{pattern}"),
    };
    if joined.is_empty() {
        "/".to_string()
    } else if joined.starts_with('/') {
        joined
    } else {
        format!("/{joined}")
    }
}

/// The name prefix of a resource at `path`: its last literal segment.
fn resource_name(path: &str) -> String {
    path.rsplit('/').find(|s| !s.is_empty() && !s.starts_with([':', '*', '{'])).unwrap_or("resource").to_string()
}

/// The terminal of a group route's middleware chain: the handler itself.
struct Endpoint<'a, S> {
    handler: &'a SyncHandler<S>,
    params: &'a PathParams,
    state: &'a Arc<S>,
}

impl<S> Application for Endpoint<'_, S> {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        Ok((self.handler)(request, self.params, connection, self.state))
    }
}

/// The rest of the middleware chain after `layers[0]`.
struct Rest<'a> {
    layers: &'a [Arc<dyn Middleware>],
    endpoint: &'a dyn Application,
}

impl Application for Rest<'_> {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        match self.layers.split_first() {
            Some((layer, layers)) => layer.handle(request, connection, &Rest { layers, endpoint: self.endpoint }),
            None => self.endpoint.execute(request, connection),
        }
    }
}

fn internal_error(message: String) -> Response {
    use crate::error::{AppError, IntoResponse};
    AppError::Internal(message).into_response()
}

/// Wrap `handler` in `guards` then `layers` (first = outermost).
fn wrap_sync<S: Send + Sync + 'static>(handler: SyncHandler<S>, guards: Arc<[Guard<S>]>, layers: Arc<[Arc<dyn Middleware>]>) -> SyncHandler<S> {
    if guards.is_empty() && layers.is_empty() {
        return handler;
    }
    Arc::new(move |request, params, connection, state| {
        for guard in guards.iter() {
            if let Err(response) = guard(request, connection, state) {
                return response;
            }
        }
        if layers.is_empty() {
            return handler(request, params, connection, state);
        }
        let endpoint = Endpoint { handler: &handler, params, state };
        match (Rest { layers: &layers, endpoint: &endpoint }).execute(request, connection) {
            Ok(response) => response,
            Err(message) => internal_error(message),
        }
    })
}

// ── Sync groups ──────────────────────────────────────────────────────────────

/// Routes under one path prefix for [`AppWithState`](crate::state::AppWithState),
/// sharing guards and middleware. Made by `app.group(prefix, |g| …)`.
pub struct Group<S> {
    prefix: String,
    guards: Vec<Guard<S>>,
    layers: Vec<Arc<dyn Middleware>>,
    routes: Vec<GroupRoute<SyncHandler<S>>>,
}

impl<S: Send + Sync + 'static> Group<S> {
    pub(crate) fn new(prefix: &str) -> Self {
        Group { prefix: prefix.to_string(), guards: Vec::new(), layers: Vec::new(), routes: Vec::new() }
    }

    fn push(mut self, method: &'static str, pattern: &str, handler: SyncHandler<S>) -> Self {
        self.routes.push(GroupRoute { method, pattern: join_path(&self.prefix, pattern), name: None, handler });
        self
    }

    /// Run `guard` before every route of this group (and its nested groups).
    pub fn guard<G>(mut self, guard: G) -> Self
    where
        G: Fn(&Request, &ConnectionInfo, &S) -> Result<(), Response> + Send + Sync + 'static,
    {
        self.guards.push(Arc::new(guard));
        self
    }

    /// Run `layer` around every route of this group, inside its guards;
    /// the first layer added is the outermost.
    pub fn wrap<M: Middleware + 'static>(mut self, layer: M) -> Self {
        self.layers.push(Arc::new(layer));
        self
    }

    /// Name the route registered last, for [`url_for`](crate::router::url_for).
    pub fn name(mut self, name: &str) -> Self {
        if let Some(route) = self.routes.last_mut() {
            route.name = Some(name.to_string());
        }
        self
    }

    /// Register a `GET` handler at `prefix + pattern`.
    pub fn get<F>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(&Request, &PathParams, &ConnectionInfo, &S) -> Response + Send + Sync + 'static,
    {
        self.push("GET", pattern, Arc::new(move |r, p, c, s| handler(r, p, c, s)))
    }

    /// Register a `POST` handler at `prefix + pattern`.
    pub fn post<F>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(&Request, &PathParams, &ConnectionInfo, &S) -> Response + Send + Sync + 'static,
    {
        self.push("POST", pattern, Arc::new(move |r, p, c, s| handler(r, p, c, s)))
    }

    /// Register a `PUT` handler at `prefix + pattern`.
    pub fn put<F>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(&Request, &PathParams, &ConnectionInfo, &S) -> Response + Send + Sync + 'static,
    {
        self.push("PUT", pattern, Arc::new(move |r, p, c, s| handler(r, p, c, s)))
    }

    /// Register a `PATCH` handler at `prefix + pattern`.
    pub fn patch<F>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(&Request, &PathParams, &ConnectionInfo, &S) -> Response + Send + Sync + 'static,
    {
        self.push("PATCH", pattern, Arc::new(move |r, p, c, s| handler(r, p, c, s)))
    }

    /// Register a `DELETE` handler at `prefix + pattern`.
    pub fn delete<F>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(&Request, &PathParams, &ConnectionInfo, &S) -> Response + Send + Sync + 'static,
    {
        self.push("DELETE", pattern, Arc::new(move |r, p, c, s| handler(r, p, c, s)))
    }

    /// Register a typed handler (`#[get("/path")]` & co.) at
    /// `prefix + its pattern`.
    pub fn route<H: crate::handler::Handler<S>>(self, handler: H) -> Self {
        self.push(H::METHOD, H::PATTERN, Arc::new(move |r, p, c, s| handler.handle(r, p, c, s)))
    }

    /// A nested group at `prefix + pattern`: its routes run this group's
    /// guards and middleware first, then its own.
    pub fn group(mut self, pattern: &str, build: impl FnOnce(Group<S>) -> Group<S>) -> Self {
        let inner = build(Group::new(&join_path(&self.prefix, pattern)));
        self.routes.extend(inner.finish());
        self
    }

    /// The conventional routes of a resource at `prefix + path` (see the
    /// module docs).
    pub fn resource(mut self, path: &str, build: impl FnOnce(Resource<S>) -> Resource<S>) -> Self {
        let resource = build(Resource::new(&join_path(&self.prefix, path)));
        self.routes.extend(resource.finish());
        self
    }

    /// The routes with this group's guards and middleware applied.
    pub(crate) fn finish(self) -> Vec<GroupRoute<SyncHandler<S>>> {
        let guards: Arc<[Guard<S>]> = self.guards.into();
        let layers: Arc<[Arc<dyn Middleware>]> = self.layers.into();
        self.routes
            .into_iter()
            .map(|route| GroupRoute { handler: wrap_sync(route.handler, Arc::clone(&guards), Arc::clone(&layers)), ..route })
            .collect()
    }
}

/// The conventional routes of a collection (see the module docs). Made by
/// `app.resource(path, |r| …)` or a group's `resource`.
pub struct Resource<S> {
    group: Group<S>,
    name: String,
    actions: Vec<(&'static str, &'static str, &'static str, SyncHandler<S>)>,
}

macro_rules! resource_action {
    ($(#[$doc:meta])* $fn:ident, $action:literal, $method:literal, $suffix:literal) => {
        $(#[$doc])*
        pub fn $fn<F>(mut self, handler: F) -> Self
        where
            F: Fn(&Request, &PathParams, &ConnectionInfo, &S) -> Response + Send + Sync + 'static,
        {
            self.actions.push(($action, $method, $suffix, Arc::new(move |r, p, c, s| handler(r, p, c, s))));
            self
        }
    };
}

impl<S: Send + Sync + 'static> Resource<S> {
    fn new(path: &str) -> Self {
        Resource { name: resource_name(path), group: Group::new(path), actions: Vec::new() }
    }

    /// The route names' prefix (default: the path's last segment).
    pub fn name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    /// Run `guard` before every route of this resource.
    pub fn guard<G>(mut self, guard: G) -> Self
    where
        G: Fn(&Request, &ConnectionInfo, &S) -> Result<(), Response> + Send + Sync + 'static,
    {
        self.group = self.group.guard(guard);
        self
    }

    /// Run `layer` around every route of this resource.
    pub fn wrap<M: Middleware + 'static>(mut self, layer: M) -> Self {
        self.group = self.group.wrap(layer);
        self
    }

    resource_action!(/// `GET /posts`: the list.
        index, "index", "GET", "");
    resource_action!(/// `GET /posts/new`: the form for a new one.
        new_form, "new", "GET", "/new");
    resource_action!(/// `POST /posts`: create one.
        create, "create", "POST", "");
    resource_action!(/// `GET /posts/{id}`: one.
        show, "show", "GET", "/{id}");
    resource_action!(/// `GET /posts/{id}/edit`: the form to change one.
        edit, "edit", "GET", "/{id}/edit");
    resource_action!(/// `PUT` and `PATCH /posts/{id}`: change one.
        update, "update", "PUT", "/{id}");
    resource_action!(/// `DELETE /posts/{id}`: delete one.
        destroy, "destroy", "DELETE", "/{id}");

    /// More routes under the resource's path (`/posts/{id}/publish`), with
    /// its guards.
    pub fn extra(mut self, build: impl FnOnce(Group<S>) -> Group<S>) -> Self {
        let prefix = self.group.prefix.clone();
        let inner = build(Group::new(&prefix));
        self.group.routes.extend(inner.finish());
        self
    }

    fn finish(mut self) -> Vec<GroupRoute<SyncHandler<S>>> {
        // `new` before `{id}`, whatever order the builder calls came in.
        let order = ["index", "new", "create", "show", "edit", "update", "destroy"];
        self.actions.sort_by_key(|(action, ..)| order.iter().position(|a| a == action));
        let mut group = self.group;
        let mut extra = std::mem::take(&mut group.routes);
        for (action, method, suffix, handler) in self.actions {
            let pattern = join_path(&group.prefix, suffix);
            let name = Some(format!("{}.{}", self.name, action));
            if action == "update" {
                group.routes.push(GroupRoute { method: "PATCH", pattern: pattern.clone(), name: None, handler: Arc::clone(&handler) });
            }
            group.routes.push(GroupRoute { method, pattern, name, handler });
        }
        group.routes.append(&mut extra);
        group.finish()
    }
}

// ── Async groups ─────────────────────────────────────────────────────────────

#[cfg(feature = "http2")]
pub use self::r#async::{AsyncGroup, AsyncResource};
#[cfg(feature = "http2")]
pub(crate) use self::r#async::AsyncHandlerFn;

#[cfg(feature = "http2")]
mod r#async {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;

    use super::{join_path, resource_name, Guard, GroupRoute};
    use crate::request::Request;
    use crate::response::Response;
    use crate::router::PathParams;
    use crate::server::ConnectionInfo;

    pub(crate) type BoxFuture = Pin<Box<dyn Future<Output = Response> + Send + 'static>>;
    /// An async handler as groups store it.
    pub(crate) type AsyncHandlerFn<S> = Arc<dyn Fn(Request, PathParams, ConnectionInfo, Arc<S>) -> BoxFuture + Send + Sync>;

    fn wrap_async<S: Send + Sync + 'static>(handler: AsyncHandlerFn<S>, guards: Arc<[Guard<S>]>) -> AsyncHandlerFn<S> {
        if guards.is_empty() {
            return handler;
        }
        Arc::new(move |request, params, connection, state| {
            for guard in guards.iter() {
                if let Err(response) = guard(&request, &connection, &state) {
                    return Box::pin(async move { response });
                }
            }
            handler(request, params, connection, state)
        })
    }

    /// [`Group`](super::Group) for `AsyncAppWithState`: async handlers,
    /// guards (sync checks before the handler runs). Middleware wraps the
    /// whole app there (`app.wrap(layer)`), not a group.
    pub struct AsyncGroup<S> {
        prefix: String,
        guards: Vec<Guard<S>>,
        routes: Vec<GroupRoute<AsyncHandlerFn<S>>>,
    }

    impl<S: Send + Sync + 'static> AsyncGroup<S> {
        pub(crate) fn new(prefix: &str) -> Self {
            AsyncGroup { prefix: prefix.to_string(), guards: Vec::new(), routes: Vec::new() }
        }

        fn push(mut self, method: &'static str, pattern: &str, handler: AsyncHandlerFn<S>) -> Self {
            self.routes.push(GroupRoute { method, pattern: join_path(&self.prefix, pattern), name: None, handler });
            self
        }

        fn boxed<F, Fut>(handler: F) -> AsyncHandlerFn<S>
        where
            F: Fn(Request, PathParams, ConnectionInfo, Arc<S>) -> Fut + Send + Sync + 'static,
            Fut: Future<Output = Response> + Send + 'static,
        {
            Arc::new(move |r, p, c, s| Box::pin(handler(r, p, c, s)))
        }

        /// Run `guard` before every route of this group.
        pub fn guard<G>(mut self, guard: G) -> Self
        where
            G: Fn(&Request, &ConnectionInfo, &S) -> Result<(), Response> + Send + Sync + 'static,
        {
            self.guards.push(Arc::new(guard));
            self
        }

        /// Name the route registered last, for [`url_for`](crate::router::url_for).
        pub fn name(mut self, name: &str) -> Self {
            if let Some(route) = self.routes.last_mut() {
                route.name = Some(name.to_string());
            }
            self
        }

        /// Register an async `GET` handler at `prefix + pattern`.
        pub fn get<F, Fut>(self, pattern: &str, handler: F) -> Self
        where
            F: Fn(Request, PathParams, ConnectionInfo, Arc<S>) -> Fut + Send + Sync + 'static,
            Fut: Future<Output = Response> + Send + 'static,
        {
            self.push("GET", pattern, Self::boxed(handler))
        }

        /// Register an async `POST` handler at `prefix + pattern`.
        pub fn post<F, Fut>(self, pattern: &str, handler: F) -> Self
        where
            F: Fn(Request, PathParams, ConnectionInfo, Arc<S>) -> Fut + Send + Sync + 'static,
            Fut: Future<Output = Response> + Send + 'static,
        {
            self.push("POST", pattern, Self::boxed(handler))
        }

        /// Register an async `PUT` handler at `prefix + pattern`.
        pub fn put<F, Fut>(self, pattern: &str, handler: F) -> Self
        where
            F: Fn(Request, PathParams, ConnectionInfo, Arc<S>) -> Fut + Send + Sync + 'static,
            Fut: Future<Output = Response> + Send + 'static,
        {
            self.push("PUT", pattern, Self::boxed(handler))
        }

        /// Register an async `PATCH` handler at `prefix + pattern`.
        pub fn patch<F, Fut>(self, pattern: &str, handler: F) -> Self
        where
            F: Fn(Request, PathParams, ConnectionInfo, Arc<S>) -> Fut + Send + Sync + 'static,
            Fut: Future<Output = Response> + Send + 'static,
        {
            self.push("PATCH", pattern, Self::boxed(handler))
        }

        /// Register an async `DELETE` handler at `prefix + pattern`.
        pub fn delete<F, Fut>(self, pattern: &str, handler: F) -> Self
        where
            F: Fn(Request, PathParams, ConnectionInfo, Arc<S>) -> Fut + Send + Sync + 'static,
            Fut: Future<Output = Response> + Send + 'static,
        {
            self.push("DELETE", pattern, Self::boxed(handler))
        }

        /// Register a typed handler (`#[get("/path")]` & co.) at
        /// `prefix + its pattern`.
        pub fn route<H: crate::handler::AsyncHandler<S>>(self, handler: H) -> Self {
            let handler = Arc::new(handler);
            self.push(H::METHOD, H::PATTERN, Arc::new(move |r, p, c, s| handler.handle_async(r, p, c, s)))
        }

        /// A nested group at `prefix + pattern`.
        pub fn group(mut self, pattern: &str, build: impl FnOnce(AsyncGroup<S>) -> AsyncGroup<S>) -> Self {
            let inner = build(AsyncGroup::new(&join_path(&self.prefix, pattern)));
            self.routes.extend(inner.finish());
            self
        }

        /// The conventional routes of a resource at `prefix + path`.
        pub fn resource(mut self, path: &str, build: impl FnOnce(AsyncResource<S>) -> AsyncResource<S>) -> Self {
            let resource = build(AsyncResource::new(&join_path(&self.prefix, path)));
            self.routes.extend(resource.finish());
            self
        }

        pub(crate) fn finish(self) -> Vec<GroupRoute<AsyncHandlerFn<S>>> {
            let guards: Arc<[Guard<S>]> = self.guards.into();
            self.routes
                .into_iter()
                .map(|route| GroupRoute { handler: wrap_async(route.handler, Arc::clone(&guards)), ..route })
                .collect()
        }
    }

    /// [`Resource`](super::Resource) for `AsyncAppWithState`.
    pub struct AsyncResource<S> {
        group: AsyncGroup<S>,
        name: String,
        actions: Vec<(&'static str, &'static str, &'static str, AsyncHandlerFn<S>)>,
    }

    macro_rules! async_resource_action {
        ($(#[$doc:meta])* $fn:ident, $action:literal, $method:literal, $suffix:literal) => {
            $(#[$doc])*
            pub fn $fn<F, Fut>(mut self, handler: F) -> Self
            where
                F: Fn(Request, PathParams, ConnectionInfo, Arc<S>) -> Fut + Send + Sync + 'static,
                Fut: Future<Output = Response> + Send + 'static,
            {
                self.actions.push(($action, $method, $suffix, AsyncGroup::<S>::boxed(handler)));
                self
            }
        };
    }

    impl<S: Send + Sync + 'static> AsyncResource<S> {
        fn new(path: &str) -> Self {
            AsyncResource { name: resource_name(path), group: AsyncGroup::new(path), actions: Vec::new() }
        }

        /// The route names' prefix (default: the path's last segment).
        pub fn name(mut self, name: &str) -> Self {
            self.name = name.to_string();
            self
        }

        /// Run `guard` before every route of this resource.
        pub fn guard<G>(mut self, guard: G) -> Self
        where
            G: Fn(&Request, &ConnectionInfo, &S) -> Result<(), Response> + Send + Sync + 'static,
        {
            self.group = self.group.guard(guard);
            self
        }

        async_resource_action!(/// `GET /posts`: the list.
            index, "index", "GET", "");
        async_resource_action!(/// `GET /posts/new`: the form for a new one.
            new_form, "new", "GET", "/new");
        async_resource_action!(/// `POST /posts`: create one.
            create, "create", "POST", "");
        async_resource_action!(/// `GET /posts/{id}`: one.
            show, "show", "GET", "/{id}");
        async_resource_action!(/// `GET /posts/{id}/edit`: the form to change one.
            edit, "edit", "GET", "/{id}/edit");
        async_resource_action!(/// `PUT` and `PATCH /posts/{id}`: change one.
            update, "update", "PUT", "/{id}");
        async_resource_action!(/// `DELETE /posts/{id}`: delete one.
            destroy, "destroy", "DELETE", "/{id}");

        /// More routes under the resource's path, with its guards.
        pub fn extra(mut self, build: impl FnOnce(AsyncGroup<S>) -> AsyncGroup<S>) -> Self {
            let prefix = self.group.prefix.clone();
            let inner = build(AsyncGroup::new(&prefix));
            self.group.routes.extend(inner.finish());
            self
        }

        fn finish(mut self) -> Vec<GroupRoute<AsyncHandlerFn<S>>> {
            let order = ["index", "new", "create", "show", "edit", "update", "destroy"];
            self.actions.sort_by_key(|(action, ..)| order.iter().position(|a| a == action));
            let mut group = self.group;
            let mut extra = std::mem::take(&mut group.routes);
            for (action, method, suffix, handler) in self.actions {
                let pattern = join_path(&group.prefix, suffix);
                let name = Some(format!("{}.{}", self.name, action));
                if action == "update" {
                    group.routes.push(GroupRoute { method: "PATCH", pattern: pattern.clone(), name: None, handler: Arc::clone(&handler) });
                }
                group.routes.push(GroupRoute { method, pattern, name, handler });
            }
            group.routes.append(&mut extra);
            group.finish()
        }
    }
}
