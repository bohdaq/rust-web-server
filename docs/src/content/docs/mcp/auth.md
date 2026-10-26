---
title: MCP Authentication
description: Protect your MCP endpoint with a static Bearer token (require_bearer) or full OAuth 2.0 / OIDC JWT verification (require_oauth).
---

By default the MCP endpoint (`POST /mcp`) is open to any caller. Call `.require_bearer(token)` to gate every request behind a static Bearer token, or `.require_oauth(provider, audience)` (`sso` feature) for full per-user OAuth 2.0 / OIDC bearer JWT verification — see [OAuth 2.0 authorization](#oauth-20-authorization-mcp-2025-03-26) below.

## require_bearer

```rust
use rust_web_server::mcp::McpServer;

let mcp = McpServer::new("my-server", "1.0")
    .require_bearer(std::env::var("MCP_TOKEN").expect("MCP_TOKEN not set"));
```

Every request to `/mcp` (`POST`, the `GET` SSE stream and `OPTIONS` alike) must include:

```
Authorization: Bearer <token>
```

A missing or incorrect token produces an immediate `401 Unauthorized` before any JSON-RPC processing:

```
HTTP/1.1 401 Unauthorized
WWW-Authenticate: Bearer
Content-Type: text/plain

Unauthorized
```

## Loading the token from an environment variable

Never hard-code the token in source code. The conventional variable is `MCP_TOKEN`:

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;

let mcp = App::new()
    .mcp("my-server", "1.0")
    .require_bearer(
        std::env::var("MCP_TOKEN").expect("MCP_TOKEN env var not set"),
    );
```

Set the variable before starting the server:

```sh
export MCP_TOKEN="$(openssl rand -hex 32)"
cargo run
```

The bundled `rws` binary mounts its built-in MCP tools only when `MCP_TOKEN`
is set to a non-empty value (since 17.113.1): its tools change the running
server, so without a token `/mcp` answers `404`. There `/mcp` also sits behind
the access log, the IP blocklist and the global rate limit (not maintenance
mode — the `maintenance` tool is how it's switched off). An empty token passed
to `require_bearer` refuses every request rather than accepting an empty
`Authorization` header, and tokens are compared in constant time.

## One token per caller

`require_bearer_tokens` (since 18.0.0) accepts several tokens and names the
caller behind each; tools see that name as `{"sub":"<name>"}` in
[`ctx.auth_claims`](#reading-verified-claims), the same place OAuth puts the
token's claims:

```rust
let mcp = McpServer::new("team-tools", "1.0")
    .require_bearer_tokens([("token-for-alice", "alice"), ("token-for-ci", "ci")])
    // Only alice may call tools whose names start with "admin_".
    .tool_filter(|ctx, tool| {
        !tool.starts_with("admin_") || ctx.auth_claims.as_deref() == Some(r#"{"sub":"alice"}"#)
    });
```

`tool_filter(|ctx, tool_name| -> bool)` decides per request which tools a
caller sees in `tools/list`; calling any other answers `Unknown tool`, as
for a tool that doesn't exist. It works with `require_oauth` too (decide
from the claims). A `require_bearer` token carries no name, so its requests
have no `auth_claims`.

## Browser clients: `Origin` and `Content-Type`

A request that carries an `Origin` header — every request a browser sends —
is refused with `403` unless that origin is listed (since 17.113.1). This is
the MCP transport's required `Origin` check: without it a web page, or a
DNS-rebinding attack, could drive a local MCP server from the visitor's
browser. Clients that send no `Origin` (CLI tools, agents, Claude Desktop)
are unaffected.

```rust
let mcp = McpServer::new("my-server", "1.0")
    .require_bearer(token)
    .allowed_origins(["https://inspector.example.com"]);
```

The `rws` binary takes the list from `MCP_ALLOWED_ORIGINS` (comma-separated).
`POST /mcp` must carry `Content-Type: application/json` (otherwise `415`): a
form or `text/plain` body is what a page can send cross-site without a CORS
preflight.

## Configuring Claude Desktop

Add the `Authorization` header to the server entry in `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "my-server": {
      "url": "http://localhost:7878/mcp",
      "headers": {
        "Authorization": "Bearer <your-token-here>"
      }
    }
  }
}
```

Replace `<your-token-here>` with the value of `MCP_TOKEN`.

## Full example

```rust
use rust_web_server::server::Server;
use rust_web_server::mcp::{McpServer, McpContent};

# #[cfg(not(feature = "http2"))]
# fn main() {
let token = std::env::var("MCP_TOKEN")
    .expect("Set MCP_TOKEN before starting the server");

let mcp = McpServer::new("secure-server", "1.0")
    .require_bearer(token)
    .tool(
        "ping",
        "Returns pong",
        r#"{"type":"object","properties":{}}"#,
        |_| Ok(McpContent::text("pong")),
    );

let (listener, pool) = Server::setup().unwrap();
Server::run(listener, pool, mcp);
# }
```

## CORS preflight

`OPTIONS /mcp` answers `200 OK` with `Allow: GET, POST, OPTIONS` — but only after the auth check: with `.require_bearer()` or `.require_oauth()` configured, a preflight without a valid `Authorization` header gets `401`, like any other request to `/mcp`.

:::note[Multi-user auth]
`.require_bearer` enforces a single shared token — it is not a per-user authentication mechanism. For per-user access control validating JWT claims against a live IdP, use [`.require_oauth`](#oauth-20-authorization-mcp-2025-03-26) below instead of a custom wrapper. For anything else (e.g. checking a database), implement a custom `Application` wrapper that inspects the `Authorization` header and delegates to `McpServer` only on success:

```rust
use rust_web_server::application::Application;
use rust_web_server::request::Request;
use rust_web_server::response::Response;
use rust_web_server::server::ConnectionInfo;
use rust_web_server::mcp::McpServer;

struct AuthGate {
    inner: McpServer,
}

impl Application for AuthGate {
    fn execute(&self, req: &Request, conn: &ConnectionInfo) -> Result<Response, String> {
        // your custom auth logic here
        self.inner.execute(req, conn)
    }
}
```
:::

## OAuth 2.0 authorization (MCP 2025-03-26)

The 2025-03-26 revision of the MCP spec defines an OAuth 2.0 authorization flow for multi-tenant or enterprise deployments, where each connecting user authenticates independently rather than sharing one static token. `.require_oauth(provider, audience)` (`sso` feature) implements this by verifying the client's bearer token as a signed JWT against a live JWKS endpoint — reusing [`sso::jwks::JwksCache`](/features/sso/#rs256es256-jwt-verification-via-jwks), the same verifier `OidcAuth` and `AuthServer` use elsewhere in this crate.

```toml
[dependencies]
rust-web-server = { version = "17", features = ["sso"] }
```

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::sso::OidcProvider;

let app = App::new()
    .mcp("my-server", "1.0")
    .require_oauth(OidcProvider::google(), "my-mcp-client-id");
```

`provider` supplies the issuer, JWKS URI, and (for the metadata endpoint below) the authorization/token endpoints — use a preset (`OidcProvider::google()`, `::okta(domain)`, `::keycloak(base, realm)`, etc.) or discover them live:

```rust
use rust_web_server::sso::OidcProvider;

let provider = OidcProvider::discover("https://accounts.example.com")?;
let app = mcp.require_oauth(provider, "my-mcp-client-id");
```

A request with a missing, malformed, or invalid-signature bearer token gets the same `401 Unauthorized` / `WWW-Authenticate: Bearer` response `.require_bearer()` returns — only the verification underneath differs.

### Reading verified claims

On success, the verified claims (a serialized [`OidcClaims`](/features/sso/)) populate `McpContext.auth_claims` as a JSON string, readable from any `.tool_with_context()` handler:

```rust
use rust_web_server::mcp::{McpContent, McpServer};

let mcp = McpServer::new("my-server", "1.0")
    .require_oauth(rust_web_server::sso::OidcProvider::google(), "my-mcp-client-id")
    .tool_with_context("whoami", "Report the caller", "{}", |ctx, _args| {
        let claims = ctx.auth_claims.unwrap_or_default();
        Ok(McpContent::text(claims))
    });
```

### Discovery endpoint

With `.require_oauth()` configured, the server publishes RFC 9728
protected-resource metadata (MCP 2025-06-18) at
`/.well-known/oauth-protected-resource` (and at that path followed by the MCP
path, e.g. `/.well-known/oauth-protected-resource/mcp`):

```json
{
  "resource": "https://mcp.example.com/mcp",
  "authorization_servers": ["https://accounts.google.com"],
  "bearer_methods_supported": ["header"]
}
```

A `401` points clients at it:
`WWW-Authenticate: Bearer resource_metadata="https://mcp.example.com/.well-known/oauth-protected-resource"`.
The client then discovers the authorization server from its own metadata
and asks it for a token for `resource` (RFC 8707).

Pass this server's canonical URL as the audience —
`.require_oauth(provider, "https://mcp.example.com/mcp")`: tokens must carry
it in `aud`, and it is the metadata's `resource`. With a plain client id as
the audience, `resource` is built from the request's `Host` and the MCP
path. (Before 18.0.0 an authorization-server-style document was served at
`/.well-known/oauth-authorization-server`; it's gone.)

If `.require_oauth()` isn't configured, these paths aren't handled specially —
they fall through to the fallback app (or the built-in `App`'s `404`) like any
other non-MCP path.

### Combining with `require_bearer`

If `.require_bearer(token)` or `.require_bearer_tokens(...)` and `.require_oauth(provider, audience)` are configured on the same server, OAuth verification takes precedence and the static token is never checked. This isn't a supported combination to actually run in production — pick one — it's just an unambiguous tie-break so the behavior isn't undefined if both happen to be set.
