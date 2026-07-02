---
title: Sessions
description: Server-side session management with an in-memory, TTL-aware thread-safe store.
---

`rust-web-server` ships a complete server-side session system in `src/session/mod.rs`. Sessions are stored in an in-memory, thread-safe map with automatic TTL-based expiry. The store is designed to live inside your application state so every handler shares the same session map without any global state.

## Session store

### Creating a store

```rust
use rust_web_server::session::SessionStore;

// Sessions expire after 3600 seconds (1 hour).
let store = SessionStore::new(3600);
```

`SessionStore` is cheaply `Clone`-able — all clones share the same backing map via `Arc`. Place one instance in your application state.

### Creating a session

```rust
// Allocates a new session with a generated ID and inserts it into the store.
let mut session = store.create();
```

For public-facing services that need cryptographically unpredictable IDs, supply your own ID instead:

```rust
let id = my_csprng_hex_id(); // e.g. from `ring` or `getrandom`
let mut session = store.create_with_id(id);
```

:::note[ID generation]
The default `create()` uses a non-cryptographic splitmix64 finalizer seeded from the system clock and an atomic counter. It is sufficient for most internal applications. Replace it with a CSPRNG for public deployments.
:::

### Reading and writing session data

`Session` holds key/value pairs as strings. Mutate the session locally, then call `save` to persist changes.

```rust
session.set("user_id", "42");
session.set("role", "admin");

let user_id: Option<&str> = session.get("user_id"); // Some("42")
let missing: Option<&str> = session.get("absent");  // None

session.remove("role");
let has_role: bool = session.contains("role"); // false

// Persist changes back to the store.
store.save(&session);
```

### Loading a session

```rust
// Returns None if the session is unknown or expired.
let session = store.load(&session_id);
```

### Destroying a session

```rust
store.destroy(&session_id);
```

Call `destroy_cookie` to also clear the browser cookie (see below).

### Purging expired sessions

Sessions are not removed automatically when they expire — they are simply invisible to `load`. To reclaim memory, call `purge_expired()` periodically:

```rust
store.purge_expired(); // removes all sessions whose TTL has elapsed
```

Use the [Scheduler](./scheduler) to run this automatically:

```rust
use std::time::Duration;
use rust_web_server::scheduler::Scheduler;

let store_clone = store.clone();
Scheduler::new()
    .every(Duration::from_secs(3600), move || store_clone.purge_expired())
    .start();
```

## Cookie helpers

Three free functions bridge the session store and the HTTP cookie layer.

### `session_id_from_request`

Extracts the session ID from a named cookie in a request's `Cookie` header.

```rust
use rust_web_server::session;

let sid: Option<String> = session::session_id_from_request(&request, "sid");
```

Returns `None` if the `Cookie` header is absent or the named cookie is missing.

### `session_cookie`

Builds a `Set-Cookie` header value with `HttpOnly`, `SameSite=Lax`, `Path=/`, and `Max-Age`.

```rust
let cookie_value = session::session_cookie(&session.id, "sid", 3600);
response.headers.push(Header {
    name: "Set-Cookie".to_string(),
    value: cookie_value,
});
```

### `destroy_cookie`

Clears the browser cookie by setting `Max-Age=0`.

```rust
let cookie_value = session::destroy_cookie("sid");
response.headers.push(Header {
    name: "Set-Cookie".to_string(),
    value: cookie_value,
});
```

## Complete login/profile example

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::session::{self, SessionStore};
use rust_web_server::header::Header;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};

struct State { sessions: SessionStore }

let app = App::with_state(State { sessions: SessionStore::new(3600) })
    .post("/login", |req, _params, _conn, state| {
        // Verify credentials here ...
        let mut sess = state.sessions.create();
        sess.set("user_id", "42");
        state.sessions.save(&sess);

        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        r.headers.push(Header {
            name: "Set-Cookie".to_string(),
            value: session::session_cookie(&sess.id, "sid", 3600),
        });
        r
    })
    .get("/profile", |req, _params, _conn, state| {
        let mut r = Response::new();

        let sid = match session::session_id_from_request(&req, "sid") {
            Some(id) => id,
            None => {
                r.status_code = *STATUS_CODE_REASON_PHRASE.n401_unauthorized.status_code;
                r.reason_phrase = STATUS_CODE_REASON_PHRASE.n401_unauthorized.reason_phrase.to_string();
                return r;
            }
        };
        let sess = match state.sessions.load(&sid) {
            Some(s) => s,
            None => {
                r.status_code = *STATUS_CODE_REASON_PHRASE.n401_unauthorized.status_code;
                r.reason_phrase = STATUS_CODE_REASON_PHRASE.n401_unauthorized.reason_phrase.to_string();
                return r;
            }
        };

        let _user_id = sess.get("user_id").unwrap_or("guest");
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        r
    })
    .get("/logout", |req, _params, _conn, state| {
        let mut r = Response::new();
        if let Some(sid) = session::session_id_from_request(&req, "sid") {
            state.sessions.destroy(&sid);
        }
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        r.headers.push(Header {
            name: "Set-Cookie".to_string(),
            value: session::destroy_cookie("sid"),
        });
        r
    });
```

## Store size

```rust
let count = store.len();       // total entries, including expired-but-not-purged
let empty = store.is_empty();  // true when no entries exist
```
