//! An inventory service whose domain functions are both a REST API and MCP
//! tools an AI agent can call — the same code behind both. The REST API is
//! described in OpenAPI 3.1 from the same `JsonSchema` types (`/openapi.json`,
//! Swagger UI at `/docs`).
//!
//! ```sh
//! MCP_TOKEN=$(openssl rand -hex 32) cargo run --example mcp_inventory --features "macros serde sso openapi"
//! curl localhost:7878/items
//! curl localhost:7878/openapi.json
//! claude mcp add --transport http inventory http://localhost:7878/mcp --header "Authorization: Bearer $MCP_TOKEN"
//! ```
//!
//! With `MCP_OAUTH_ISSUER` and `MCP_OAUTH_AUDIENCE` set, `/mcp` takes tokens
//! from that identity provider instead of `MCP_TOKEN`. See
//! docs/src/content/docs/mcp/agent-tools.md.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

use rust_web_server::mcp::{McpContent, McpServer, ToolAnnotations};
use rust_web_server::openapi::{OpenApiConfig, Operation};
use rust_web_server::prelude::*;
use rust_web_server::response::StatusCodeReasonPhrase;
use rust_web_server::state::AppWithState;
use rust_web_server::JsonSchema;
use serde::{Deserialize, Serialize};

// ── The domain ───────────────────────────────────────────────────────────────

/// A stocked item.
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    /// Stock keeping unit, e.g. "MUG-01".
    pub sku: String,
    pub name: String,
    /// Units on hand.
    pub on_hand: u32,
    /// Price per unit in cents.
    pub price_cents: u64,
    /// Who last changed the stock level, when known.
    pub last_changed_by: Option<String>,
}

/// Filter for `list_items`.
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListItems {
    /// Only items with at most this many units on hand.
    pub max_on_hand: Option<u32>,
}

/// The matching items.
#[derive(Serialize, JsonSchema)]
pub struct Items {
    pub items: Vec<Item>,
}

/// Which item.
#[derive(Deserialize, JsonSchema)]
pub struct GetItem {
    pub sku: String,
}

/// A stock change.
#[derive(Deserialize, JsonSchema)]
pub struct AdjustStock {
    pub sku: String,
    /// Units to add (positive) or remove (negative).
    pub delta: i64,
}

/// The body of `POST /items/:sku/adjust`.
#[derive(Deserialize, JsonSchema)]
pub struct StockDelta {
    /// Units to add (positive) or remove (negative).
    pub delta: i64,
}

/// What a failed REST call answers.
#[derive(Serialize, JsonSchema)]
pub struct ApiError {
    pub error: String,
}

/// The items, the one place both the REST routes and the MCP tools change.
pub struct Inventory {
    items: Mutex<BTreeMap<String, Item>>,
    /// Told after every change, so MCP clients watching
    /// `inventory://items` refresh.
    on_change: OnceLock<Box<dyn Fn() + Send + Sync>>,
}

impl Inventory {
    pub fn new(items: impl IntoIterator<Item = Item>) -> Arc<Self> {
        Arc::new(Inventory {
            items: Mutex::new(items.into_iter().map(|item| (item.sku.clone(), item)).collect()),
            on_change: OnceLock::new(),
        })
    }

    /// Call `notify` after every stock change (once; later calls are ignored).
    pub fn on_change(&self, notify: impl Fn() + Send + Sync + 'static) {
        let _ = self.on_change.set(Box::new(notify));
    }

    pub fn list(&self, filter: &ListItems) -> Items {
        let items = self.items.lock().unwrap();
        let items = items.values().filter(|item| filter.max_on_hand.is_none_or(|max| item.on_hand <= max)).cloned().collect();
        Items { items }
    }

    pub fn get(&self, sku: &str) -> Result<Item, String> {
        self.items.lock().unwrap().get(sku).cloned().ok_or_else(|| format!("no item with SKU {sku}"))
    }

    pub fn adjust(&self, change: &AdjustStock, changed_by: Option<String>) -> Result<Item, String> {
        let item = {
            let mut items = self.items.lock().unwrap();
            let item = items.get_mut(&change.sku).ok_or_else(|| format!("no item with SKU {}", change.sku))?;
            let on_hand = i64::from(item.on_hand) + change.delta;
            item.on_hand = u32::try_from(on_hand)
                .map_err(|_| format!("only {} of {} on hand", item.on_hand, change.sku))?;
            item.last_changed_by = changed_by;
            item.clone()
        };
        if let Some(on_change) = self.on_change.get() {
            on_change();
        }
        Ok(item)
    }
}

// ── REST ─────────────────────────────────────────────────────────────────────

fn json_response(status: &'static StatusCodeReasonPhrase, body: String) -> Response {
    Response::json(status, body.into_bytes())
}

fn result_response(result: Result<Item, String>, failure: &'static StatusCodeReasonPhrase) -> Response {
    match result {
        Ok(item) => json_response(STATUS_CODE_REASON_PHRASE.n200_ok, serde_json::to_string(&item).unwrap_or_default()),
        Err(error) => json_response(failure, serde_json::json!({ "error": error }).to_string()),
    }
}

fn rest(inventory: Arc<Inventory>) -> AppWithState<Arc<Inventory>> {
    App::with_state(inventory)
        .get("/items", |_, _, _, inventory| {
            let items = inventory.list(&ListItems { max_on_hand: None });
            json_response(STATUS_CODE_REASON_PHRASE.n200_ok, serde_json::to_string(&items).unwrap_or_default())
        })
        .get("/items/:sku", |_, params, _, inventory| {
            result_response(inventory.get(params.get("sku").unwrap_or_default()), STATUS_CODE_REASON_PHRASE.n404_not_found)
        })
        .post("/items/:sku/adjust", |request, params, _, inventory| {
            let Ok(body) = serde_json::from_slice::<StockDelta>(&request.body) else {
                return json_response(STATUS_CODE_REASON_PHRASE.n400_bad_request, r#"{"error":"expected {\"delta\": n}"}"#.into());
            };
            let change = AdjustStock { sku: params.get("sku").unwrap_or_default().to_string(), delta: body.delta };
            result_response(inventory.adjust(&change, None), STATUS_CODE_REASON_PHRASE.n409_conflict)
        })
        .openapi(openapi())
}

/// The REST API's OpenAPI description: the same types the handlers and the
/// MCP tools use.
fn openapi() -> OpenApiConfig {
    OpenApiConfig::new("Inventory", env!("CARGO_PKG_VERSION"))
        .description("Stock levels, as a REST API (and as MCP tools at `/mcp`).")
        .operation("GET", "/items", Operation::new("List every item").operation_id("listItems").tag("items").response::<Items>(200, "The items"))
        .operation(
            "GET",
            "/items/:sku",
            Operation::new("Look up one item")
                .operation_id("getItem")
                .tag("items")
                .path_param_schema("sku", r#"{"type":"string"}"#, "Stock keeping unit, e.g. MUG-01")
                .response::<Item>(200, "The item")
                .response::<ApiError>(404, "No item with that SKU"),
        )
        .operation(
            "POST",
            "/items/:sku/adjust",
            Operation::new("Add or remove stock")
                .operation_id("adjustStock")
                .tag("items")
                .path_param_schema("sku", r#"{"type":"string"}"#, "Stock keeping unit")
                .body::<StockDelta>()
                .response::<Item>(200, "The item after the change")
                .response::<ApiError>(400, "The body isn't {\"delta\": n}")
                .response::<ApiError>(409, "Unknown SKU, or the change would take stock below zero"),
        )
}

// ── MCP ──────────────────────────────────────────────────────────────────────

/// How `/mcp` checks callers.
pub enum McpAuth {
    /// One shared token (`MCP_TOKEN`).
    Bearer(String),
    /// Tokens from an OAuth 2.0 / OIDC provider, for `audience`.
    OAuth { issuer: String, audience: String },
}

/// The REST app with `/mcp` in front of it.
pub fn build(inventory: Arc<Inventory>, auth: McpAuth) -> Result<McpServer, String> {
    let read_only = || ToolAnnotations { read_only_hint: Some(true), ..Default::default() };
    let list = Arc::clone(&inventory);
    let get = Arc::clone(&inventory);
    let adjust = Arc::clone(&inventory);
    let resource = Arc::clone(&inventory);
    let server = rest(Arc::clone(&inventory))
        .mcp("inventory", "1.0")
        .title("Inventory")
        .typed_tool("list_items", "List stocked items, optionally only those running low.", move |_, filter: ListItems| {
            Ok(list.list(&filter))
        })
        .tool_annotations("list_items", read_only())
        .typed_tool("get_item", "Look up one item by SKU.", move |_, args: GetItem| get.get(&args.sku))
        .tool_annotations("get_item", read_only())
        .typed_tool("adjust_stock", "Add or remove units of an item; fails rather than go below zero.", move |ctx, change: AdjustStock| {
            adjust.adjust(&change, caller(ctx.auth_claims.as_deref()))
        })
        .resource("inventory://items", "items", "Every item, as JSON; subscribe to hear about stock changes.", move |_| {
            let items = resource.list(&ListItems { max_on_hand: None });
            serde_json::to_string(&items).map(McpContent::json).map_err(|e| e.to_string())
        });
    let server = match auth {
        McpAuth::Bearer(token) => server.require_bearer(token),
        McpAuth::OAuth { issuer, audience } => {
            let provider = rust_web_server::sso::OidcProvider::discover(&issuer).map_err(|e| e.to_string())?;
            server.require_oauth(provider, audience)
        }
    };
    let notifier = server.clone();
    inventory.on_change(move || notifier.notify_resource_updated("inventory://items"));
    Ok(server)
}

/// The verified caller's `sub` (or e-mail), from OAuth claims.
fn caller(claims: Option<&str>) -> Option<String> {
    let claims: serde_json::Value = serde_json::from_str(claims?).ok()?;
    claims.get("email").or_else(|| claims.get("sub"))?.as_str().map(str::to_string)
}

fn sample_items() -> Vec<Item> {
    let item = |sku: &str, name: &str, on_hand, price_cents| Item {
        sku: sku.into(),
        name: name.into(),
        on_hand,
        price_cents,
        last_changed_by: None,
    };
    vec![item("MUG-01", "Enamel mug", 24, 1200), item("TEE-02", "Logo T-shirt", 3, 2500), item("CAP-03", "Cap", 0, 1800)]
}

fn auth_from_env() -> Result<McpAuth, String> {
    let var = |name| std::env::var(name).ok().filter(|v: &String| !v.is_empty());
    match (var("MCP_OAUTH_ISSUER"), var("MCP_OAUTH_AUDIENCE"), var("MCP_TOKEN")) {
        (Some(issuer), Some(audience), _) => Ok(McpAuth::OAuth { issuer, audience }),
        (_, _, Some(token)) => Ok(McpAuth::Bearer(token)),
        _ => Err("set MCP_TOKEN, or MCP_OAUTH_ISSUER and MCP_OAUTH_AUDIENCE".into()),
    }
}

#[tokio::main]
async fn main() {
    let server = match auth_from_env().and_then(|auth| build(Inventory::new(sample_items()), auth)) {
        Ok(server) => server,
        Err(error) => {
            eprintln!("mcp_inventory: {error}");
            std::process::exit(2);
        }
    };
    let (listener, pool) = Server::setup().unwrap();
    Server::run_tls(listener, pool, server).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_web_server::test_client::TestClient;

    const TOKEN: &str = "test-token";

    fn client() -> (TestClient<McpServer>, Arc<Inventory>) {
        let inventory = Inventory::new(sample_items());
        let server = build(Arc::clone(&inventory), McpAuth::Bearer(TOKEN.into())).unwrap();
        (TestClient::new(server), inventory)
    }

    fn rpc(client: &TestClient<McpServer>, body: &str) -> String {
        let response = client
            .post("/mcp")
            .header("Authorization", &format!("Bearer {TOKEN}"))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .body_text(body)
            .send();
        assert_eq!(200, response.status(), "{}", response.body_text());
        response.body_text().to_string()
    }

    fn call(client: &TestClient<McpServer>, tool: &str, arguments: &str) -> String {
        rpc(client, &format!(r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"{tool}","arguments":{arguments}}}}}"#))
    }

    #[test]
    fn tools_have_schemas_from_the_domain_types() {
        let (client, _) = client();
        let list = rpc(&client, r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#);
        assert!(list.contains(r#""name":"adjust_stock""#), "{list}");
        assert!(list.contains(r#""delta":{"description":"Units to add (positive) or remove (negative).","type":"integer"}"#), "{list}");
        assert!(list.contains(r#""onHand":{"description":"Units on hand.","type":"integer","minimum":0}"#), "{list}");
        assert!(list.contains(r#""readOnlyHint":true"#), "{list}");
    }

    #[test]
    fn mcp_and_rest_share_the_domain() {
        let (client, inventory) = client();
        let low = call(&client, "list_items", r#"{"maxOnHand":3}"#);
        assert!(low.contains(r#""structuredContent":{"items":[{"sku":"CAP-03""#), "{low}");
        assert!(!low.contains("MUG-01"), "{low}");

        let adjusted = call(&client, "adjust_stock", r#"{"sku":"TEE-02","delta":-2}"#);
        assert!(adjusted.contains(r#""onHand":1"#), "{adjusted}");
        assert_eq!(1, inventory.get("TEE-02").unwrap().on_hand);
        let rest = client.get("/items/TEE-02").send();
        assert!(rest.body_text().contains(r#""onHand":1"#), "{}", rest.body_text());

        let refused = call(&client, "adjust_stock", r#"{"sku":"TEE-02","delta":-5}"#);
        assert!(refused.contains(r#""isError":true"#) && refused.contains("only 1 of TEE-02 on hand"), "{refused}");
        let missing = call(&client, "get_item", r#"{"sku":"NOPE"}"#);
        assert!(missing.contains(r#""isError":true"#), "{missing}");

        let posted = client.post("/items/MUG-01/adjust").body_text(r#"{"delta":1}"#).send();
        assert_eq!(200, posted.status());
        let resource = rpc(&client, r#"{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":"inventory://items"}}"#);
        assert!(resource.contains(r#"\"onHand\":25"#), "{resource}");
    }

    #[test]
    fn mcp_needs_the_token_and_rest_does_not() {
        let (client, _) = client();
        let anonymous = client.post("/mcp").header("Content-Type", "application/json").body_text(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).send();
        assert_eq!(401, anonymous.status());
        assert_eq!(200, client.get("/items").send().status());
    }

    #[test]
    fn the_caller_comes_from_oauth_claims() {
        assert_eq!(Some("a@example.com".to_string()), caller(Some(r#"{"sub":"1","email":"a@example.com"}"#)));
        assert_eq!(Some("1".to_string()), caller(Some(r#"{"sub":"1"}"#)));
        assert_eq!(None, caller(None));
        assert_eq!(None, caller(Some("not json")));
    }

    #[test]
    fn the_rest_api_is_described_in_openapi_3_1() {
        let (client, _) = client();
        let response = client.get("/openapi.json").send();
        assert_eq!(200, response.status());
        let spec: serde_json::Value = serde_json::from_slice(response.body_bytes()).unwrap();
        assert_eq!("3.1.0", spec["openapi"]);
        let adjust = &spec["paths"]["/items/{sku}/adjust"]["post"];
        assert_eq!("adjustStock", adjust["operationId"]);
        assert_eq!("integer", adjust["requestBody"]["content"]["application/json"]["schema"]["properties"]["delta"]["type"]);
        let item = &adjust["responses"]["200"]["content"]["application/json"]["schema"];
        assert!(item["properties"]["onHand"].is_object(), "serde's camelCase names: {item}");
        assert!(adjust["responses"]["409"]["content"]["application/json"]["schema"]["properties"]["error"].is_object());
        assert_eq!("sku", spec["paths"]["/items/{sku}"]["get"]["parameters"][0]["name"]);
    }
}
