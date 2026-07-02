---
title: Circuit Breaker & Retry
description: Protect upstream services from cascading failures with a three-state circuit breaker and automatic retry middleware.
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
| `HalfOpen` | One probe request is let through after `recovery_secs` have elapsed. Success closes the circuit; failure re-opens it and resets the recovery timer. |

## Creating a circuit breaker

```rust
use rust_web_server::circuit_breaker::CircuitBreaker;

// threshold=5 consecutive failures, recovery window=30 s
let cb = CircuitBreaker::new(5, 30);
```

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

## Integrating the circuit breaker manually

`RetryLayer` does not consult the `CircuitBreaker` automatically. To wire them together, call `circuit_breaker::global()` inside a custom controller or middleware before forwarding:

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
