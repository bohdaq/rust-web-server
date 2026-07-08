---
title: Circuit Breaker & Retry
description: Protect upstream services from cascading failures with a three-state circuit breaker, automatic retry middleware, and optional Redis-backed persistence.
---

The `circuit_breaker` module provides a per-backend state machine that stops forwarding requests to a broken upstream and a `RetryLayer` middleware that automatically re-dispatches failed requests.

## State machine

A `CircuitBreaker` tracks each backend independently and moves through three states:

```
Closed ──(threshold failures)──► Open ──(recovery_secs elapsed)──► HalfOpen
  ▲                                                                      │
  └─────────────────────(probe succeeds)────────────────────────────────┘
                        (probe fails → back to Open)
```

| State | Behaviour |
|-------|-----------|
| `Closed` | All requests are forwarded. Failure counter is incremented on each error. |
| `Open` | All requests are rejected immediately — no TCP connection is attempted. Entered when consecutive failures reach `failure_threshold`. |
| `HalfOpen` | Up to `max_half_open_probes` (default 1) concurrent probe requests are let through after `recovery_secs` have elapsed; further concurrent requests are rejected like `Open` until an outcome resolves. Success closes the circuit; failure re-opens it and resets the recovery timer. |

## Creating a circuit breaker

```rust
use rust_web_server::circuit_breaker::CircuitBreaker;

// threshold=5 consecutive failures, recovery window=30 s
let cb = CircuitBreaker::new(5, 30);

// Optional: raise how many concurrent probes are let through while HalfOpen
// (default 1). Chainable — call before the breaker goes behind a Mutex/Arc.
let cb = CircuitBreaker::new(5, 30).max_half_open_probes(2);
```

:::note[Why cap HalfOpen probes at all]
Before this cap existed, *every* concurrent request arriving the instant a backend transitioned to `HalfOpen` saw `is_available() == true` — a burst of concurrent traffic could all count as "the" trial request at once, defeating the point of testing with a single probe. The cap (default 1) rejects anything beyond it, the same as `Open`, until `record_success`/`record_failure` resolves the in-flight probe(s) and resets the count to 0.
:::

### Global singleton

```rust
use rust_web_server::circuit_breaker;

// Returns a &'static Mutex<CircuitBreaker> (threshold=5, recovery=30 s)
let available = circuit_breaker::global()
    .lock()
    .unwrap()
    .is_available("backend-a:8080");
```

`circuit_breaker::global()` is initialised once via `OnceLock` and shared across the entire process. Acquire the `Mutex` guard before calling any method.

## Methods

```rust
// Returns true if the request should be forwarded
cb.is_available("backend:8080");

// Record the outcome after a request completes
cb.record_success("backend:8080"); // HalfOpen → Closed, counter reset
cb.record_failure("backend:8080"); // increments counter or re-opens

// Inspect current state
cb.state("backend:8080"); // BreakerState::{Closed, Open, HalfOpen}

// Manually reset to Closed
cb.reset("backend:8080");
```

## Persistence (`RedisCircuitBreaker`)

`CircuitBreaker` keeps state in a plain in-process `HashMap`. A restart — or a rolling deploy — resets every backend back to `Closed`, so a backend that tripped the breaker moments before a restart looks healthy again immediately, and can cascade the same failures again before anything notices.

`RedisCircuitBreaker` has the exact same method names and the same Closed → Open → HalfOpen state machine, but persists each backend's state in Redis instead of an in-memory map:

```rust
use rust_web_server::circuit_breaker::RedisCircuitBreaker;

// threshold=5 consecutive failures, recovery window=30s — same as CircuitBreaker::new
let cb = RedisCircuitBreaker::new("127.0.0.1:6379", None, 5, 30);

// Or from RWS_REDIS_HOST/PORT/PASSWORD +
// RWS_CONFIG_CIRCUIT_BREAKER_FAILURE_THRESHOLD/RECOVERY_SECS:
let cb = RedisCircuitBreaker::from_env();
```

Every method is the same as `CircuitBreaker`'s, except each one is a Redis round trip and therefore returns `Result` — the network call can fail:

```rust
match cb.is_available("backend-a:8080") {
    Ok(true)  => { /* forward the request */ }
    Ok(false) => { /* short-circuit — 503 without contacting the backend */ }
    Err(e)    => { /* Redis unreachable — you decide: fail open or fail closed */ }
}

cb.record_success("backend-a:8080")?;
cb.record_failure("backend-a:8080")?;
cb.state("backend-a:8080")?;        // Result<BreakerState, io::Error>
cb.reset("backend-a:8080")?;
cb.set_limits(10, 60);              // update thresholds on a live breaker, no restart needed
```

Because state lives in Redis rather than in the struct, this also gets you circuit-sharing across every `rws` instance pointed at the same Redis server for free — not just restart survival, but a backend that trips the breaker on one instance is immediately seen as open by every other instance too.

:::note[Why Redis, not the model layer]
`DbPool` (the model layer) is `async fn`-only, while `CircuitBreaker`'s methods and `Middleware::handle` (what `RetryLayer` implements) are both synchronous. `RedisCircuitBreaker` reaches Redis over a plain blocking `TcpStream` (the same hand-rolled RESP client `RedisRateLimiter` and `RedisSessionStore` use), so it stays fully synchronous and drops into the same call sites `CircuitBreaker` already has, with no new Cargo dependency.
:::

:::caution[Not a distributed lock]
Each operation is a `GET` then a `SET` against one Redis key per backend — not a single atomic command. Two `rws` instances racing to record a failure for the same backend at the same instant can lose one of the two increments. This is intentional: unlike a rate limit (a hard resource/security boundary, where `RedisRateLimiter` uses genuinely atomic `INCR` for exactly this reason), a circuit breaker is a self-healing heuristic — opening one failure later than a perfectly-synchronized count would have has no real consequence.
:::

## RetryLayer middleware

`RetryLayer` wraps any `Application` and re-dispatches the request when the inner app returns a retryable status code (default: 502, 503, 504), up to `max_retries` additional attempts. The last response is returned as-is if all attempts are retryable.

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::circuit_breaker::RetryLayer;
use rust_web_server::middleware::WithMiddleware;

let app = WithMiddleware::new(App::new())
    .wrap(RetryLayer::new().max_retries(2));
```

### Builder options

```rust
RetryLayer::new()
    .max_retries(3)                    // default: 3
    .retry_on(vec![502, 503, 504])     // default: [502, 503, 504]
```

### Custom retry codes

```rust
// Retry on 429 Too Many Requests and 503 Service Unavailable only
let retry = RetryLayer::new()
    .max_retries(5)
    .retry_on(vec![429, 503]);
```

## Using with ReverseProxy

Combine `RetryLayer` with `ReverseProxy` for resilient upstream calls. `RetryLayer` sits outside `ReverseProxy` in the middleware stack so that each retry attempt may land on a different backend via round-robin:

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::proxy::{LoadBalancing, ReverseProxy};
use rust_web_server::circuit_breaker::RetryLayer;

let app = App::new()
    .wrap(ReverseProxy::new([
        "http://api-1:8080",
        "http://api-2:8080",
        "http://api-3:8080",
    ]))
    .wrap(RetryLayer::new().max_retries(2));
```

:::note[Layer ordering]
Middleware is applied in push order — last-pushed is innermost. Push `ReverseProxy` first and `RetryLayer` second so that retries are visible to the outermost layers (metrics, rate limiting, etc.).
:::

## Automatic ReverseProxy wiring

`RetryLayer` does not consult the `CircuitBreaker` automatically — it retries purely on status code, with no memory between requests. `ReverseProxy` does, via `.with_circuit_breaker(breaker)`: an `Open` backend is skipped with no TCP attempt at all, and `record_success`/`record_failure` is called automatically after every dial. No manual `is_available` checks around each proxied call needed:

```rust
use std::sync::Arc;
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::circuit_breaker::global as global_breaker;
use rust_web_server::proxy::ReverseProxy;

let app = App::new().wrap(
    ReverseProxy::new(["http://backend-1:8080", "http://backend-2:8080"])
        .with_circuit_breaker(Arc::new(global_breaker())),
);
```

`with_circuit_breaker` takes `Arc<dyn Breaker>` — a small object-safe trait implemented for both `Mutex<CircuitBreaker>` and `RedisCircuitBreaker`, so either kind plugs in through the same method:

```rust
use std::sync::{Arc, Mutex};
use rust_web_server::circuit_breaker::CircuitBreaker;

// A dedicated (non-global) in-memory breaker:
let breaker = Arc::new(Mutex::new(CircuitBreaker::new(5, 30)));
let app = App::new().wrap(
    ReverseProxy::new(["http://backend-1:8080"]).with_circuit_breaker(breaker),
);

// Or a Redis-backed one:
use rust_web_server::circuit_breaker::RedisCircuitBreaker;
let breaker = Arc::new(RedisCircuitBreaker::from_env());
let app2 = App::new().wrap(
    ReverseProxy::new(["http://backend-1:8080"]).with_circuit_breaker(breaker),
);
```

:::note[Fail open on Redis errors]
Through this integration, `RedisCircuitBreaker::is_available`'s `Err` (Redis unreachable) is treated as *available* — failing open, not closed. A breaker that can't be reached shouldn't become a new single point of failure for every proxied request. Call `RedisCircuitBreaker::is_available` directly (outside this integration) if your use case needs fail-closed semantics instead.
:::

No breaker is wired by default, so existing `ReverseProxy` code sees no behavior change unless it opts in.

If you're integrating a circuit breaker into your *own* custom controller or middleware instead of `ReverseProxy`, the manual pattern still works exactly as before:

```rust
use rust_web_server::circuit_breaker;

let backend = "api-service:8080";
let allowed = circuit_breaker::global()
    .lock()
    .unwrap()
    .is_available(backend);

if !allowed {
    // Return 503 without attempting a connection
}
```

## Metrics

`GET /metrics` includes `rws_circuit_breaker_state{backend}` automatically — no opt-in layer needed — for every backend known to `circuit_breaker::global()`:

```
# HELP rws_circuit_breaker_state Circuit breaker state per backend (0=closed, 1=half_open, 2=open)
# TYPE rws_circuit_breaker_state gauge
rws_circuit_breaker_state{backend="api-1:8080"} 0
rws_circuit_breaker_state{backend="api-2:8080"} 2
```

Wiring `ReverseProxy` to `circuit_breaker::global()` (as in the example above) gets you both the automatic proxy integration *and* this metric from the same breaker instance — the recommended default setup. `RedisCircuitBreaker` state isn't enumerable this way: the minimal hand-rolled RESP client has no `SCAN`/`KEYS` support to discover its keys.
