[Read Me](README.md) > Ideas

# Ideas

Implementation hints and recommendations for evolving `rust-web-server` toward framework-level usability. Items follow the priority order from [FRAMEWORK_ROADMAP.md](FRAMEWORK_ROADMAP.md).

---

## Start here: the quick wins

### Merge duplicate dispatch

`App::execute` and `App::handle_request` are identical if/else chains over the same controllers. Before touching anything else, remove `handle_request` and route tests through `execute` directly. This reduces every subsequent change from two edits to one.

### `ConnectionInfo` → `SocketAddr`

Replace `client: Address` / `server: Address` (String + i32 fields) with `std::net::SocketAddr`. It is already available everywhere — `TcpListener::accept()` returns it. One afternoon of refactoring with a clear before/after and no design decisions.

---

## 1. Shared state

Remove `Copy` from `App` and add a generic state parameter:

```rust
pub struct App<S = ()> {
    state: Arc<S>,
}
```

State must be `Send + Sync + 'static`. Pass `Arc<S>` into `Application::execute` alongside the request. Everything else — routing, middleware, extractors — builds on top of this. Do not add those before state works.

**Thread-safety note:** `Arc<T>` is read-only sharing. For mutable shared state use `Arc<Mutex<T>>` or `Arc<RwLock<T>>`. Prefer `RwLock` for read-heavy data (connection pools, config).

---

## 2. Dynamic routing

Do not write a router from scratch. The [`matchit`](https://crates.io/crates/matchit) crate is a radix-trie router used by axum. It is tiny, has no dependencies, and handles `:param` and `*wildcard` segments.

```toml
# Cargo.toml
matchit = "0.8"
```

```rust
let mut router: matchit::Router<Box<dyn Handler>> = matchit::Router::new();
router.insert("/users/:id", Box::new(UserController::show))?;

// in dispatch:
let matched = router.at(request.request_uri.as_str())?;
let id = matched.params.get("id").unwrap();
matched.value.handle(&request, id, &state)
```

Build one router per HTTP method (`GET`, `POST`, etc.) stored in a `HashMap<Method, matchit::Router<…>>`. The existing if/else chain in `App::execute` can remain as a fallback for built-in controllers while routes are migrated over incrementally.

---

## 3. Middleware

Tower's `Service`/`Layer` is the gold standard but has a steep learning curve. For a first pass, a simple ordered `Vec` is sufficient and composable:

```rust
pub trait Middleware: Send + Sync {
    fn handle(&self, req: &Request, next: &dyn Fn(&Request) -> Response) -> Response;
}

pub struct App<S> {
    state: Arc<S>,
    middleware: Vec<Arc<dyn Middleware>>,
    router: Router<S>,
}
```

Dispatch walks the `Vec`. Each middleware calls `next` to continue, or short-circuits by returning a response directly — an `AuthMiddleware` returns 401 without calling `next`. Upgrade to Tower later when ecosystem interop is needed.

---

## 4. HTTP/1.1 keep-alive

After writing a response in `Server::process`, check the `Connection` header. HTTP/1.1 is persistent by default — loop back and read the next request on the same stream unless `Connection: close` was sent. Add a per-idle-request timeout so stalled clients do not hold threads indefinitely:

```rust
loop {
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    let request = match read_request(&mut stream) {
        Ok(r)  => r,
        Err(_) => break,  // timeout or client closed
    };
    let keep_alive = should_keep_alive(&request);
    let response = app.execute(&request, &connection)?;
    write_response(&mut stream, response)?;
    if !keep_alive { break; }
}
```

---

## 5. Async handlers

The thread pool (`Server::run`) and the TLS async path (`Server::run_tls`) are two code paths with different concurrency models. Carrying everything on tokio removes the 200-thread ceiling and lets handlers `await` database calls. The plain HTTP/1.1 and TLS paths then share the same handler code — only the stream type differs.

```rust
// replace the thread pool accept loop
let listener = tokio::net::TcpListener::bind(addr).await?;
loop {
    let (stream, peer) = listener.accept().await?;
    let app = app.clone();
    tokio::spawn(async move {
        handle_connection(stream, peer, app).await;
    });
}
```

Handler trait with async:

```rust
pub trait Handler<S>: Send + Sync {
    fn call(&self, req: Request, state: Arc<S>) -> BoxFuture<'_, Response>;
}
```

---

## 6. Typed request extractors

Add `serde` and define a `FromRequest` trait. Handlers then receive typed values instead of a raw `&Request`.

```toml
# Cargo.toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

```rust
pub trait FromRequest: Sized {
    type Error: IntoResponse;
    fn from_request(req: &Request) -> Result<Self, Self::Error>;
}

pub struct Json<T>(pub T);

impl<T: serde::de::DeserializeOwned> FromRequest for Json<T> {
    type Error = Response;
    fn from_request(req: &Request) -> Result<Self, Self::Error> {
        serde_json::from_slice(&req.body)
            .map(Json)
            .map_err(|_| /* 400 response */ todo!())
    }
}
```

Implement the same pattern for `Query<T>` (URL query string via `serde_urlencoded`), `Path<T>` (router params), and `Form<T>` (URL-encoded body).

---

## 7. Typed error handling

Add an `IntoResponse` trait and change `Application::execute` to return `Result<Response, Box<dyn IntoResponse>>`. Application errors implement the trait and carry their own HTTP status code — the framework calls `.into_response()` on `Err` automatically.

```rust
pub trait IntoResponse {
    fn into_response(self) -> Response;
}

impl IntoResponse for Response {
    fn into_response(self) -> Response { self }
}

// application error example
enum AppError {
    NotFound(String),
    Unauthorized,
    Internal(Box<dyn std::error::Error>),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::NotFound(msg)  => /* 404 */ todo!(),
            AppError::Unauthorized   => /* 401 */ todo!(),
            AppError::Internal(_)    => /* 500 */ todo!(),
        }
    }
}
```

---

## 8. Streaming responses

Replace `ContentRange.body: Vec<u8>` with a `Body` enum so large files are not fully buffered before sending:

```rust
pub enum Body {
    Bytes(Vec<u8>),
    Stream(Box<dyn Read + Send>),
    // after migrating to tokio:
    // AsyncStream(Pin<Box<dyn AsyncRead + Send>>),
}
```

For HTTP/1.1, write the stream as `Transfer-Encoding: chunked`:

```
<hex-length>\r\n
<chunk bytes>\r\n
...
0\r\n
\r\n
```

For HTTP/2 and HTTP/3, the protocol frames data natively — feed chunks to the send stream incrementally.

---

## Recommended order

| Step | What | Why first |
|------|------|-----------|
| 1 | Merge duplicate dispatch | Reduces work for every subsequent step |
| 2 | `ConnectionInfo` → `SocketAddr` | Trivial; unblocks logging and rate-limiting |
| 3 | Shared state (`Arc<S>` in `App`) | Everything else depends on this |
| 4 | `matchit` router + path params | Unlocks real API routes |
| 5 | `IntoResponse` + typed errors | Clean error handling before more routes are added |
| 6 | HTTP/1.1 keep-alive | High impact, self-contained change |
| 7 | Middleware `Vec` | Cross-cutting concerns, auth |
| 8 | Unify on tokio | Async handlers, removes thread-pool ceiling |
| 9 | `FromRequest` extractors | After routing and state are stable |
| 10 | `Body::Stream` + chunked | After async path is in place |
