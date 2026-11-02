[Read Me](../README.md) > [Spec](.) > Workspace split

# Workspace split

Decided 2026-11-02 (the user: "full split", after the measurements below).
Split the `rust-web-server` crate into layered crates under `crates/`, with
`rust-web-server` kept as the facade every user depends on. No public path
changes: `rust_web_server::request::Request` and every other item stay where
they are, re-exported from the crate that now holds them.

## Why, and what it costs

Measured on 18.25.0 (8 cores, debug, no debug info):

| | |
|---|---|
| Clean library build | 47 s |
| Library rebuild after touching `src/mcp/mod.rs` | 2.6 s |
| Test binary, first build | 66 s |
| Test binary rebuild after touching `src/mcp/mod.rs` | 21 s |

110 top-level modules, 109K non-test lines. **76 modules (94K lines, 86 %)
form one strongly connected component**: `request` ↔ `application` ↔
`server` ↔ `app` ↔ `proxy` ↔ `mcp` … (`ci/module_graph.py` prints it). A
crate can't depend on one that depends back on it, so the split is mostly
cutting these edges; moving files is the easy part. The gain is per-crate
incremental builds and test binaries (a change in `mcp` stops recompiling and
relinking the server's tests), clearer boundaries, and a dependency graph
that can't silently grow back into one knot.

## Rules

1. **The facade keeps the API.** `src/lib.rs` re-exports each moved module
   under its old name (`#[doc(inline)] pub use rws_http::request;`). The
   `rws` binary, `examples/`, the starters and the agent pack keep using
   `rust_web_server::…` only.
2. **The public API gate verifies every tranche.** `cargo public-api` shows a
   re-exported module as one `pub use` line, so `ci/public_api.sh` lists the
   facade and each crate in `CRATES` and rewrites the crate's paths to the
   facade's (`rws_mime::` → `rust_web_server::mime_type::`). The merged
   listing must equal `ci/public-api.txt` unchanged — tried on `mime_type`:
   179 of 179 lines identical. A tranche that changes the listing is wrong.
3. **Items other crates need but users don't** become `pub` + `#[doc(hidden)]`
   (rustdoc JSON leaves them out, so the gate stays exact), never a new
   documented API.
4. **Doctests keep the user's path.** A doctest in a moved crate starts with
   the hidden line `# extern crate <crate> as rust_web_server;`, so the
   example still reads `use rust_web_server::…` in the rendered docs.
5. **Lockstep versions.** `[workspace.package] version` (and edition,
   rust-version, license, repository); every crate `version.workspace = true`;
   path dependencies pinned `=<version>`. All crates are released together;
   each sub-crate's description says it's internal to rust-web-server.
6. **Features forward.** A facade feature enables the same feature in the
   crates that implement it (`acme = ["rws-server/acme", …]`); default
   features unchanged.
7. **Cycles are cut by moving the item down, not by traits for their own
   sake**: a type used below its module moves to the lower crate (e.g.
   `ConnectionInfo` out of `server`); a function that needs a higher layer
   moves up next to its caller.
8. **Shared checkout.** Other sessions edit `src/` while this runs: each
   tranche starts from fresh `main`, moves with `git mv` (history follows),
   and is pushed as soon as it's green, so nobody's edits land on a file that
   has moved.

## Layers (target)

| Crate | Holds | May depend on |
|---|---|---|
| `rws-base` | `symbol`, `ext`, `null`, `core` (base64, sha256, json escape, x509), `mime_type`, `url`, `http`, `dns` | std + its own deps |
| `rws-http` | `json`, `extract`, `request`, `response`, `header`, `range`, `body`, `client_hint`, `cors`, `compression`, `error`, `server_config`, `entry_point` (settings), `virtual_host` (`ConnectionInfo` and the `Application` / `Middleware` / `Controller` traits stay with `server`: they'd pull it in) | `rws-base` |
| `rws-server` | `server`, `h2_server`, `h3_handler`, `thread_pool`, `log`, `metrics`, `app` + controllers, `router`, `state`, `test_client`, `websocket`, `sse`, `tls` | `rws-http`, `rws-base` |
| feature crates | `rws-proxy` (proxy, proxy_config, upstream pools, ws/tcp/udp proxies), `rws-mcp`, `rws-sso` (+ auth, rsa_key), `rws-acme`, `rws-model`, `rws-storage`, `rws-gateway-api`, `rws-tower`, `rws-a2a`, … | the layers below; not each other unless the table says so |
| `rust-web-server` | re-exports, `main.rs`, `fuzz_api`, glue that wires features together (admin, config_check, ctl) | everything |

The exact cut of the feature crates is decided when their tranche starts,
from `ci/module_graph.py`'s output at that point.

## Tranches

| # | Tranche | Status |
|---|---|---|
| T0 | Tooling: `ci/module_graph.py` (SCCs, a crate's upward edges), `ci/public_api.sh` crate map, `[workspace.package]`, `crates/`, release.yml `crates` job publishes the workspace in order (`cargo publish --workspace`), `ci/e2e/starters.sh` patches every crate, `check_docs.py` / the settings scanner read `crates/*/src` too | **done**: `module_graph.py` (iterative Tarjan; today one cycle of 76 modules); `public_api.sh` `CRATES` + `public_api_merge.py` — listing kept in a normal form (each re-export one `pub use` line, repeats counted once; baseline regenerated, same lines as before); tried on a throwaway `rws-mime`: equal. `[workspace.package]` (version etc.; facade inherits; tooling greps `^version = `); `ci/publish_order.py` drives the release `crates` job; starters patch every `crates/*`; settings scanner reads `crates/*/src`. `check_docs.py` reads `src/entry_point` — moves with that module in T2 |
| T1 | `rws-base` | **done**: `core`, `ext`, `http`, `mime_type`, `null`, `symbol`, `url`, `dns` (`json` moved to T2: its `Json<T>` extractor needs `request`); cut `http` → `scheduler` (`days_to_ymd`/`ymd_to_days` now in `http::date`); 40 `pub(crate)` items → `#[doc(hidden)] pub`; one test (PKCE) moved to `sso::pkce` |
| T2 | `rws-http` | **done**: `body`, `client_hint`, `compression`, `cors`, `entry_point`, `error`, `extract`, `header`, `json`, `range`, `request`, `response`, `server_config`, `virtual_host`, new hidden `static_path`; cuts: `extract::RequestId` reads its own `REQUEST_ID_HEADER` (a facade test pins it to `request_id::DEFAULT_HEADER`), `range` uses `static_path` (moved out of `static_resource`), `TrustedProxies` + range parsing → rws-http `ip_filter` and `early_hints::valid` → rws-http `early_hints` (the facade modules `pub use rws_http::<m>::*` and add the middleware), doc links to higher layers → plain text, `rws --version`'s FIPS label from rws-http's `fips` feature. Features `serde`/`brotli`/`zstd`/`fips` forward. Tests needing the facade or repo fixtures → `src/http_tests/` (cors, range, server_config, 13 fixture tests, the `#[derive(FromRequest)]` tests); doctests use the `extern crate … as rust_web_server` line. API gate: `ci/public_api_merge.py` drops a facade `pub use` of a path a crate defines |
| T3 | `rws-server` | planned |
| T4… | feature crates, one per tranche, largest dependency first (`gateway_api`, `tower`, `model`, `storage` are already outside the knot) | planned |
| last | facade cleanup; docs (`reference/api.md` names the crate per row, CLAUDE.md architecture), measurements re-taken | planned |

Each tranche: all three test suites, the e2e scripts touching what moved, the
API gate unchanged, CI green, then pushed. Releases go out as usual between
tranches; the crates.io publish of new crates happens with the first release
after the tranche that adds them.
