use super::*;
use crate::app::App;
use crate::core::New;
use crate::test_client::TestClient;

/// An environment variable no test sets: "no token configured".
const UNSET: &str = "RWS_DEV_MCP_TEST_NEVER_SET_TOKEN";

fn ok(_: &Request, _: &crate::router::PathParams, _: &ConnectionInfo, _: &()) -> Response {
    let mut r = Response::new();
    r.status_code = 200;
    r
}

/// A layer that does nothing, to be listed.
struct TagLayer;

impl crate::middleware::Middleware for TagLayer {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        next.execute(request, connection)
    }
}

fn app() -> WithMiddleware<crate::state::AppWithState<()>> {
    App::with_state(()).get("/api/items/:id", ok).post("/api/items", ok).wrap(TagLayer)
}

fn dev() -> DevTools {
    DevTools::new().env(RwsEnv::Dev).token_env(UNSET)
}

/// A JSON-RPC call; the answer with the inner tool JSON unescaped.
fn rpc<A: Application>(client: &TestClient<A>, token: Option<&str>, body: &str) -> (i16, String) {
    let mut req = client.post("/mcp").body_json(body);
    if let Some(token) = token {
        req = req.header("Authorization", &format!("Bearer {token}"));
    }
    let resp = req.send();
    (resp.status(), resp.body_text().replace("\\\"", "\""))
}

fn tools_list<A: Application>(client: &TestClient<A>, token: Option<&str>) -> String {
    let (status, body) = rpc(client, token, r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#);
    assert_eq!(status, 200, "{body}");
    body
}

fn call<A: Application>(client: &TestClient<A>, name: &str, args: &str) -> String {
    let (status, body) =
        rpc(client, None, &format!(r#"{{"jsonrpc":"2.0","method":"tools/call","id":2,"params":{{"name":"{name}","arguments":{args}}}}}"#));
    assert_eq!(status, 200, "{body}");
    body
}

fn listed(list: &str, tool: &str) -> bool {
    list.contains(&format!(r#""name":"{tool}""#))
}

// ── gating ───────────────────────────────────────────────────────────────────

#[test]
fn prod_without_a_token_mounts_nothing() {
    let tools = DevTools::new().env(RwsEnv::Prod).token_env(UNSET);
    assert!(!tools.enabled());
    let served = app().dev_mcp_with(tools);
    assert!(!served.is_mounted());
    let client = TestClient::new(served);
    let (status, body) = rpc(&client, None, r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#);
    assert_ne!(status, 200, "no MCP endpoint in prod without a token: {body}");
    assert!(!body.contains("jsonrpc"), "{body}");
    // The app itself is untouched.
    assert_eq!(client.get("/api/items/1").send().status(), 200);
}

#[test]
fn test_env_without_a_token_is_off_too() {
    assert!(!DevTools::new().env(RwsEnv::Test).token_env(UNSET).enabled());
    assert!(!DevTools::new().env(RwsEnv::Prod).token("   ").token_env(UNSET).enabled(), "a blank token is none");
}

#[test]
fn a_token_mounts_it_in_prod_and_is_required() {
    let served = app().dev_mcp_with(DevTools::new().env(RwsEnv::Prod).token("s3cret").token_env(UNSET));
    assert!(served.is_mounted());
    let client = TestClient::new(served);
    let (status, _) = rpc(&client, None, r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#);
    assert_eq!(status, 401);
    let (status, _) = rpc(&client, Some("wrong"), r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#);
    assert_eq!(status, 401);
    let list = tools_list(&client, Some("s3cret"));
    assert!(listed(&list, "routes"), "{list}");
    // Changing tools come with the token.
    let served = app().dev_mcp_with(DevTools::new().env(RwsEnv::Prod).token("s3cret").with_ops());
    let list = tools_list(&TestClient::new(served), Some("s3cret"));
    assert!(listed(&list, "block_ip"), "{list}");
}

#[test]
fn a_token_from_the_environment_counts() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_DEV_MCP_TEST_TOKEN", "from-env");
    let tools = DevTools::new().env(RwsEnv::Prod).token_env("RWS_DEV_MCP_TEST_TOKEN");
    let enabled = tools.enabled();
    let client = TestClient::new(app().dev_mcp_with(tools));
    std::env::remove_var("RWS_DEV_MCP_TEST_TOKEN");
    assert!(enabled);
    assert_eq!(rpc(&client, None, r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#).0, 401);
    assert!(listed(&tools_list(&client, Some("from-env")), "config"));
}

#[test]
fn dev_without_a_token_is_open() {
    let served = app().dev_mcp_with(dev());
    assert!(served.is_mounted());
    let client = TestClient::new(served);
    let (status, body) = rpc(
        &client,
        None,
        r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"agent","version":"1"}}}"#,
    );
    assert_eq!(status, 200, "{body}");
    assert!(body.contains(r#""name":"rws-dev""#), "{body}");
    let list = tools_list(&client, None);
    for tool in ["routes", "config", "recent_requests", "recent_errors"] {
        assert!(listed(&list, tool), "{tool}: {list}");
    }
    assert!(!listed(&list, "server_metrics"), "operations tools only with with_ops(): {list}");
    // Other paths still reach the app.
    assert_eq!(client.get("/api/items/7").send().status(), 200);
}

// ── tools ────────────────────────────────────────────────────────────────────

#[test]
fn routes_lists_patterns_and_the_middleware_around_them() {
    let client = TestClient::new(app().dev_mcp_with(dev()));
    let body = call(&client, "routes", "{}");
    assert!(body.contains(r#"{"method":"GET","pattern":"/api/items/:id"}"#), "{body}");
    assert!(body.contains(r#"{"method":"POST","pattern":"/api/items"}"#), "{body}");
    assert!(body.contains(r#""middleware":["TagLayer"]"#), "the app's layers, not dev_mcp's own LogLayer: {body}");
}

#[test]
fn routes_through_an_app_without_routes_is_empty_not_an_error() {
    let client = TestClient::new(App::new().dev_mcp_with(dev()));
    let body = call(&client, "routes", "{}");
    assert!(body.contains(r#"{"routes":[],"middleware":[]}"#), "{body}");
}

#[test]
fn config_shows_settings_with_secrets_hidden() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_DEV_MCP_TEST_PASSWORD", "hunter2");
    std::env::set_var("RWS_DEV_MCP_TEST_URL", "postgres://app:pw-in-url@db:5432/app");
    std::env::set_var("RWS_DEV_MCP_TEST_PLAIN", "visible-value");
    let client = TestClient::new(app().dev_mcp_with(dev()));
    let body = call(&client, "config", "{}");
    std::env::remove_var("RWS_DEV_MCP_TEST_PASSWORD");
    std::env::remove_var("RWS_DEV_MCP_TEST_URL");
    std::env::remove_var("RWS_DEV_MCP_TEST_PLAIN");
    assert!(body.contains(r#""rws_env":"dev""#), "{body}");
    assert!(body.contains(r#""RWS_DEV_MCP_TEST_PASSWORD":"(hidden)""#), "{body}");
    assert!(body.contains("postgres://app:(hidden)@db:5432/app"), "{body}");
    assert!(body.contains("visible-value"), "{body}");
    assert!(!body.contains("hunter2") && !body.contains("pw-in-url"), "{body}");
    assert!(body.contains(r#""features":{"#), "{body}");
}

#[test]
fn masking() {
    assert_eq!(masked("RWS_CONFIG_PORT", "7878"), "7878");
    assert_eq!(masked("RWS_DB_PASSWORD", "x"), "(hidden)");
    assert_eq!(masked("MCP_TOKEN", "abc"), "(hidden)");
    assert_eq!(masked("RWS_DB_PASSWORD", ""), "", "nothing to hide");
    assert_eq!(masked("DATABASE_URL", "mysql://root:pw@localhost/db?x=1"), "mysql://root:(hidden)@localhost/db?x=1");
    assert_eq!(masked("DATABASE_URL", "postgres://app@db/app"), "postgres://app@db/app", "no password");
    assert_eq!(masked("RWS_X", "https://example.com/a:b@c"), "https://example.com/a:b@c", "@ in the path is not user info");
    assert_eq!(masked("RWS_X", "not a url: a@b"), "not a url: a@b");
    assert!(shown_setting("RWS_ENV") && shown_setting("OTEL_SERVICE_NAME") && shown_setting("DATABASE_URL"));
    assert!(!shown_setting("HOME") && !shown_setting("AWS_SECRET_ACCESS_KEY"));
}

#[test]
fn recent_requests_and_errors_see_the_apps_requests() {
    let client = TestClient::new(app().dev_mcp_with(dev()));
    assert_eq!(client.get("/api/items/dev-mcp-recent-42").send().status(), 200);
    assert_eq!(client.get("/dev-mcp-missing-page").send().status(), 404);
    let body = call(&client, "recent_requests", r#"{"n":100}"#);
    assert!(body.contains("/api/items/dev-mcp-recent-42"), "{body}");
    let body = call(&client, "recent_errors", r#"{"n":100}"#);
    assert!(body.contains("/dev-mcp-missing-page"), "{body}");
    assert!(!body.contains("dev-mcp-recent-42"), "a 200 is not an error: {body}");
}

#[test]
fn requests_are_not_recorded_when_told_not_to() {
    let client = TestClient::new(app().dev_mcp_with(dev().record_requests(false)));
    client.get("/api/items/dev-mcp-unrecorded-9").send();
    let body = call(&client, "recent_requests", r#"{"n":100}"#);
    assert!(!body.contains("dev-mcp-unrecorded-9"), "{body}");
}

#[test]
fn with_ops_adds_the_operations_tools_and_read_only_drops_the_changing_ones() {
    let list = tools_list(&TestClient::new(app().dev_mcp_with(dev().with_ops())), None);
    for tool in ["server_metrics", "list_routes", "block_ip", "set_maintenance", "routes", "config"] {
        assert!(listed(&list, tool), "{tool}: {list}");
    }
    assert!(!listed(&list, "cache_stats"), "cache tools need a cache: {list}");
    assert_eq!(list.matches(r#""name":"recent_requests""#).count(), 1, "registered once: {list}");

    let list = tools_list(&TestClient::new(app().dev_mcp_with(dev().with_ops().read_only())), None);
    for tool in OPS_WRITE_TOOLS {
        assert!(!listed(&list, tool), "{tool}: {list}");
    }
    assert!(listed(&list, "server_metrics") && listed(&list, "list_blocked_ips"), "{list}");
}

#[test]
fn ops_tools_is_the_binarys_set_of_25() {
    let cache = crate::cache::CacheLayer::memory(10);
    let mcp = ops_tools(McpServer::new("rws", "1"), vec![("GET".into(), "/api/version".into())], Some(cache));
    let list = tools_list(&TestClient::new(mcp), None);
    assert_eq!(list.matches(r#""inputSchema""#).count(), 25, "{list}");
    for tool in OPS_WRITE_TOOLS {
        assert!(listed(&list, tool), "{tool}");
    }
    // The order the binary has always listed them in.
    let at = |t: &str| list.find(&format!(r#""name":"{t}""#)).unwrap();
    assert!(at("server_config") < at("cache_clear") && at("cache_clear") < at("reload_config") && at("feature_set") < at("recent_requests"));
    assert!(list.contains(r#""readOnlyHint":true"#), "{list}");
    let routes = call(&TestClient::new(ops_tools(McpServer::new("rws", "1"), vec![("GET".into(), "/api/version".into())], None)), "list_routes", "{}");
    assert!(routes.contains(r#"[{"method":"GET","pattern":"/api/version"}]"#), "{routes}");

    let list = tools_list(&TestClient::new(ops_tools(McpServer::new("rws", "1"), Vec::new(), None)), None);
    assert_eq!(list.matches(r#""inputSchema""#).count(), 23, "without a cache: {list}");
}

#[test]
fn register_adds_the_tools_to_an_apps_own_server() {
    let mcp = dev().register(app().mcp("mine", "2.0"));
    let client = TestClient::new(mcp);
    let body = call(&client, "routes", "{}");
    assert!(body.contains("/api/items/:id") && body.contains("TagLayer"), "{body}");
}

#[test]
fn another_path_for_an_app_with_its_own_mcp() {
    let client = TestClient::new(app().dev_mcp_with(dev().at("/_dev/mcp")));
    let resp = client.post("/_dev/mcp").body_json(r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#).send();
    assert_eq!(resp.status(), 200);
    assert!(resp.body_text().contains("\"routes\""));
}

// ── database (model-sqlite) ─────────────────────────────────────────────────

#[cfg(feature = "model-sqlite")]
mod db {
    use super::*;
    use crate::model::DbPool;

    fn pool(rt: &tokio::runtime::Runtime) -> DbPool {
        rt.block_on(async {
            let pool = DbPool::memory().await.unwrap();
            pool.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT NOT NULL, name TEXT DEFAULT 'anon')", &[]).await.unwrap();
            pool
        })
    }

    fn migrations_dir(name: &str) -> crate::test_env::TempPath {
        let dir = crate::test_env::TempPath::new(name);
        std::fs::create_dir_all(dir.as_str()).unwrap();
        std::fs::write(format!("{}/0001_posts.sql", dir.as_str()), "CREATE TABLE posts (id INTEGER PRIMARY KEY, title TEXT)").unwrap();
        std::fs::write(format!("{}/0001_posts.down.sql", dir.as_str()), "DROP TABLE posts").unwrap();
        dir
    }

    #[test]
    fn db_schema_lists_tables_and_columns() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = TestClient::new(app().dev_mcp_with(dev().database(pool(&rt))));
        assert!(listed(&tools_list(&client, None), "db_schema"));
        let body = call(&client, "db_schema", "{}");
        assert!(body.contains(r#""backend":"sqlite""#), "{body}");
        assert!(body.contains(r#""name":"users""#), "{body}");
        assert!(body.contains(r#"{"name":"id","type":"INTEGER","nullable":true,"default":null,"primary_key":true}"#), "{body}");
        assert!(body.contains(r#"{"name":"email","type":"TEXT","nullable":false,"default":null,"primary_key":false}"#), "{body}");
        assert!(body.contains(r#""name":"name","type":"TEXT","nullable":true,"default":"'anon'""#), "{body}");
    }

    #[test]
    fn no_database_no_database_tools() {
        let list = tools_list(&TestClient::new(app().dev_mcp_with(dev())), None);
        assert!(!listed(&list, "db_schema") && !listed(&list, "migrations"), "{list}");
    }

    #[test]
    fn migrations_status_run_and_rollback() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let dir = migrations_dir("dev_mcp_migrations");
        let client = TestClient::new(app().dev_mcp_with(dev().database(pool(&rt)).migrations(dir.as_str())));
        let list = tools_list(&client, None);
        for tool in ["migrations", "migrations_run", "migrations_rollback_last"] {
            assert!(listed(&list, tool), "{tool}: {list}");
        }
        let body = call(&client, "migrations", "{}");
        assert!(body.contains(r#"{"version":"0001_posts.sql","applied":false,"has_down":true}"#), "{body}");
        let body = call(&client, "migrations_run", "{}");
        assert!(body.contains(r#""applied":true"#), "{body}");
        assert!(call(&client, "db_schema", "{}").contains(r#""name":"posts""#));
        let body = call(&client, "migrations_rollback_last", "{}");
        assert!(body.contains(r#"{"rolled_back":"0001_posts.sql"}"#), "{body}");
        assert!(!call(&client, "db_schema", "{}").contains(r#""name":"posts""#));
        let body = call(&client, "migrations_rollback_last", "{}");
        assert!(body.contains(r#"{"rolled_back":null}"#), "nothing left: {body}");
    }

    #[test]
    fn a_missing_migrations_directory_is_a_tool_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = TestClient::new(app().dev_mcp_with(dev().database(pool(&rt)).migrations("/nonexistent/rws-dev-mcp")));
        let body = call(&client, "migrations", "{}");
        assert!(body.contains(r#""isError":true"#), "{body}");
    }

    #[test]
    fn read_only_keeps_status_but_not_the_runners() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let dir = migrations_dir("dev_mcp_migrations_ro");
        let client = TestClient::new(app().dev_mcp_with(dev().database(pool(&rt)).migrations(dir.as_str()).read_only()));
        let list = tools_list(&client, None);
        assert!(listed(&list, "migrations") && listed(&list, "db_schema"), "{list}");
        for tool in DEV_WRITE_TOOLS {
            assert!(!listed(&list, tool), "{tool}: {list}");
        }
    }
}
