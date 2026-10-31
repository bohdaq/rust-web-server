---
title: Early Hints
description: Send 103 Early Hints so browsers start loading stylesheets, scripts and connections while the page is still being made.
---

A `103 Early Hints` response (RFC 8297) carries `Link` headers ahead of the real response. A browser that gets one starts preloading the listed stylesheets and scripts, or connecting to the listed origins, while the server is still working on the page: a slow database query or upstream no longer delays those downloads. Since 18.12.0.

## In code

`EarlyHintsLayer` sends the same hints for every request it wraps:

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::early_hints::EarlyHintsLayer;

let app = App::new().wrap(EarlyHintsLayer::new([
    "</app.css>; rel=preload; as=style",
    "</app.js>; rel=preload; as=script",
    "<https://cdn.example.com>; rel=preconnect",
]));
```

A handler or middleware that only knows the hints once it sees the request calls `early_hints::send` before it returns. The `103` goes out at once and the final response follows:

```rust
use rust_web_server::early_hints;

// inside a handler, before the slow part
early_hints::send(&["</product.css>; rel=preload; as=style"]);
```

`send` returns `false` and does nothing when there is nowhere to send the hint: under `TestClient`, for an app called directly, on HTTP/1.1 unless enabled (below), on HTTP/1.0, after `early_hints::MAX_PER_REQUEST` (4) `103`s for the request, or for a value that is empty or contains CR, LF or NUL. `early_hints::available()` says whether a `send` would reach the client.

## Protocols

| Connection | Sent |
|---|---|
| HTTP/2, HTTP/3 | Yes |
| HTTP/1.1 | Only with `early_hints_http1 = true` (`RWS_CONFIG_EARLY_HINTS_HTTP1`) |
| HTTP/1.0 | Never |

Browsers act on `103` only over HTTP/2 and HTTP/3, and some older HTTP/1.1 clients mishandle a `1xx` they did not ask for, so HTTP/1.1 is opt-in. The `http1`-only build runs the handler on the thread that owns the socket and writes its hints just before the response. HTTP/1.1 requests with a streamed body (a large upload read while the handler runs) get no hints.
