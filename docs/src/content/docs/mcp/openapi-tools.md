---
title: Tools from OpenAPI
description: Turn an OpenAPI 3 document's operations into MCP tools, with input schemas from its parameters and body and HTTP requests built from each call.
---

Most APIs an agent should use already describe themselves with OpenAPI.
`mcp::openapi_tools` (since 18.19.0) reads an OpenAPI 3.0 or 3.1 document
(JSON) and makes each operation an MCP tool: its path, query and header
parameters and its JSON body become the tool's `inputSchema`, a JSON object
response its `outputSchema`, and `request(arguments)` builds the HTTP
request for a call.

```rust
use rust_web_server::mcp::openapi_tools::{tools, ToolOptions};

let spec = r#"{"openapi":"3.1.0","info":{"title":"Billing","version":"1"},
  "paths":{"/invoices/{id}":{"get":{"operationId":"getInvoice","summary":"Get an invoice",
    "parameters":[{"name":"id","in":"path","required":true,"schema":{"type":"integer"}}]}}}}"#;
let found = tools(spec, &ToolOptions::default()).unwrap();
let call = found[0].request(r#"{"id": 42}"#).unwrap();
assert_eq!("/invoices/42", call.path_and_query);
```

## Your app's own routes

An app that describes its routes with [OpenAPI](/features/openapi/) gets
MCP tools for them in one call; each call runs the route in-process (no HTTP
round trip), through the app's middleware:

```rust,ignore
use rust_web_server::mcp::{openapi_tools::ToolOptions, McpServer};
use rust_web_server::openapi::{OpenApiConfig, Operation};

let config = OpenApiConfig::new("Invoices", "1.0")
    .operation("GET", "/invoices/:id", Operation::new("Get an invoice").operation_id("getInvoice")
        .path_param_schema("id", r#"{"type":"string"}"#, "The invoice"));
let app = App::with_state(state).get("/invoices/:id", get_invoice).post("/invoices", create_invoice);
let spec = app.openapi_spec(&config);           // `openapi` feature
let mcp = McpServer::new("invoices", "1.0")
    .openapi_tools(app.clone(), &spec, &ToolOptions::default())?
    .require_bearer_tokens([("token", "alice")])
    .wrap(app.openapi(config));                 // the HTTP API stays as it was
```

The handler sees the MCP caller's identity (the token's `sub`) in the
`X-Rws-Mcp-Caller` header. A non-2xx answer is a tool error with its status
and body. `openapi_tools` takes any `Application` and any OpenAPI document,
so it also works for apps whose document is written by hand.

## Through the MCP gateway

The `rws` binary's [MCP gateway](/mcp/gateway/) takes an API as a source next
to MCP upstreams: its operations become `<prefix>__<operation>` tools, each
call an HTTP request to `base_url` with the API's own credentials (never the
caller's token), the gateway's allow rules, limits and audit log applying as
to any tool.

```toml
[[route.action.mcp.openapi]]
prefix      = "billing"
spec        = "https://billing.internal/openapi.json"   # or a file path
base_url    = "https://billing.internal"                # calls go only here, without redirects
include     = ["getInvoice", "listInvoices"]            # operationIds; default: every GET
exclude     = []
timeout_ms  = 30000
headers_env = { Authorization = "BILLING_API_AUTHORIZATION" }
```

The document is read when the gateway starts (and on reload). A JSON object
answer is the result's `structuredContent` when the operation declares an
object response; other answers are text; a non-2xx answer is a tool error
carrying its status and body (results are cut at 1 MiB). In code:
`McpGateway::openapi(OpenApiUpstream::new(prefix, spec, base_url))`.

## What becomes a tool

- **Only `GET` operations**, unless `ToolOptions::include` lists
  operationIds: an agent changing data should be a decision you make per
  operation. `exclude` drops some.
- **Names**: the `operationId` (characters outside `A-Za-z0-9_-` become
  `_`, at most 64), else `<method>_<path>`; `prefix` adds `<prefix>__`;
  a repeated name gets `_2`, `_3`.
- **Description**: `summary`, then `description` (cut at 1 KiB).
- **Annotations**: `GET`/`HEAD` read-only, `DELETE` destructive,
  `GET`/`PUT`/`DELETE` idempotent, all open-world.

## Building the request

`request(arguments)` fills path parameters (percent-encoded, so a value
can't leave its segment), adds query parameters (arrays repeat the name),
headers (control characters refused) and the `body` argument as JSON.
Cookie parameters aren't sent.

## Hostile documents

Only local `$ref`s (`#/components/...`) are followed; remote ones become
`{}` and are never fetched. A cycle, a reference chain over 24 deep, a
schema nested over 64 levels or one that would inline to more than 50,000
values (refs that fan out) stops there with `{}`. Documents over 16 MB are
refused.
