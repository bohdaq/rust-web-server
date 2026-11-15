---
title: Roadmap
description: What is still planned for rust-web-server, what shipped from earlier versions of this page, and where the full ordered list of open work lives.
---

Still planned:

:::caution[Coming Soon]
**Baggage and automatic child spans**

W3C baggage propagation, and spans created automatically for each database query and outbound call. Child spans already work when started by hand with `otel::span` / `otel::client_span` — see [Distributed Tracing](/features/tracing/#child-spans).
:::

Shipped since this page first listed them:

- **Admin UI**: live metrics, configuration, the access log and a Traffic page, behind sign-in. See [Admin API and UI](/deployment/admin/) and [Traffic](/deployment/traffic/).
- **Access log rotation** (18.15.0): set `access_log_max_size` and rws rotates the file itself. See [systemd](/deployment/systemd/).

---

**WebAssembly guest component** — shipped. `App`/`Router`/middleware now run inside a `wasi:http/proxy` WASM component (Wasmtime, Spin, Fastly Compute) via the separate `rws-wasm-shim` package. See [WASM Guest Component](/deployment/wasm/). Larger follow-up phases (streaming bodies, outbound HTTP from the guest) remain open — tracked in `spec/WASM_SHIM.md` in the repository.

---

The full ordered list of open work is [`spec/ROADMAP.md`](https://github.com/bohdaq/rust-web-server/blob/main/spec/ROADMAP.md); track progress and open issues at [github.com/bohdaq/rust-web-server](https://github.com/bohdaq/rust-web-server/issues).
