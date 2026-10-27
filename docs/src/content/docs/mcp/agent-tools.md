---
title: Build agent tools on your app
description: Expose an app's domain functions to AI agents as MCP tools — typed schemas, OAuth and live updates from the code the REST API already uses.
---

An app built on rws can answer AI agents with the same functions its REST
API calls. `app.mcp(name, version)` puts an MCP endpoint at `/mcp` in front
of the app; each domain function becomes a [typed tool](/mcp/tools/#typed-tools)
whose schemas come from its Rust types; everything else still reaches the
app. The runnable version is
[`examples/mcp_inventory.rs`](https://github.com/bohdaq/rust-web-server/blob/main/examples/mcp_inventory.rs).

```bash
MCP_TOKEN=$(openssl rand -hex 32) \
  cargo run --example mcp_inventory --features "macros serde sso openapi"
curl localhost:7878/items
claude mcp add --transport http inventory http://localhost:7878/mcp \
  --header "Authorization: Bearer $MCP_TOKEN"
```

## One domain, two front doors

Keep the logic in plain functions over plain types, and let the REST
handlers and the tools both call them:

```rust
/// A stock change.
#[derive(Deserialize, JsonSchema)]
pub struct AdjustStock {
    pub sku: String,
    /// Units to add (positive) or remove (negative).
    pub delta: i64,
}

impl Inventory {
    pub fn adjust(&self, change: &AdjustStock, changed_by: Option<String>) -> Result<Item, String> { /* ... */ }
}

let server = App::with_state(Arc::clone(&inventory))
    .post("/items/:sku/adjust", |request, params, _, inventory| { /* parse, inventory.adjust(..) */ })
    .mcp("inventory", "1.0")
    .typed_tool("adjust_stock", "Add or remove units of an item; fails rather than go below zero.",
        move |ctx, change: AdjustStock| inventory.adjust(&change, caller(ctx.auth_claims.as_deref())))
    .typed_tool("list_items", "List stocked items, optionally only those running low.",
        move |_, filter: ListItems| Ok(inventory.list(&filter)))
    .tool_annotations("list_items", ToolAnnotations { read_only_hint: Some(true), ..Default::default() });
```

- The doc comments on the types are what the model reads about each
  argument; write them for it.
- A domain error (`only 1 of TEE-02 on hand`) returns as an `isError`
  result the model can act on, not a protocol failure.
- Mark tools that only read with `read_only_hint`, so clients that ask
  before acting ask only for the ones that change something.

## Who is calling

`/mcp` needs a token; the REST routes keep their own rules.

- **One shared token:** `.require_bearer(token)`.
- **Your identity provider:** `.require_oauth(OidcProvider::discover(issuer)?,
  audience)`. rws checks the token's signature, issuer, expiry and audience,
  serves `/.well-known/oauth-protected-resource` so clients can find the
  provider, and hands the verified claims to every tool as
  `ctx.auth_claims` — the example records the caller's e-mail on each stock
  change. The example takes the issuer and audience from `MCP_OAUTH_ISSUER`
  and `MCP_OAUTH_AUDIENCE`. See [Auth](/mcp/auth/).

## Live updates

A resource plus `notify_resource_updated` lets a client keep a view
current. The example serves `inventory://items` and, whenever the stock
changes — through a tool or through the REST API — tells every session
subscribed to it:

```rust
let notifier = server.clone();
inventory.on_change(move || notifier.notify_resource_updated("inventory://items"));
```

A client holding `GET /mcp` open (SSE) and subscribed with
`resources/subscribe` receives
`{"method":"notifications/resources/updated","params":{"uri":"inventory://items"}}`
and reads the resource again.

## Testing

`TestClient` drives the whole stack, auth included, without a socket — the
example's tests call tools over `/mcp`, then check the change through the
REST route:

```rust
let adjusted = call(&client, "adjust_stock", r#"{"sku":"TEE-02","delta":-2}"#);
assert!(adjusted.contains(r#""onHand":1"#));
assert!(client.get("/items/TEE-02").send().body_text().contains(r#""onHand":1"#));
```

`cargo test --example mcp_inventory --features "macros serde sso openapi"` runs them.
