---
title: Dependency Injection
description: Use Container to register and resolve typed services across request handlers.
---

`Container` is a type-keyed service store backed by `HashMap<TypeId, Box<dyn Any + Send + Sync>>`. Register services at startup, seal the container with `into_arc()`, then share it as application state.

## Registering concrete services

`register::<T>` wraps the value in `Arc<T>` and stores it under `TypeId::of::<T>()`. A second registration for the same type replaces the first.

```rust
use rust_web_server::di::Container;

struct EmailService {
    host: String,
}

let mut container = Container::new();
container.register(EmailService { host: "smtp.example.com".into() });

let svc = container.get::<EmailService>().unwrap();
assert_eq!(svc.host, "smtp.example.com");
```

## Registering trait objects

Use `provide::<dyn Trait>(Arc::new(impl))` when the concrete type must be erased. The key is `TypeId::of::<dyn Trait>()`, so later resolution via `get::<dyn Trait>()` works correctly.

```rust
use std::sync::Arc;
use rust_web_server::di::Container;

pub trait UserRepo: Send + Sync {
    fn find(&self, id: i64) -> Option<String>;
}

struct PgUserRepo;
impl UserRepo for PgUserRepo {
    fn find(&self, _id: i64) -> Option<String> {
        Some("Alice".into())
    }
}

let mut container = Container::new();
container.provide::<dyn UserRepo>(Arc::new(PgUserRepo));

let repo = container.get::<dyn UserRepo>().unwrap();
assert_eq!(repo.find(1), Some("Alice".into()));
```

## Resolving services

Both concrete and trait-object registrations are resolved with the same method:

```rust
// concrete type
let svc: Option<Arc<EmailService>> = container.get::<EmailService>();

// trait object
let repo: Option<Arc<dyn UserRepo>> = container.get::<dyn UserRepo>();
```

`get` returns `None` when no matching registration exists — no panics.

## Named services

Register multiple instances of the same type under distinct string names with `register_named`. This is useful for primary/replica database pools or multiple external services.

```rust
use rust_web_server::di::Container;

let mut container = Container::new();
container
    .register_named("primary", 5432u16)
    .register_named("replica", 5433u16);

assert_eq!(*container.get_named::<u16>("primary").unwrap(), 5432);
assert_eq!(*container.get_named::<u16>("replica").unwrap(), 5433);
```

`provide_named::<dyn Trait>("name", arc)` is the trait-object equivalent.

## Sealing the container

`into_arc()` wraps the container in `Arc<Container>`, making it immutable and safe to clone across threads and handler closures.

```rust
let arc = container.into_arc(); // Arc<Container>
```

Once sealed you cannot add more registrations. Perform all registrations before calling `into_arc()`.

## Wiring into request handlers

Pass the sealed container as application state using `App::with_state`. Each handler receives `state: &Arc<Container>` and can call `state.get::<T>()` to resolve dependencies.

```rust
use std::sync::Arc;
use rust_web_server::app::App;
use rust_web_server::di::Container;
use rust_web_server::request::Request;
use rust_web_server::router::PathParams;
use rust_web_server::server::ConnectionInfo;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::routes;

fn get_version(
    _req: &Request,
    _params: &PathParams,
    _conn: &ConnectionInfo,
    state: &Arc<Container>,
) -> Response {
    let svc = state.get::<EmailService>().unwrap();
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r
}

let mut container = Container::new();
container.register(EmailService { host: "smtp.example.com".into() });

let app = routes! {
    App::with_state(container.into_arc()),
    GET "/version" => get_version,
};
```

## Complete example: UserRepository + DI wiring

```rust
use std::sync::Arc;
use rust_web_server::app::App;
use rust_web_server::di::Container;
use rust_web_server::request::Request;
use rust_web_server::router::PathParams;
use rust_web_server::server::ConnectionInfo;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::routes;

// --- domain trait ---
pub trait UserRepository: Send + Sync {
    fn find_by_id(&self, id: i64) -> Option<String>;
    fn count(&self) -> usize;
}

// --- in-memory implementation (test / dev) ---
pub struct InMemoryUserRepo {
    users: Vec<(i64, String)>,
}

impl UserRepository for InMemoryUserRepo {
    fn find_by_id(&self, id: i64) -> Option<String> {
        self.users.iter().find(|(i, _)| *i == id).map(|(_, n)| n.clone())
    }
    fn count(&self) -> usize {
        self.users.len()
    }
}

// --- handler ---
fn get_user(
    req: &Request,
    params: &PathParams,
    _conn: &ConnectionInfo,
    state: &Arc<Container>,
) -> Response {
    let repo = state.get::<dyn UserRepository>().unwrap();
    let id: i64 = params.get("id").and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut r = Response::new();
    match repo.find_by_id(id) {
        Some(name) => {
            r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
            r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        }
        None => {
            r.status_code = *STATUS_CODE_REASON_PHRASE.n404_not_found.status_code;
            r.reason_phrase = STATUS_CODE_REASON_PHRASE.n404_not_found.reason_phrase.to_string();
        }
    }
    r
}

// --- wiring ---
let repo = InMemoryUserRepo {
    users: vec![(1, "Alice".into()), (2, "Bob".into())],
};

let mut container = Container::new();
container.provide::<dyn UserRepository>(Arc::new(repo));

let app = routes! {
    App::with_state(container.into_arc()),
    GET "/users/:id" => get_user,
};
```

:::note[Thread safety]
Every type registered with `register` or `provide` must implement `Send + Sync + 'static`. `Arc<T>` satisfies those bounds whenever `T: Send + Sync`, making it the natural wrapper for shared services.
:::

## Inspection helpers

```rust
container.contains::<EmailService>();           // unnamed registration exists?
container.contains_named::<u16>("primary");     // named registration exists?
container.len();                                // total unnamed registrations
container.is_empty();                           // no registrations at all?
```
