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
