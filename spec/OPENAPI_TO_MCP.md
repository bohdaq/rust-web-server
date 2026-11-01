[Read Me](../README.md) > [Spec](.) > OpenAPI to MCP tools

# OpenAPI → MCP tools (18.19.0)

Planned 2026-10-31. Most services an agent should use already have an
OpenAPI document; writing an MCP tool per operation by hand is busywork.
Kong's AI MCP proxy, FastMCP's `from_openapi`, Speakeasy and Stainless
generate tools from OpenAPI; rws can do it at runtime, both for its own
routes and for upstream APIs it proxies.

## Two directions

**1. Your app's own routes** (`openapi` + MCP): routes described with
`OpenApiConfig::operation(…)` (summary, body, query and path types) become
MCP tools on the app's `McpServer`, calling the handler in-process — no
HTTP round trip, the request built from the tool arguments.

```rust,ignore
let app = App::with_state(state)
    .get("/invoices/:id", get_invoice)
    .post("/invoices", create_invoice)
    .openapi(OpenApiConfig::new("Invoices", "1.0")
        .operation("GET", "/invoices/:id", Operation::new("Get an invoice").path_param::<u64>("id")))
    .mcp_tools_from_routes(McpRoutes::described_only().read_only_get());
```

**2. An upstream API** (config file, proxy): an MCP endpoint whose tools
are an upstream service's OpenAPI operations, each call forwarded as an
HTTP request — an MCP gateway source next to MCP upstreams.

```toml
[[route]]
name = "billing-mcp"
[route.match]
path = "/mcp/billing"
[route.action]
type = "mcp"
[[route.action.openapi]]
name    = "billing"
spec    = "https://billing.internal/openapi.json"   # or a file
base_url = "https://billing.internal"
include = ["getInvoice", "listInvoices"]          # operationIds; default: all GET
auth_header_env = "BILLING_TOKEN"                 # sent upstream, never to the client
```

## Mapping

- Tool name: `operationId` (sanitized to MCP's `[A-Za-z0-9_-]{1,64}`), else
  `<method>_<path>`; prefixed `<source>__` like gateway tools; collisions
  get a numeric suffix and a startup warning.
- Description: `summary`, then `description` (trimmed to 1 KiB).
- `inputSchema`: an object with one property per path, query and header
  parameter (their schemas) plus `body` for `requestBody`
  (`application/json` schema); `required` from the spec. Local `$ref`s
  (`#/components/...`) inlined with a depth cap (cycles → `{}`); remote
  refs refused.
- `outputSchema` from the 2xx `application/json` response when present.
- Annotations: `GET`/`HEAD` → `readOnlyHint`; `DELETE` →
  `destructiveHint`; `PUT` → `idempotentHint`.
- Call: path params percent-encoded into the path, query params encoded,
  JSON body; response → `structuredContent` for JSON, text otherwise;
  non-2xx → `isError: true` with status and body (truncated).
- Default exposure: only `GET` operations unless `include` lists others —
  write operations are an explicit choice.

## Safety

- Upstream calls go only to `base_url` (no URL from the spec's `servers`
  unless chosen), redirects not followed across hosts.
- Per-source timeout, body size cap, rate limit via the route's middleware,
  the MCP gateway's `allow` rules per tool, audit log lines per call.
- Spec fetched at startup and on reload; a spec that fails to parse keeps
  the previous tools.

## Parsing

OpenAPI 3.0 and 3.1, JSON (YAML only if a user asks: no YAML parser today).
A bounded, loop-based JSON value parser (`service_discovery::json_lite`'s,
with a depth and size cap) — a stack-depth case and a fuzz target
(`openapi_to_mcp`) per CLAUDE.md.

## Tranches

| # | Scope | Status |
|---|---|---|
| 1 | **done 2026-11-01**: `mcp::openapi_tools` — `tools(spec, &ToolOptions)` (3.0/3.1 JSON; Swagger 2 and others refused; 16 MB cap), names (operationId sanitized ≤ 64, else method_path, prefix, `_2` suffixes), path-level + operation parameters merged (op wins), cookie params dropped, JSON body as `body`, `outputSchema` only for object 2xx JSON, annotations by method, GET-only default + include/exclude; `$ref` local only, resolved with depth 24, schemas inlined with cycle → `{}`, nesting cap 64 and a 50,000-node budget (fan-out bombs); `OpenApiTool::request` (percent-encoded path params, repeated array query params, header control chars refused, body JSON). Tests: 4 (Petstore 3.0 with refs/cycle, include/exclude/prefix/collisions, request building, refused docs + 2^30 fan-out bomb + remote ref) + stack-depth case + fuzz `openapi_to_mcp`; docs `mcp/openapi-tools.md` | done |
| 2 | **done 2026-11-01**: `mcp::gateway::OpenApiUpstream` (prefix, spec URL or file, base_url, headers, include/exclude, timeout) + `McpGateway::openapi`; spec read at build (configured headers, no redirects), tools registered with output schemas and annotations, calls to `base_url` + built path (no redirects), JSON object → `structuredContent` when declared, else json/text, non-2xx → tool error with status + body, results cut at 1 MiB; prefixes unique across MCP and OpenAPI sources. Config `[[route.action.mcp.openapi]]` (`prefix`, `spec`, `base_url`, `include`, `exclude`, `timeout_ms`, `headers_env`), validation (prefix, spec file, base_url, env), unknown keys. Tests: gateway (tools listed GET-only with annotations/outputSchema, call through a stand-in API with the API's credentials and not the caller's, 404 → tool error, schema-required args; unusable sources) + config parse/check. A Python-API e2e was replaced by the in-process stand-in API | done |
| 3 | **done 2026-11-01**: `McpServer::openapi_tools(app, spec, &ToolOptions)` (any `Application` + document; each call → `Request` built from the tool's `ToolRequest`, Host localhost, JSON body, caller `sub` as `X-Rws-Mcp-Caller` (a spec header of that name is dropped), executed in-process; non-2xx → tool error; object JSON → structuredContent; annotations open-world false) and `AppWithState::openapi_spec(&config)` (`openapi`). Tests: in-process GET/POST/404/caller identity; with `openapi`, the app's generated document → tools → call | done |
| 4 | **docs done 2026-11-01**: `mcp/openapi-tools.md` (converter, app routes, gateway sources, hostile documents), config reference, API row, CHANGELOG. Release 18.19.0: waiting for the user's go | docs done |
