---
title: Frontend Dev Server Proxy (React / Vite / webpack)
description: How to run a React (or Vue/Svelte) dev server side-by-side with rws during local development, so API calls reach the backend and everything else keeps fast HMR.
---

`rws` is a production HTTP server, not a bundler — it has no built-in dev-mode HMR/asset-compilation story. During local development you run your frontend's own dev server (Vite, webpack-dev-server, etc.) for fast hot-module-reload, and `rws` as your API backend on a separate port. This page covers the two ways to wire the two together, and which one to reach for.

## Recommended: point the dev server's own proxy at rws

This is the standard pattern for every JS frontend tooling ecosystem, and the simplest option here — the frontend dev server proxies API requests to `rws`; everything else (the app bundle, HMR's own WebSocket) stays entirely inside the dev server, which already handles it correctly. The browser only ever talks to one origin (the dev server's port), so **no CORS configuration is needed at all**.

### Vite

```js
// vite.config.js
export default {
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:7878',
        changeOrigin: true,
      },
    },
  },
};
```

```bash
# Terminal 1
rws  # backend on :7878 (RWS_CONFIG_PORT, default 7878)

# Terminal 2
npm run dev  # Vite dev server on :5173, proxies /api/* to rws
```

Your React code just calls `fetch('/api/users')` — Vite transparently forwards it to `http://localhost:7878/api/users` and the response looks like it came from the same origin.

### webpack-dev-server (Create React App / custom webpack)

```js
// webpack.config.js
module.exports = {
  devServer: {
    proxy: {
      '/api': {
        target: 'http://localhost:7878',
        changeOrigin: true,
      },
    },
  },
};
```

Create React App reads the same shape from a `"proxy"` key in `package.json` instead, if you haven't ejected:

```json
{
  "proxy": "http://localhost:7878"
}
```

## Alternative: call rws directly, cross-origin

If you'd rather not proxy through the dev server — e.g. you're testing against a shared staging backend rather than a local `rws` instance — call it directly from the browser at its own origin instead. `RWS_CONFIG_CORS_ALLOW_ALL=true` is already the default, so a fresh `rws` instance accepts cross-origin requests (including credentialed ones) from any origin with no configuration at all — convenient for a quick local test, but scope it down for anything beyond that:

```bash
export RWS_CONFIG_CORS_ALLOW_ALL=false
export RWS_CONFIG_CORS_ALLOW_ORIGINS=http://localhost:5173
export RWS_CONFIG_CORS_ALLOW_CREDENTIALS=true   # only if you send cookies
rws
```

See [CORS & Security](/features/cors-security/) for the full set of `RWS_CONFIG_CORS_*` variables.

This works with no bundler-side config, but every request now does a real cross-origin fetch — mutating requests will trigger a CORS preflight `OPTIONS` round-trip, and cookie-based auth needs `credentials: 'include'` in `fetch`.

## Alternative: rws as the single front door

You can flip the direction — run `rws` as the one port you point your browser at, and have it reverse-proxy through to the dev server instead. `ReverseProxy`'s `.path_prefix()` is shaped for the opposite split from what you'd want here, though, so it's worth understanding before reaching for it:

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::proxy::ReverseProxy;

// path_prefix("/api") means /api/* is what gets FORWARDED to the given
// backend — everything else falls through to the wrapped App instead.
let app = App::new()  // serves everything that ISN'T /api locally
    .wrap(ReverseProxy::new(["http://backend-api:8080"]).path_prefix("/api"));
```

That's the right shape for *production* — `App` serves your built, static frontend and `/api` is proxied to a real backend service (see [Reverse Proxy](/proxy/reverse-proxy/)). For the *dev-time* case on this page — proxy everything **except** `/api` to the dev server, and answer `/api` locally — there's no built-in "exclude prefix" on `ReverseProxy` (unlike `RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES`'s exclude-list for the static-file fallback), so it isn't a one-line `.wrap()`. You'd need a small custom `Application` that checks the path itself and dispatches to either your local API handler or `ReverseProxy` explicitly — meaningfully more code than the [recommended approach](#recommended-point-the-dev-servers-own-proxy-at-rws) above for the same result.

:::caution[HMR's WebSocket does not pass through `ReverseProxy`, either way]
`ReverseProxy` strips the `Upgrade` header on every forwarded request (it's a plain buffered request/response HTTP/1.1 proxy, not a WebSocket-aware one — see [Reverse Proxy](/proxy/reverse-proxy/)). Vite's HMR client opens a WebSocket back to the dev server for live updates; proxied through `ReverseProxy` that connection simply won't upgrade, and Vite falls back to full-page reloads instead of true HMR — regardless of which direction the proxying goes.

This is the real reason the [recommended approach](#recommended-point-the-dev-servers-own-proxy-at-rws) above has the dev server proxy *to* rws, not the other way around: the dev server keeps serving its own bundle and HMR socket directly, so none of its WebSocket handling is ever in rws's path.
:::

Given both the extra composition work and the HMR limitation, reach for this direction only if you specifically need rws's own middleware (auth, rate limiting, IP filtering) sitting in front of the dev bundle for a test — otherwise the recommended approach above gets you the same local dev loop with far less setup.

## Production has none of this

In production there's no separate dev server at all — build your frontend (`npm run build`), point `rws` at the output directory, and let `StaticResourceController` serve it directly, with `RWS_CONFIG_SPA_FALLBACK` handling client-side-router deep links. See [Static Files, Directory Listing & SPA Fallback](/features/static-files/).
