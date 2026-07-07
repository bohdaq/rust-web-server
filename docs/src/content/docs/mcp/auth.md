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

Every `POST /mcp` request must include:

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

The bundled `rws` binary reads `MCP_TOKEN` automatically when the config file enables the built-in MCP server.

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

`OPTIONS /mcp` requests always receive `200 OK` without checking the Bearer token. This allows browser-based MCP clients to complete the CORS preflight before attaching credentials.

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

`GET /.well-known/oauth-authorization-server` is served automatically whenever `.require_oauth()` is configured:

```json
{
  "issuer": "https://accounts.google.com",
  "authorization_endpoint": "https://accounts.google.com/o/oauth2/v2/auth",
  "token_endpoint": "https://oauth2.googleapis.com/token",
  "jwks_uri": "https://www.googleapis.com/oauth2/v3/certs",
  "response_types_supported": ["code"]
}
```

:::note[Not a full RFC 8414 document]
This server is a *resource* server verifying tokens issued elsewhere, not the authorization server itself — the metadata document only carries what `OidcProvider` already knows (issuer, endpoints, JWKS URI), not the full set of fields a standalone authorization server's own metadata would advertise.
:::

If `.require_oauth()` isn't configured, this path isn't handled specially — it falls through to the fallback app (or the built-in `App`'s `404`) like any other non-MCP path.

### Combining with `require_bearer`

If both `.require_bearer(token)` and `.require_oauth(provider, audience)` are configured on the same server, OAuth verification takes precedence and the static token is never checked. This isn't a supported combination to actually run in production — pick one — it's just an unambiguous tie-break so the behavior isn't undefined if both happen to be set.
