---
title: WebAssembly Plugins
description: Extend rws without recompiling it. A WebAssembly module checks or changes each request and response, sandboxed with no imports, and bounded in CPU and memory.
---

A WebAssembly plugin (since 18.10.0) runs on every request of a route, and optionally on its response. It can let the request through, change its headers or target, answer it itself, or change the response's status and headers. Plugins are compiled WebAssembly core modules, written in any language that targets `wasm32`, and rws loads them without being rebuilt.

The release binaries include it. Building from source, enable the `wasm-plugins` feature (it adds wasmtime and several MB to the binary):

```sh
cargo install rust-web-server --features "wasm-plugins"
```

## In the config file

```toml
[[route]]
  [route.match]
  path = "/app/*"
  [route.action]
  type = "proxy"
  upstream = "app"

  [route.middleware.wasm]
  module    = "/etc/rws/plugins/tag.wasm"
  fuel      = 50000000      # CPU per hook call (default 50,000,000, roughly tens of ms)
  memory_mb = 16            # the plugin's memory limit (default 16)
  config    = "region=eu"   # a string the plugin's rws_init gets (optional)
```

The plugin runs after authentication on the route and before the upstream. `rws --check` loads the module and reports one that can't be loaded or has the wrong exports. A route whose plugin can't be loaded at startup answers `503`, so a missing plugin never means skipped checks. The module is loaded when the routes are built: at startup, and on a reload (`SIGHUP`, `POST /reload` on the [admin API](/deployment/admin/)) that finds the config file changed. Replacing only the `.wasm` file isn't picked up until then; ship a new plugin under a new file name and point `module` at it.

## Sandbox

- **No imports.** A plugin gets no WASI, clock, files, network or host functions. A module that imports anything is refused when it loads. It works only on what rws passes in.
- **CPU.** Each hook call gets `fuel`. A plugin that runs out (an endless loop) is stopped.
- **Memory.** The plugin's linear memory can't grow past `memory_mb`.
- **A fresh instance per request**, so nothing carries over from one request to the next.

A trap, running out of fuel or memory, or output rws can't read answers `500` and counts in `rws_wasm_plugin_errors_total{plugin, kind}` (`kind` is `fuel`, `memory`, `trap` or `output`). The server keeps running. Every call counts in `rws_wasm_plugin_calls_total{plugin, hook}`.

## Writing a plugin

The ABI is plain functions over the plugin's memory. The module exports:

| Export | Signature | |
|---|---|---|
| `memory` | memory | required |
| `rws_alloc` | `(len: i32) -> i32` | required: a buffer of `len` bytes that rws writes the input into |
| `rws_on_request` | `(ptr: i32, len: i32) -> i64` | required |
| `rws_on_response` | `(ptr: i32, len: i32) -> i64` | optional |
| `rws_init` | `(ptr: i32, len: i32)` | optional: called with `config` before the hooks |

A hook returns `0` to change nothing, or `(ptr << 32) | len` of its output in its own memory.

The request input is the request line, the client address, the headers in lower case, and a blank line. The body isn't passed.

```text
GET /app/orders?page=2 HTTP/1.1
203.0.113.7
host: shop.example.com
authorization: Bearer eyJ…

```

The response input is the status code, then the headers and a blank line.

The output is one directive per line:

| Directive | In | Does |
|---|---|---|
| `set <name>: <value>` | request, response | replace the header |
| `add <name>: <value>` | request, response | add a header |
| `remove <name>` | request, response | remove the header |
| `uri <target>` | request | send the request on with another target (`/…`) |
| `status <code>` | response | change the status |
| `respond <code>` | request | answer now: `name: value` header lines follow, then a blank line and the body |

Anything else, and header names or values containing CR, LF or NUL, is an error that answers `500`: a plugin can't smuggle headers.

### In Rust

The repository's `examples/wasm_plugin` crate is a complete plugin with no dependencies. It tags requests, answers one itself, and stamps responses with its config:

```rust,ignore
#[no_mangle]
pub extern "C" fn rws_on_request(ptr: i32, len: i32) -> i64 {
    let request = input(ptr, len);
    if request.lines().skip(2).any(|l| l.eq_ignore_ascii_case("x-block: yes")) {
        return output("respond 403\ncontent-type: text/plain\n\nblocked\n".to_string());
    }
    output("set x-plugin: rws-example\n".to_string())
}
```

```sh
cd examples/wasm_plugin
cargo build --release --target wasm32-unknown-unknown
# target/wasm32-unknown-unknown/release/rws_wasm_plugin_example.wasm (about 20 KB)
```

Use `wasm32-unknown-unknown`, not a WASI target: a WASI build imports WASI functions, and rws refuses it.

## In code

`rust_web_server::wasm_plugin::WasmLayer` is a `Middleware`:

```rust,ignore
use rust_web_server::wasm_plugin::WasmLayer;

let plugin = WasmLayer::from_file("plugins/tag.wasm")?.fuel(10_000_000).memory_mb(8).config("region=eu");
let app = my_app.wrap(plugin);
```

`WasmLayer::new(name, &bytes)` loads a module from memory; `on_request` runs the request hook on its own, for tests.
