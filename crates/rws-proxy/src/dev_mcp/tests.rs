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
    for tool in ["routes", "config", "layout", "recent_requests", "recent_errors"] {
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

#[test]
fn layout_reads_the_manifest_and_the_paths_that_exist() {
    let dir = crate::test_env::TempPath::new("dev_mcp_layout");
    std::fs::create_dir_all(dir.join("src/routes")).unwrap();
    std::fs::create_dir_all(dir.join("public")).unwrap();
    std::fs::write(dir.join("src/main.rs"), "fn main() {}").unwrap();
    std::fs::write(dir.join("src/routes/todos.rs"), "").unwrap();
    std::fs::write(dir.join("rws.app.toml"), "[app]\nname = \"todos\"\n[paths]\nstatic = \"public\"\nextra = \"x\"\n").unwrap();
    let client = TestClient::new(app().dev_mcp_with(dev().app_dir(&*dir)));
    let body = call(&client, "layout", "{}");
    assert!(body.contains(r#""manifest":"rws.app.toml","name":"todos","error":null"#), "{body}");
    assert!(body.contains(r#"{"role":"main","path":"src/main.rs","kind":"file","exists":true}"#), "{body}");
    assert!(body.contains(r#"{"role":"routes","path":"src/routes","kind":"dir","exists":true,"entries":["todos.rs"]}"#), "{body}");
    assert!(body.contains(r#"{"role":"models","path":"src/models","kind":"dir","exists":false}"#), "{body}");
    assert!(body.contains(r#"{"role":"static","path":"public","kind":"dir","exists":true,"entries":[]}"#), "{body}");
    assert!(body.contains("unknown key `extra`"), "warnings reported: {body}");

    // Edits show on the next call; a broken manifest is reported, not fatal.
    std::fs::write(dir.join("rws.app.toml"), "[paths]\nroutes = \"/etc\"\n").unwrap();
    let body = call(&client, "layout", "{}");
    assert!(!body.contains(r#""isError":true"#), "{body}");
    assert!(body.contains("rws.app.toml line 2:") && body.contains("absolute"), "{body}");
    assert!(body.contains(r#""path":"src/routes""#), "the conventional paths beside the error: {body}");
}

#[test]
fn layout_without_a_manifest_is_the_convention() {
    let dir = crate::test_env::TempPath::new("dev_mcp_layout_none");
    std::fs::create_dir_all(&*dir).unwrap();
    std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"blog\"\n").unwrap();
    let body = call(&TestClient::new(app().dev_mcp_with(dev().app_dir(&*dir))), "layout", "{}");
    assert!(body.contains(r#""manifest":null,"name":"blog","error":null"#), "{body}");
    assert_eq!(body.matches(r#""exists":false"#).count(), 9, "{body}");
}

/// The names `tools/list` lists.
fn tool_names(list: &str) -> Vec<String> {
    list.split(r#"{"name":""#).skip(1).filter_map(|rest| rest.split('"').next()).map(str::to_string).collect()
}

/// Every tool the endpoint lists answers a call without arguments, and
/// none with an error — the 18.28 "done when" check on this crate's side
/// (`ci/e2e/starters.sh` runs the same against each starter).
fn every_listed_tool_answers<A: Application>(client: &TestClient<A>) -> Vec<String> {
    let names = tool_names(&tools_list(client, None));
    assert!(names.len() >= 5, "{names:?}");
    // The generators need a name and write files: `generators_*` below.
    for name in names.iter().filter(|n| !n.starts_with("gen_")) {
        let body = call(client, name, "{}");
        assert!(!body.contains(r#""isError":true"#) && !body.contains(r#""error":{"#), "{name}: {body}");
    }
    names
}

#[test]
fn every_dev_tool_answers() {
    let names = every_listed_tool_answers(&TestClient::new(app().dev_mcp_with(dev())));
    for tool in ["routes", "config", "layout", "recent_requests", "recent_errors"] {
        assert!(names.iter().any(|n| n == tool), "{tool}: {names:?}");
    }
}

// ── generators ───────────────────────────────────────────────────────────────

/// A project in the conventional layout: `app()` in `src/lib.rs`.
fn gen_project(name: &str) -> crate::test_env::TempPath {
    let dir = crate::test_env::TempPath::new(name);
    let _ = std::fs::remove_dir_all(&*dir);
    std::fs::create_dir_all(dir.join("src/routes")).unwrap();
    std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"blog\"\n\n[dependencies]\nrust-web-server = { version = \"18\", features = [\"macros\"] }\n").unwrap();
    std::fs::write(dir.join("src/lib.rs"), "pub mod routes;\n\npub fn app() -> AppWithState<()> {\n    routes::todos::register(App::with_state(()))\n}\n").unwrap();
    std::fs::write(dir.join("src/routes/mod.rs"), "pub mod todos;\n").unwrap();
    dir
}

#[test]
fn generators_are_listed_unless_read_only() {
    let list = tools_list(&TestClient::new(app().dev_mcp_with(dev())), None);
    for tool in GEN_TOOLS {
        assert!(listed(&list, tool), "{tool}: {list}");
    }
    let list = tools_list(&TestClient::new(app().dev_mcp_with(dev().read_only())), None);
    for tool in GEN_TOOLS {
        assert!(!listed(&list, tool), "{tool}: {list}");
    }
}

#[test]
fn generators_plan_write_and_refuse_to_overwrite() {
    let dir = gen_project("dev_mcp_gen");
    let client = TestClient::new(app().dev_mcp_with(dev().app_dir(&*dir)));

    // A dry run answers the plan and writes nothing.
    let body = call(&client, "gen_route", r#"{"resource":"posts","fields":["title:String"],"methods":["list","show"],"prefix":"/api","dry_run":true}"#);
    assert!(body.contains(r#""status":"planned","written":false"#) && body.contains(r#""path":"src/routes/posts.rs","action":"create""#), "{body}");
    assert!(body.contains("/api/posts/{id}") && !body.contains("#[post("), "{body}");
    assert!(!dir.join("src/routes/posts.rs").exists());

    // Written; the name argument sent first is not the tool's name.
    let (status, body) = rpc(&client, None, r#"{"jsonrpc":"2.0","method":"tools/call","id":3,"params":{"arguments":{"name":"add_index"},"name":"gen_migration"}}"#);
    assert_eq!(200, status, "{body}");
    assert!(body.contains(r#""status":"written","written":true"#), "{body}");
    assert!(dir.join("migrations/0001_add_index.sql").is_file());
    let body = call(&client, "gen_migration", r#"{"name":"add_index"}"#);
    assert!(body.contains(r#""status":"unchanged""#), "{body}");

    // A file in the way: a tool error with the diff, the file kept.
    std::fs::write(dir.join("migrations/0001_add_index.sql"), "CREATE TABLE mine (id INTEGER);\n").unwrap();
    let body = call(&client, "gen_migration", r#"{"name":"add_index"}"#);
    assert!(body.contains(r#""isError":true"#) && body.contains(r#""status":"conflict""#) && body.contains("-CREATE TABLE mine"), "{body}");
    assert_eq!("CREATE TABLE mine (id INTEGER);\n", std::fs::read_to_string(dir.join("migrations/0001_add_index.sql")).unwrap());

    // Bad input is a tool error naming the problem.
    let body = call(&client, "gen_model", r#"{"name":"Post","fields":["title:u8"]}"#);
    assert!(body.contains(r#""isError":true"#) && body.contains(r#""status":"usage""#) && body.contains("unknown type"), "{body}");
    let body = call(&client, "gen_test", "{}");
    assert!(body.contains(r#""isError":true"#) && body.contains("resource"), "the schema says it is required: {body}");
    let body = call(&client, "gen_model", r#"{"name":"Post","fields":["title:String"],"db":"oracle"}"#);
    assert!(body.contains("db `oracle`"), "{body}");
    let body = call(&client, "gen_model", r#"{"name":"Post","fields":["title:String","views:i64"],"dry_run":true}"#);
    assert!(body.contains(r#""path":"src/models/post.rs","action":"create""#) && body.contains("views"), "{body}");
    let body = call(&client, "gen_model", r#"{"name":"Post","fields":["title:String"],"timestamps":true,"soft_delete":true,"dry_run":true}"#);
    assert!(body.contains("--timestamps --soft-delete") && body.contains("#[soft_delete]") && body.contains("created_at"), "{body}");
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
        // 18.30.0: keys, indexes and foreign keys too (DbPool::schema).
        assert!(body.contains(r#""primary_key":["id"],"indexes":["#) && body.contains(r#""foreign_keys":[]"#), "{body}");
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

    struct Account;

    impl crate::model::Model for Account {
        fn table_name() -> &'static str { "users" }
        fn column_names() -> &'static [&'static str] { &["id", "email", "age"] }
        fn primary_key_name() -> &'static str { "id" }
        fn primary_key_value(&self) -> crate::model::Value { crate::model::Value::Int(0) }
        fn primary_key_auto_increment() -> bool { true }
        fn from_row(_: &crate::model::ModelRow) -> Result<Self, crate::model::DbError> { Ok(Account) }
        fn to_values(&self) -> Vec<(&'static str, crate::model::Value)> { Vec::new() }
    }

    struct Invoice;

    impl crate::model::Model for Invoice {
        fn table_name() -> &'static str { "invoices" }
        fn column_names() -> &'static [&'static str] { &["number"] }
        fn primary_key_name() -> &'static str { "number" }
        fn primary_key_value(&self) -> crate::model::Value { crate::model::Value::Int(0) }
        fn from_row(_: &crate::model::ModelRow) -> Result<Self, crate::model::DbError> { Ok(Invoice) }
        fn to_values(&self) -> Vec<(&'static str, crate::model::Value)> { Vec::new() }
    }

    #[test]
    fn models_without_a_database_report_the_mapping() {
        let client = TestClient::new(app().dev_mcp_with(dev().model::<Account>().model::<Account>()));
        let body = call(&client, "models", "{}");
        assert!(body.contains(r#""database":false"#), "{body}");
        assert!(body.contains(r#""name":"Account","table":"users","primary_key":"id","auto_increment":true"#), "{body}");
        assert!(body.contains(r#""columns":[{"name":"id","primary_key":true},{"name":"email","primary_key":false},{"name":"age","primary_key":false}]"#), "{body}");
        assert_eq!(body.matches(r#""table":"users""#).count(), 1, "registered twice, listed once: {body}");
        assert!(!body.contains("missing_columns"), "nothing to compare with: {body}");
    }

    #[test]
    fn models_with_none_registered_say_how() {
        let body = call(&TestClient::new(app().dev_mcp_with(dev())), "models", "{}");
        assert!(body.contains(r#""models":[]"#) && body.contains("DevTools::new().model::<YourModel>()"), "{body}");
    }

    #[test]
    fn models_with_a_database_compare_with_the_tables() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = TestClient::new(app().dev_mcp_with(dev().database(pool(&rt)).model::<Account>().model::<Invoice>()));
        let body = call(&client, "models", "{}");
        assert!(body.contains(r#""database":true"#), "{body}");
        assert!(body.contains(r#"{"name":"id","primary_key":true,"db":{"type":"INTEGER","nullable":true,"primary_key":true}}"#), "{body}");
        assert!(body.contains(r#"{"name":"age","primary_key":false,"db":null}"#), "{body}");
        assert!(body.contains(r#""table_exists":true,"missing_columns":["age"],"unmapped_columns":["name"]"#), "{body}");
        assert!(body.contains(r#""table":"invoices""#) && body.contains(r#""table_exists":false,"missing_columns":["number"],"unmapped_columns":[]"#), "{body}");
    }

    #[test]
    fn every_tool_answers_with_a_database_models_and_migrations() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let dir = migrations_dir("dev_mcp_every_tool");
        let app_dir = crate::test_env::TempPath::new("dev_mcp_every_tool_app");
        std::fs::create_dir_all(&*app_dir).unwrap();
        let tools = dev().database(pool(&rt)).migrations(dir.as_str()).model::<Account>().app_dir(&*app_dir);
        let names = every_listed_tool_answers(&TestClient::new(app().dev_mcp_with(tools)));
        for tool in ["layout", "models", "db_schema", "migrations", "migrations_run", "migrations_rollback_last"] {
            assert!(names.iter().any(|n| n == tool), "{tool}: {names:?}");
        }
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
