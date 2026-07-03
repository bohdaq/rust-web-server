---
title: Test Client
description: Dispatch HTTP requests directly through your Application without opening a TCP socket.
---

`TestClient<A>` wraps any type that implements `Application` and dispatches requests through `Application::execute` in-process. No network, no port binding, no spawned threads — tests run as fast as plain function calls.

## Creating a client

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::test_client::TestClient;

let client = TestClient::new(App::new());
```

Every request is dispatched on a synthetic `127.0.0.1:12345 → 127.0.0.1:7878` connection. Pass any `Application` implementation — `App`, `AppWithState`, `WithMiddleware`, `McpServer`, or your own type.

## State-aware applications

```rust
use std::sync::Arc;
use rust_web_server::app::App;
use rust_web_server::test_client::TestClient;
use rust_web_server::routes;

struct AppState { counter: u32 }

let state = Arc::new(AppState { counter: 42 });
let app = routes! {
    App::with_state(Arc::clone(&state)),
    GET "/count" => |_req, _params, _conn, s: &Arc<AppState>| {
        // ... build response
    },
};
let client = TestClient::new(app);
```

## Building requests

Each HTTP method has a corresponding builder method on `TestClient`:

```rust
client.get("/users")
client.post("/users")
client.put("/users/1")
client.patch("/users/1")
client.delete("/users/1")
client.options("/users")
```

All return a `TestRequest` that you configure with chained builder calls before calling `.send()`.

### Adding headers

```rust
let resp = client.get("/api/data")
    .header("Authorization", "Bearer my-token")
    .header("Accept", "application/json")
    .send();
```

### Setting the body

```rust
// Raw bytes
let resp = client.post("/upload")
    .header("Content-Type", "application/octet-stream")
    .body_bytes(vec![0x89, 0x50, 0x4E, 0x47])
    .send();

// UTF-8 text
let resp = client.post("/echo")
    .header("Content-Type", "text/plain")
    .body_text("hello world")
    .send();

// JSON (set Content-Type yourself)
let resp = client.post("/users")
    .header("Content-Type", "application/json")
    .body_text(r#"{"name":"Alice","email":"alice@example.com"}"#)
    .send();
```

## Reading the response

`.send()` returns a `TestResponse`:

```rust
let resp = client.get("/healthz").send();

resp.status()        // i16 — e.g. 200
resp.reason()        // &str — e.g. "OK"
resp.is_success()    // bool — true when 200–299
resp.body_text()     // &str — panics if body is not valid UTF-8
resp.body_bytes()    // &[u8] — raw body
resp.header("content-type")   // Option<&str> — case-insensitive lookup
resp.headers()       // &[Header] — all response headers
```

## Testing middleware

Wrap any application with middleware before handing it to `TestClient`:

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::rate_limit::RateLimitLayer;
use rust_web_server::test_client::TestClient;

let app = App::new().wrap(RateLimitLayer::new(5, 60)); // 5 req / 60 s
let client = TestClient::new(app);

// First five requests succeed
for _ in 0..5 {
    assert_eq!(200, client.get("/healthz").send().status());
}

// Sixth request is rate-limited
assert_eq!(429, client.get("/healthz").send().status());
```

## Complete CRUD test suite

```rust
#[cfg(test)]
mod tests {
    use rust_web_server::app::App;
    use rust_web_server::core::New;
    use rust_web_server::test_client::TestClient;

    fn make_client() -> TestClient<App> {
        TestClient::new(App::new())
    }

    #[test]
    fn health_check_returns_200() {
        let client = make_client();
        let resp = client.get("/healthz").send();
        assert_eq!(200, resp.status());
    }

    #[test]
    fn unknown_route_returns_404() {
        let client = make_client();
        let resp = client.get("/does-not-exist").send();
        assert_eq!(404, resp.status());
    }

    #[test]
    fn post_creates_resource() {
        let client = make_client();
        let resp = client
            .post("/users")
            .header("Content-Type", "application/json")
            .body_text(r#"{"name":"Alice","email":"alice@example.com"}"#)
            .send();
        assert_eq!(201, resp.status());
        assert!(resp.header("location").is_some());
    }

    #[test]
    fn get_returns_json_content_type() {
        let client = make_client();
        let resp = client
            .get("/users/1")
            .header("Accept", "application/json")
            .send();
        assert_eq!(200, resp.status());
        let ct = resp.header("content-type").unwrap_or("");
        assert!(ct.contains("application/json"), "content-type was: {ct}");
    }

    #[test]
    fn put_updates_resource() {
        let client = make_client();
        let resp = client
            .put("/users/1")
            .header("Content-Type", "application/json")
            .body_text(r#"{"name":"Alice Smith"}"#)
            .send();
        assert_eq!(200, resp.status());
    }

    #[test]
    fn delete_removes_resource() {
        let client = make_client();
        let resp = client.delete("/users/1").send();
        assert!(resp.status() == 200 || resp.status() == 204);
    }

    #[test]
    fn body_text_is_accessible() {
        let client = make_client();
        let resp = client.get("/healthz").send();
        let body = resp.body_text();
        // body is a &str; check it is non-empty or contains expected content
        assert!(!body.is_empty() || resp.status() == 200);
    }
}
```

## Isolated CORS and security-header tests

By default, `App::new()` reads CORS, CSP, and other settings from `RWS_CONFIG_*` environment variables on each request. That means tests which set those env vars can race against each other when `cargo test` runs in parallel — and any test that calls `override_environment_variables_from_config` must hold `test_env::lock()` for its duration.

For tests that only verify header behavior, the cleaner approach is `App::with_config`:

```rust
use rust_web_server::app::App;
use rust_web_server::server_config::ServerConfig;
use rust_web_server::test_client::TestClient;
use rust_web_server::header::Header;

#[test]
fn cors_denied_for_unlisted_origin() {
    // No env writes → no test_env::lock() → runs safely in parallel.
    let config = ServerConfig {
        cors_allow_all: false,
        cors_allow_origins: String::new(),
        ..ServerConfig::default()
    };
    let client = TestClient::new(App::with_config(config));

    let resp = client
        .options("/static/file.png")
        .header(Header::_ORIGIN, "https://evil.example.com")
        .send();

    assert!(resp.header("access-control-allow-origin").is_none());
}

#[test]
fn cors_passes_for_listed_origin() {
    let config = ServerConfig {
        cors_allow_all: false,
        cors_allow_origins: "https://trusted.example.com".to_string(),
        cors_allow_credentials: "true".to_string(),
        ..ServerConfig::default()
    };
    let client = TestClient::new(App::with_config(config));

    let resp = client
        .options("/static/file.png")
        .header(Header::_ORIGIN, "https://trusted.example.com")
        .header(Header::_ACCESS_CONTROL_REQUEST_METHOD, "GET")
        .send();

    assert_eq!(
        "https://trusted.example.com",
        resp.header("access-control-allow-origin").unwrap_or("")
    );
}
```

`App::with_config(config)` pins the app to a fixed `ServerConfig` for the lifetime of the `App` instance — SIGHUP and `POST /admin/config/reload` do not affect it. `App::new()` (no pinned config) continues to read env vars per request, which is correct for production and for tests that specifically verify hot-reload behavior.

:::note[No TCP overhead]
Because `TestClient` bypasses the network stack entirely, tests do not need `#[tokio::test]`, `async`, or any port allocation. They run in plain `#[test]` functions and complete in microseconds.
:::

:::note[Error responses]
If `Application::execute` returns `Err(String)`, `TestClient` converts it to a synthetic `500 Internal Server Error` response whose body is the error message. This means error paths are testable with the same `resp.status()` assertions.
:::
