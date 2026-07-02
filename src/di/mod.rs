//! Lightweight type-keyed dependency injection container.
//!
//! [`Container`] stores services keyed by their Rust type (`TypeId`). Register
//! services at startup; resolve them anywhere that has access to the container.
//! Share across request handlers by wrapping in `Arc<Container>` and using
//! `App::with_state(container.into_arc())`.
//!
//! # Concrete services
//!
//! ```rust
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
//! # Usage with `App::with_state`
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use rust_web_server::app::App;
//! use rust_web_server::di::Container;
//! use rust_web_server::request::Request;
//! use rust_web_server::router::PathParams;
//! use rust_web_server::server::ConnectionInfo;
//! use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
//! use rust_web_server::routes;
//!
//! struct Config { version: &'static str }
//!
//! fn get_version(
//!     _req: &Request,
//!     _p: &PathParams,
//!     _c: &ConnectionInfo,
//!     state: &Arc<Container>,
//! ) -> Response {
//!     let _cfg = state.get::<Config>().unwrap();
//!     Response::get_response(&STATUS_CODE_REASON_PHRASE.n200_ok, None, None)
//! }
//!
//! let mut container = Container::new();
//! container.register(Config { version: "1.0" });
//!
//! let app = routes! {
//!     App::with_state(container.into_arc()),
//!     GET "/version" => get_version,
//! };
//! ```

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

    // ── Sharing ─────────────────────────────────────────────────────────────────

    /// Seal this container into an `Arc<Container>` for sharing across threads
    /// and handler closures.
    ///
    /// Typical usage with `App::with_state`:
    /// ```rust,no_run
    /// # use rust_web_server::di::Container;
    /// # use rust_web_server::app::App;
    /// # use rust_web_server::routes;
    /// let mut c = Container::new();
    /// // c.register(...);
    /// let app = routes! { App::with_state(c.into_arc()), };
    /// ```
    pub fn into_arc(self) -> Arc<Self> {
        Arc::new(self)
    }
}

#[cfg(test)]
mod tests;
