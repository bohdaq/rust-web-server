//! Lightweight type-keyed dependency injection container.
//!
//! [`Container`] stores services keyed by their Rust type (`TypeId`). Register
//! services at startup; resolve them anywhere that has access to the container.
//! Share across request handlers with `App::with_state(container)` — see
//! "Usage with `App::with_state`" below.
//!
//! # Concrete services
//!
//! ```rust
//! # extern crate rws_server as rust_web_server;
//! use rust_web_server::di::Container;
//!
//! struct EmailService { host: String }
//!
//! let mut c = Container::new();
//! c.register(EmailService { host: "smtp.example.com".into() });
//!
//! let svc = c.get::<EmailService>().unwrap();
//! assert_eq!(svc.host, "smtp.example.com");
//! ```
//!
//! # Trait-object services
//!
//! ```rust
//! # extern crate rws_server as rust_web_server;
//! use std::sync::Arc;
//! use rust_web_server::di::Container;
//!
//! pub trait Mailer: Send + Sync {
//!     fn send(&self, to: &str);
//! }
//! struct SmtpMailer;
//! impl Mailer for SmtpMailer {
//!     fn send(&self, _to: &str) {}
//! }
//!
//! let mut c = Container::new();
//! c.provide::<dyn Mailer>(Arc::new(SmtpMailer));
//! let mailer = c.get::<dyn Mailer>().unwrap();
//! mailer.send("user@example.com");
//! ```
//!
//! # Named services
//!
//! Register multiple instances of the same type under distinct string names:
//!
//! ```rust
//! # extern crate rws_server as rust_web_server;
//! use rust_web_server::di::Container;
//!
//! let mut c = Container::new();
//! c.register_named("primary",  5432u16)
//!  .register_named("replica",  5433u16);
//!
//! assert_eq!(*c.get_named::<u16>("primary").unwrap(), 5432);
//! assert_eq!(*c.get_named::<u16>("replica").unwrap(), 5433);
//! ```
//!
//! # Usage with `App::with_state` / `App::with_async_state`
//!
//! `Container` needs no special-cased integration — it's `Send + Sync +
//! 'static` like any other state type, so it plugs directly into
//! [`App::with_state`](crate::app::App::with_state) as `S`. Pass the
//! container itself, **not** `container.into_arc()`: `with_state` already
//! wraps `S` in an `Arc` internally, so calling `into_arc()` first just
//! double-wraps it (`Arc<Arc<Container>>`) for no benefit — handlers then
//! receive `&Container` directly instead of `&Arc<Container>`. `into_arc()`
//! exists for call sites that need to share one container across multiple
//! `Application`s built by hand, outside of `App::with_state`/`with_async_state`.
//!
//! ```rust,no_run
//! # extern crate rws_server as rust_web_server;
//! use rust_web_server::app::App;
//! use rust_web_server::di::Container;
//! use rust_web_server::request::Request;
//! use rust_web_server::router::PathParams;
//! use rust_web_server::server::ConnectionInfo;
//! use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
//!
//! struct Config { version: &'static str }
//!
//! fn get_version(
//!     _req: &Request,
//!     _p: &PathParams,
//!     _c: &ConnectionInfo,
//!     state: &Container,
//! ) -> Response {
//!     let _cfg = state.get::<Config>().unwrap();
//!     Response::get_response(&STATUS_CODE_REASON_PHRASE.n200_ok, None, None)
//! }
//!
//! let mut container = Container::new();
//! container.register(Config { version: "1.0" });
//!
//! let app = App::with_state(container).get("/version", get_version);
//! ```
//!
//! The async counterpart works the same way with [`App::with_async_state`](crate::app::App::with_async_state)
//! (requires the `http2` feature) — handlers are `async fn`s that receive
//! `&Container` and can `.await` inside the closure while resolving services.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

/// A type-keyed service container for dependency injection.
///
/// See the [module documentation][self] for usage examples.
pub struct Container {
    /// Unnamed services: key = TypeId of T, value = Box<dyn Any> holding Arc<T>.
    services: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    /// Named services: key = (TypeId of T, name), value = Box<dyn Any> holding Arc<T>.
    named: HashMap<(TypeId, String), Box<dyn Any + Send + Sync>>,
    /// Lifecycle hooks, in registration order (18.35.0).
    hooks: Vec<Hook>,
    /// How many hooks [`start`](Container::start) ran; [`stop`](Container::stop) undoes those.
    /// Behind `Arc`s: a container stays `Freeze` (no interior mutability inline).
    started: Arc<std::sync::atomic::AtomicUsize>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

type StartHook = Box<dyn Fn(&Container) -> Result<(), String> + Send + Sync>;
type StopHook = Box<dyn Fn(&Container) + Send + Sync>;

struct Hook {
    name: String,
    start: Option<StartHook>,
    stop: Option<StopHook>,
}

/// A service with a start and a stop: a connection pool to warm, a
/// background thread to join (18.35.0). Register it with
/// [`Container::register_managed`]; both default to doing nothing.
pub trait Lifecycle: Send + Sync {
    /// Called by [`Container::start`], in registration order.
    fn start(&self) -> Result<(), String> {
        Ok(())
    }

    /// Called by [`Container::stop`], in reverse order — only if `start`
    /// succeeded.
    fn stop(&self) {}
}

impl Default for Container {
    fn default() -> Self {
        Self::new()
    }
}

impl Container {
    /// Create an empty container.
    pub fn new() -> Self {
        Container {
            services: HashMap::new(),
            named: HashMap::new(),
            hooks: Vec::new(),
            started: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    // ── Registration ────────────────────────────────────────────────────────────

    /// Register a concrete service by value.
    ///
    /// Wraps `service` in `Arc<T>` and stores it under `TypeId::of::<T>()`.
    /// A subsequent registration for the same `T` replaces the previous one.
    pub fn register<T: Send + Sync + 'static>(&mut self, service: T) -> &mut Self {
        self.services
            .insert(TypeId::of::<T>(), Box::new(Arc::new(service)));
        self
    }

    /// Register a pre-built `Arc<T>`.
    ///
    /// Required when registering **trait objects**, because the concrete type
    /// must be erased before storage:
    ///
    /// ```rust
    /// # extern crate rws_server as rust_web_server;
    /// # use std::sync::Arc;
    /// # use rust_web_server::di::Container;
    /// # trait Greeter: Send + Sync { fn greet(&self) -> &str; }
    /// # struct Hello; impl Greeter for Hello { fn greet(&self) -> &str { "hi" } }
    /// let mut c = Container::new();
    /// c.provide::<dyn Greeter>(Arc::new(Hello));
    /// assert_eq!(c.get::<dyn Greeter>().unwrap().greet(), "hi");
    /// ```
    ///
    /// Also usable for concrete types when you already have an `Arc`.
    pub fn provide<T>(&mut self, service: Arc<T>) -> &mut Self
    where
        T: ?Sized + Send + Sync + 'static,
    {
        // Arc<T> is always Sized (fat pointer for DSTs), 'static when T: 'static,
        // Send + Sync when T: Send + Sync, and Any via blanket impl for all 'static.
        let boxed: Box<dyn Any + Send + Sync> = Box::new(service);
        self.services.insert(TypeId::of::<T>(), boxed);
        self
    }

    /// Register a named concrete service.
    ///
    /// Use this when you need multiple instances of the same type
    /// (e.g., primary vs. replica database pools).
    pub fn register_named<T: Send + Sync + 'static>(
        &mut self,
        name: impl Into<String>,
        service: T,
    ) -> &mut Self {
        self.named.insert(
            (TypeId::of::<T>(), name.into()),
            Box::new(Arc::new(service)),
        );
        self
    }

    /// Register a pre-built `Arc<T>` under a string name.
    ///
    /// Use for named trait-object registrations.
    pub fn provide_named<T>(
        &mut self,
        name: impl Into<String>,
        service: Arc<T>,
    ) -> &mut Self
    where
        T: ?Sized + Send + Sync + 'static,
    {
        let boxed: Box<dyn Any + Send + Sync> = Box::new(service);
        self.named.insert((TypeId::of::<T>(), name.into()), boxed);
        self
    }

    // ── Resolution ──────────────────────────────────────────────────────────────

    /// Resolve a service by type.
    ///
    /// Works for both **concrete types** (`get::<MyService>()`) and **trait
    /// objects** (`get::<dyn MyTrait>()`).  Returns `None` if no service of
    /// that type has been registered.
    pub fn get<T>(&self) -> Option<Arc<T>>
    where
        T: ?Sized + 'static,
        Arc<T>: Clone,
    {
        self.services
            .get(&TypeId::of::<T>())
            .and_then(|b| b.downcast_ref::<Arc<T>>())
            .cloned()
    }

    /// Resolve a named service by type and name.
    ///
    /// Returns `None` if no service of that type and name has been registered.
    pub fn get_named<T>(&self, name: &str) -> Option<Arc<T>>
    where
        T: ?Sized + 'static,
        Arc<T>: Clone,
    {
        self.named
            .get(&(TypeId::of::<T>(), name.to_string()))
            .and_then(|b| b.downcast_ref::<Arc<T>>())
            .cloned()
    }

    // ── Inspection ──────────────────────────────────────────────────────────────

    /// Returns `true` if a service of type `T` has been registered (unnamed).
    pub fn contains<T: ?Sized + 'static>(&self) -> bool {
        self.services.contains_key(&TypeId::of::<T>())
    }

    /// Returns `true` if a named service of type `T` with `name` exists.
    pub fn contains_named<T: ?Sized + 'static>(&self, name: &str) -> bool {
        self.named
            .contains_key(&(TypeId::of::<T>(), name.to_string()))
    }

    /// Total number of unnamed registrations.
    pub fn len(&self) -> usize {
        self.services.len()
    }

    /// Returns `true` if no services have been registered.
    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }

    // ── Lifecycle (18.35.0) ──────────────────────────────────────────────────────

    /// Run `hook` at [`start`](Self::start), after the hooks registered
    /// before it. An `Err` stops the start.
    pub fn on_start(&mut self, name: &str, hook: impl Fn(&Container) -> Result<(), String> + Send + Sync + 'static) -> &mut Self {
        self.hooks.push(Hook { name: name.to_string(), start: Some(Box::new(hook)), stop: None });
        self
    }

    /// Run `hook` at [`stop`](Self::stop), before the hooks registered
    /// before it (stops run in reverse).
    pub fn on_stop(&mut self, name: &str, hook: impl Fn(&Container) + Send + Sync + 'static) -> &mut Self {
        self.hooks.push(Hook { name: name.to_string(), start: None, stop: Some(Box::new(hook)) });
        self
    }

    /// Register `service` (as [`register`](Self::register)) with its
    /// [`Lifecycle`] `start`/`stop` as hooks.
    pub fn register_managed<T: Lifecycle + 'static>(&mut self, service: T) -> &mut Self {
        self.register(service);
        let name = std::any::type_name::<T>().to_string();
        self.hooks.push(Hook {
            name,
            start: Some(Box::new(|c: &Container| c.get::<T>().map_or(Ok(()), |s| s.start()))),
            stop: Some(Box::new(|c: &Container| {
                if let Some(s) = c.get::<T>() {
                    s.stop();
                }
            })),
        });
        self
    }

    /// Run the start hooks in registration order. When one fails, the stop
    /// hooks of those before it run (in reverse) and the error names the
    /// hook: `"<name>: <error>"`. Calling it again after a success does
    /// nothing until [`stop`](Self::stop).
    pub fn start(&self) -> Result<(), String> {
        use std::sync::atomic::Ordering;
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }
        for (i, hook) in self.hooks.iter().enumerate() {
            if let Some(start) = &hook.start {
                if let Err(e) = start(self) {
                    self.stop_first(i);
                    return Err(format!("{}: {e}", hook.name));
                }
            }
        }
        self.started.store(self.hooks.len(), Ordering::SeqCst);
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Run the stop hooks of everything [`start`](Self::start) started, in
    /// reverse order. Does nothing when not started; safe to call twice.
    pub fn stop(&self) {
        if self.running.swap(false, std::sync::atomic::Ordering::SeqCst) {
            let n = self.started.swap(0, std::sync::atomic::Ordering::SeqCst);
            self.stop_first(n);
        }
    }

    fn stop_first(&self, n: usize) {
        for hook in self.hooks[..n.min(self.hooks.len())].iter().rev() {
            if let Some(stop) = &hook.stop {
                stop(self);
            }
        }
    }

    /// Whether [`start`](Self::start) succeeded and [`stop`](Self::stop)
    /// hasn't run since.
    pub fn is_started(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }

    // ── Sharing ─────────────────────────────────────────────────────────────────

    /// Seal this container into an `Arc<Container>` for sharing across threads
    /// and handler closures.
    ///
    /// Typical usage with `App::with_state`:
    /// ```rust,no_run
    /// # extern crate rws_server as rust_web_server;
    /// # use rust_web_server::di::Container;
    /// # use rust_web_server::app::App;
    /// let mut c = Container::new();
    /// // c.register(...);
    /// let app = App::with_state(c.into_arc());
    /// ```
    pub fn into_arc(self) -> Arc<Self> {
        Arc::new(self)
    }
}

#[cfg(test)]
mod tests;
