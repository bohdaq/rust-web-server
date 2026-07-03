[Read Me](../README.md) > [Spec](.) > Architectural Changes

# Architectural Changes — approach-level, not feature gaps

`TODO.md`, `GAPS_V3.md`, and `IDEAS.md` catalog missing *features*. This document is different: it flags places where the current **implementation approach** is internally inconsistent or fights itself, independent of what's built vs. not built. Each item is grounded in the current code, not a hypothetical.

---

## 1. `di::Container` is built but unused by the framework itself

> **Status: resolved (v17.49.0).** Took the "wire it in" option below. `Container` needed no code changes to work as `AppWithState`/`AsyncAppWithState`'s `S` — it's already `Send + Sync + 'static` like any other state type — but the two places that documented this pattern (`src/di/mod.rs`'s module doc, `DEVELOPER.md`'s "Dependency injection" use case) both showed `App::with_state(container.into_arc())`, which double-wraps in `Arc` (`with_state` already wraps `S` internally) and was never exercised by a real test, only by an unrun (`no_run`) doc example. Fixed both to `App::with_state(container)` (handlers now receive `&Container`, not `&Arc<Container>`), added the missing `App::with_async_state` example, and added 5 real tests (`src/state/tests.rs`, `src/async_state/tests.rs`) that register concrete and trait-object services and resolve them through an actual request via `Application::execute` — not just documentation claims. `llms.txt`'s dependency-injection section had the same ambiguity and was fixed the same way.

`src/di/mod.rs` implements a real type-keyed service container (`register::<T>`, `provide::<T: ?Sized>`, `get::<T>`, named services, `into_arc()`). Nothing in `App`, `AppWithState`, `AsyncAppWithState`, or the model layer actually uses it — a repo-wide grep for `Container::new` / `di::Container` outside `src/di/` and its own tests turns up nothing.

Meanwhile the framework's actual mechanism for shared, process-wide state is `RWS_CONFIG_*` environment variables (see §4). If `Container` was meant to be the seam that eventually replaces env-var config, it hasn't been wired in anywhere yet. If it's meant as a standalone opt-in utility for user code, that's a reasonable design but should be stated explicitly — right now it reads as unfinished plumbing rather than an intentional choice.

**Options:**
- Wire `Container` into `AppWithState`/`AsyncAppWithState` as the mechanism for resolving shared services (DB pools, HTTP clients, config), replacing ad-hoc `Arc<S>` state structs for anything beyond simple cases.
- Or, document `Container` explicitly as a standalone opt-in utility with its own use-case example in DEVELOPER.md, so it doesn't look orphaned.

---

## 2. Sync `DbPool` has no guardrail against blocking the async runtime

`src/model/pool.rs` is a blocking pool: `Mutex<Vec<DbConnection>>`, `pool.get()` blocks on the mutex and on `DbConnection::open()` if empty. The default build (`http3`, implying `http2`) runs everything on a tokio runtime.

The codebase is already aware that blocking calls inside async contexts need explicit handling — `tokio::task::block_in_place` is used deliberately in [`src/proxy/mod.rs:690`](../src/proxy/mod.rs) (bridging `H2ReverseProxy`'s sync middleware trait into tokio) and [`src/server/mod.rs:505`](../src/server/mod.rs) (running the sync `http1` accept loop under tokio). There is no equivalent pattern documented or enforced for calling `pool.get()` / `conn.query(...)` from inside an `async fn` handler registered via `AsyncAppWithState`. A handler that does `let conn = pool.get()?; conn.query(...)` directly will block a tokio worker thread for the duration of the DB call — fine at low concurrency, a silent throughput cliff under load, and the kind of bug that's invisible until production traffic hits it.

**What's missing:** either an async-native pool (`spawn_blocking`-wrapped, or a real async driver behind a new feature flag — already tracked as "async ORM" in `GAPS_V3.md` §3.7), or, as a much smaller stopgap, a documented/enforced rule: model-layer calls from async handlers must go through `tokio::task::spawn_blocking`, with a DEVELOPER.md example showing the wrapped form so users don't discover the footgun themselves.

---

## 3. `App::execute` and `Router` are two competing dispatch strategies

> **Status: resolved (v17.49.0).** Added a comment on `impl Application for App` (`src/app/mod.rs`) stating explicitly why the built-in controller chain is a fixed if-chain rather than a `Router` — the built-in set is small, static, and known at compile time, so a linear scan is simpler and just as fast as a segment matcher would be there; `Router` is for user-defined routes with dynamic path params, and `AppWithState`/`AsyncAppWithState` already build on it, falling through to this same controller chain. Also added a clarifying line to the `Router` row in DEVELOPER.md's building-blocks table. As a related, previously-undocumented duplication found while looking at this: `AsyncAppWithState` had copy-pasted `Router`'s segment/pattern-matching code (`Segment`, `parse_pattern`, `try_match`) verbatim because its handlers return `Future`s, which `Router`'s `HandlerFn` type doesn't support. That logic is now extracted into `src/router/matcher.rs` (`pub(crate)`) and shared by both `Router` and `AsyncAppWithState`, so the two matchers can no longer drift apart. No public API changed; `Controller` remains available for third-party use, and `App`/`AppWithState`/`AsyncAppWithState` behavior is unchanged.

`App::execute` ([`src/app/mod.rs`](../src/app/mod.rs)) walks a hardcoded `Vec<ControllerEntry>` and calls `is_matching` on each in declaration order — an O(n) linear scan per request across the built-in controllers. `Router` ([`src/router/mod.rs`](../src/router/mod.rs)) does proper path-segment matching with named params and wildcards, and is the documented "prefer this for new code" path.

Two dispatch mechanisms living side by side isn't wrong — the built-in controller count is small and fixed, so the linear scan is harmless in practice — but it means the framework has no single canonical answer to "how does a request get routed." Anyone reading `App::execute` for the first time has to independently conclude "this is fine because N is small" rather than that being a stated design decision.

**What's missing:** a one-line comment (or a DEVELOPER.md note) stating why `App` uses a fixed if-chain instead of `Router` — e.g. "built-ins are static and few; `Router` is for user routes with dynamic path params." Cheap to fix, removes a real point of confusion for contributors.

---

## 4. Config-as-global-env-vars is the largest structural cost in the repo

> **Status: partially resolved (v17.45.0).** `ServerConfig` struct added (`src/server_config/mod.rs`). `App::with_config(config)` pins an app to explicit settings — no env reads during request processing. `Cors::get_headers_from_config` and `Header::get_header_list_with_config` are the config-aware entry points. `App::new()` preserves backward compat and hot-reload by calling `ServerConfig::from_env()` per request. Tests that check CORS/CSP behavior should use `App::with_config` and no longer need `test_env::lock()`. `ConfigDrivenApp` and `AppWithState` are not yet wired — see below.

All configuration is read once at startup into process environment variables and accessed globally via `env::var(...)` (see CLAUDE.md's "Configuration" section). This has two real costs, not just style:

- **It's the entire reason the mandatory test-locking rule exists.** CLAUDE.md documents an extensive, easy-to-violate protocol (`test_env::lock()`, the "transitive trap" of `bootstrap()`/`override_environment_variables_from_config`/`config_reload::reload()` all silently writing shared state) that every new test has to get right by hand. That's not a testing nitpick — it's evidence that the underlying design (global mutable process state) doesn't compose with parallel test execution, and the workaround is a manual discipline problem rather than something the type system prevents.
- **It rules out running more than one differently-configured server in a process.** Because config lives in `std::env`, there is no way to construct two `App`/`ConfigDrivenApp` instances with different settings side by side — which matters the moment rws is used as an embedded library rather than a standalone binary (multi-tenant hosting, in-process test harnesses that want two configs, etc.).

**What remains:** `ConfigDrivenApp` (`src/proxy_config/mod.rs`) and `AppWithState` still read env vars at point of use. Threading `ServerConfig` through those types would complete the fix and allow multi-instance deployments. The hard part (the `Cors` + `Header` refactor) is done; the remaining sites are mechanical.

---

## Priority table

| # | Issue | Cost if left alone | Effort | Blast radius |
|---|-------|---------------------|--------|---------------|
| 2 | Sync DB pool blocking async handlers | Silent throughput cliff in production under load | Small (doc/wrapper) to Large (real async pool) | Model layer + docs only |
| 4 | Config as global env vars | Ongoing test flakiness risk; blocks embedding/multi-instance use | Large | Touches every `env::var("RWS_CONFIG_*")` call site |
| 1 | ~~Unused `di::Container`~~ ✅ | ~~Confusing to contributors; duplicate-looking abstraction~~ | ~~Small (document) to Medium (wire in)~~ | ~~`di` module + one integration point~~ |
| 3 | ~~Two dispatch mechanisms (`App` if-chain vs `Router`)~~ ✅ | ~~Minor — easy to misread as an oversight~~ | ~~Trivial (comment)~~ | ~~None~~ |

**Recommended order:** #2 first (cheapest, highest real-world risk — a production incident waiting to happen), then use #1's `Container` as the vehicle for #4's config refactor rather than solving them separately.
