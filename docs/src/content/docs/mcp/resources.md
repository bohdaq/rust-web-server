---
title: MCP Resources
description: Expose readable data sources to AI agents via URI-addressed MCP resources.
---

Resources are named, URI-addressed data sources that AI agents can read. Unlike tools (which are invoked with arguments), resources are addressed by URI — the AI browses available resources via `resources/list` and fetches them with `resources/read`.

## Registering a resource

```rust
use rust_web_server::mcp::{McpServer, McpContent};

let mcp = McpServer::new("my-server", "1.0")
    .resource(
        "rws:///config",        // URI (or URI template)
        "Server Config",        // human-readable name
        "Current server configuration as JSON",  // description
        |uri| {
            let config_json = r#"{"port":7878,"threads":4}"#;
            Ok(McpContent::json(config_json.to_string()))
        },
    );
```

## URI templates

Use `{variable}` placeholders in the URI template to match any concrete URI that shares the same prefix up to the placeholder:

```rust
.resource(
    "file:///{path}",
    "Static File",
    "Read a static file by path",
    |uri| {
        // uri = "file:///public/index.html"
        let path = uri.strip_prefix("file:///").unwrap_or("");
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Cannot read {path}: {e}"))?;
        Ok(McpContent::text(content))
    },
)
```

The matching rule: a template with a `{variable}` matches any URI that starts with the text before the `{`. A template without placeholders matches only the exact URI string.

:::note[URI template parsing]
The handler receives the **full concrete URI string** — not a pre-parsed map of variable names to values. Extract variables from the URI string yourself using `strip_prefix` or similar.
:::

## Handler signature

```rust
Fn(&str) -> Result<McpContent, String>
```

The handler receives the concrete URI requested by the AI (e.g., `"rws:///config"` or `"user://42"`). Return `Ok(McpContent)` with the resource content, or `Err(String)` to signal that the resource could not be read.

## McpContent for resources

```rust
use rust_web_server::mcp::McpContent;

// Plain text resource
McpContent::text("line1\nline2\nline3")

// JSON resource — mimeType set to application/json
McpContent::json(r#"{"key":"value"}"#)
```

## Resource listing

The AI discovers available resources via `resources/list` (JSON-RPC method). Each resource is listed with its URI template, name, description, and `mimeType: "text/plain"`. Register resources in the order you want them listed.

## Resource reading

The AI reads a resource via `resources/read` (JSON-RPC method) by sending the concrete URI. The server matches it against registered templates and calls the first matching handler.

## Example: expose server config as a resource

```rust
use rust_web_server::mcp::{McpServer, McpContent};
use std::env;

let mcp = McpServer::new("rws", "17")
    .resource(
        "rws:///config",
        "Server Config",
        "Live server configuration (port, threads, TLS, log format)",
        |_uri| {
            let port    = env::var("RWS_CONFIG_PORT").unwrap_or_else(|_| "7878".to_string());
            let threads = env::var("RWS_CONFIG_THREAD_COUNT").unwrap_or_else(|_| "4".to_string());
            let tls     = env::var("RWS_CONFIG_TLS_CERT_FILE").ok();
            let log_fmt = env::var("RWS_CONFIG_LOG_FORMAT").unwrap_or_else(|_| "combined".to_string());

            let json = format!(
                r#"{{"port":{port},"threads":{threads},"tls":{},"log_format":"{log_fmt}"}}"#,
                tls.map(|_| "true").unwrap_or("false"),
            );
            Ok(McpContent::json(json))
        },
    )
    .resource(
        "rws:///metrics",
        "Server Metrics",
        "Prometheus-format metrics snapshot",
        |_uri| {
            // Pull from global metrics — example only
            Ok(McpContent::text("# HELP rws_requests_total Total requests\n"))
        },
    );
```

## Multiple resources

Chain `.resource()` calls — each registers a separate resource. Resources are matched in registration order:

```rust
McpServer::new("my-server", "1.0")
    .resource("config:///app",  "App Config",  "Application settings",  |_| Ok(McpContent::text("...")))
    .resource("config:///db",   "DB Config",   "Database settings",     |_| Ok(McpContent::text("...")))
    .resource("log:///{level}", "Log Stream",  "Recent log lines",      |uri| {
        let level = uri.strip_prefix("log:///").unwrap_or("info");
        Ok(McpContent::text(format!("Log level: {level}")))
    });
```
