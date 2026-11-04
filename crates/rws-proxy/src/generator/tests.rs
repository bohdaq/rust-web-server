use super::names::*;
use super::scan::*;
use super::*;
use crate::test_env::TempPath;

const CARGO: &str = "[package]\nname = \"blog-app\"\nversion = \"0.1.0\"\n\n[dependencies]\nrust-web-server = { version = \"18.28\", features = [\"serde\", \"macros\"] }\nserde = { version = \"1\", features = [\"derive\"] }\ntokio = { version = \"1\", features = [\"rt-multi-thread\", \"macros\"] }\n";

const SYNC_LIB: &str = "//! The app.\n\npub mod routes;\n\nuse rust_web_server::prelude::*;\nuse rust_web_server::state::AppWithState;\n\n#[derive(Clone, Default)]\npub struct Store;\n\npub fn app() -> AppWithState<Store> {\n    routes::todos::register(App::with_state(Store::default())).openapi(openapi())\n}\n";

const ASYNC_LIB: &str = "pub mod routes;\npub mod store;\n\nuse rust_web_server::async_state::AsyncAppWithState;\npub use store::Store;\n\npub fn app(store: Store) -> AsyncAppWithState<Store> {\n    let app = App::with_async_state(store);\n    let app = routes::health::register(app);\n    routes::notes::register(app)\n}\n";

const MCP_LIB: &str = "pub mod tools;\n\npub fn server(token: &str) -> McpServer {\n    McpServer::new(\"x\", \"1\")\n}\n";

/// A project on disk: `files` as (path, content), deleted on drop.
fn project(name: &str, files: &[(&str, &str)]) -> TempPath {
    let dir = TempPath::new(&format!("gen_{name}"));
    let _ = std::fs::remove_dir_all(&*dir);
    for (path, content) in files {
        let full = dir.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, content).unwrap();
    }
    dir
}

fn api_project(name: &str) -> TempPath {
    project(name, &[("Cargo.toml", CARGO), ("src/lib.rs", SYNC_LIB), ("src/routes/mod.rs", "//! Routes.\n\npub mod todos;\n"), ("src/routes/todos.rs", "")])
}

fn change<'a>(plan: &'a Plan, path: &str) -> &'a FileChange {
    plan.changes.iter().find(|c| c.path == path).unwrap_or_else(|| panic!("no {path} in {:?}", plan.changes.iter().map(|c| &c.path).collect::<Vec<_>>()))
}

fn read(dir: &TempPath, path: &str) -> String {
    std::fs::read_to_string(dir.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn fields(specs: &[&str]) -> Vec<Field> {
    parse_fields(specs).unwrap()
}

fn model(name: &str, specs: &[&str]) -> Generate {
    Generate::Model { name: name.into(), fields: fields(specs), db: None }
}

fn route(resource: &str) -> Generate {
    Generate::Route { resource: resource.into(), fields: Vec::new(), actions: RouteAction::ALL.to_vec(), prefix: String::new() }
}

// ── names ────────────────────────────────────────────────────────────────────

#[test]
fn names_are_snake_camel_singular_and_plural() {
    assert_eq!("blog_post", to_snake("BlogPost", "model").unwrap());
    assert_eq!("blog_post", to_snake("blog_post", "model").unwrap());
    assert_eq!("BlogPost", to_camel("blog_post"));
    assert_eq!(("posts", "categories", "boxes", "days"), (&*pluralize("post"), &*pluralize("category"), &*pluralize("box"), &*pluralize("day")));
    assert_eq!(("post", "category", "box", "status"), (&*singularize("posts"), &*singularize("categories"), &*singularize("boxes"), &*singularize("status")));
}

#[test]
fn names_that_could_leave_the_project_or_break_rust_are_refused() {
    for bad in ["../etc", "a/b", "posts.rs", "", "1posts", "type", "Posts", "a__b", "x_", "gen"] {
        let err = check_snake(bad, "resource").unwrap_err();
        assert!(err.usage, "{bad}");
    }
    assert!(to_snake("../Up", "model").is_err());
    assert!(check_snake(&"a".repeat(MAX_NAME_LEN + 1), "migration").unwrap_err().message.contains("longer than"));
    assert!(check_type_name("Path", "resource").unwrap_err().message.contains("clash"));
}

#[test]
fn field_types_come_from_a_fixed_list() {
    let f = Field::parse("note:Option<String>").unwrap();
    assert_eq!(("note", "Option<String>"), (f.name.as_str(), f.ty.rust().as_str()));
    assert_eq!("views:i64", Field::parse("views: i64").unwrap().spec());
    for (bad, says) in [("x:u8", "unknown type `u8`"), ("x:Option<Option<i64>>", "unknown type"), ("x", "name:type"), ("id:i64", "already"), ("Bad:i64", "snake_case")] {
        let err = Field::parse(bad).unwrap_err();
        assert!(err.usage && err.message.contains(says), "{bad}: {}", err.message);
    }
    assert!(parse_fields(&["a:i64", "a:bool"]).unwrap_err().message.contains("twice"));
}

// ── scanning the project ─────────────────────────────────────────────────────

#[test]
fn app_fn_is_found_with_its_kind_state_and_parameters() {
    let sync = find_app_fn(SYNC_LIB).unwrap();
    assert_eq!((AppKind::Sync, "crate::Store", true), (sync.kind, sync.state.as_str(), sync.no_params));
    let r#async = find_app_fn(ASYNC_LIB).unwrap();
    assert_eq!((AppKind::Async, "crate::Store", false), (r#async.kind, r#async.state.as_str(), r#async.no_params));
    let unit = find_app_fn("pub fn app() -> rust_web_server::state::AppWithState<()> {\n    App::with_state(())\n}\n").unwrap();
    assert_eq!("()", unit.state);
    assert_eq!(None, find_app_fn(MCP_LIB));
    assert_eq!(None, find_app_fn("pub fn happ() -> AppWithState<S> {}"));
    // Unbalanced input is not an app, not a panic.
    assert_eq!(None, find_app_fn("pub fn app(((((( -> AppWithState<"));
}

#[test]
fn register_calls_are_wrapped_once() {
    let app = find_app_fn(SYNC_LIB).unwrap();
    let wired = wire_register(SYNC_LIB, &app, "routes", "posts").unwrap().unwrap();
    assert!(wired.contains("routes::posts::register(routes::todos::register(App::with_state(Store::default()))).openapi(openapi())"), "{wired}");
    let again = find_app_fn(&wired).unwrap();
    assert_eq!(Ok(None), wire_register(&wired, &again, "routes", "posts"));
    let app = find_app_fn(ASYNC_LIB).unwrap();
    let wired = wire_register(ASYNC_LIB, &app, "routes", "posts").unwrap().unwrap();
    assert!(wired.contains("let app = routes::posts::register(routes::health::register(app));"), "{wired}");
    let bare = "pub fn app() -> AppWithState<()> {\n    App::with_state(())\n}\n";
    assert_eq!(Err(()), wire_register(bare, &find_app_fn(bare).unwrap(), "routes", "posts"));
}

#[test]
fn mod_lines_go_in_order_or_after_the_header() {
    assert_eq!("pub mod a;\npub mod b;\npub mod c;\n", add_mod_line("pub mod a;\npub mod c;\n", "b").unwrap());
    assert_eq!("pub mod a;\npub mod z;\n", add_mod_line("pub mod a;\n", "z").unwrap());
    assert_eq!(None, add_mod_line("mod b;\n", "b"));
    assert_eq!("//! Docs.\n\npub mod models;\n\nuse x;\n", add_mod_line("//! Docs.\n\nuse x;\n", "models").unwrap());
    assert_eq!("pub mod models;\n\nuse x;\n", add_mod_line("use x;\n", "models").unwrap());
    assert_eq!("pub mod a;\n\npub use a::A;\npub use b::B;\n", add_use_line("pub mod a;\n\npub use a::A;\n", "pub use b::B;").unwrap());
    assert_eq!(None, add_use_line("pub use a::A;\n", "pub use a::A;"));
}

#[test]
fn cargo_features_are_added_to_a_one_line_dependency() {
    let added = add_rws_feature(CARGO, "model-sqlite").unwrap().unwrap();
    assert!(added.contains(r#"features = ["serde", "macros", "model-sqlite"] }"#), "{added}");
    assert_eq!(Ok(None), add_rws_feature(&added, "model-sqlite"));
    assert_eq!(Some(vec!["serde".to_string(), "macros".to_string(), "model-sqlite".to_string()]), rws_features(&added));
    let plain = "[dependencies]\nrust-web-server = \"18.28\"\n";
    assert_eq!("[dependencies]\nrust-web-server = { version = \"18.28\", features = [\"model-mysql\"] }\n", add_rws_feature(plain, "model-mysql").unwrap().unwrap());
    let no_features = "[dependencies]\nrust-web-server = { version = \"18.28\" }\n";
    assert_eq!("[dependencies]\nrust-web-server = { version = \"18.28\", features = [\"model-sqlite\"] }\n", add_rws_feature(no_features, "model-sqlite").unwrap().unwrap());
    // A table form is left to the person.
    assert_eq!(Err(()), add_rws_feature("[dependencies.rust-web-server]\nversion = \"18\"\n", "model-sqlite"));
    assert_eq!(Some("blog_app".to_string()), crate_name(CARGO));
    assert!(dependency_has(CARGO, "tokio", "macros") && !dependency_has(CARGO, "serde_json", "x"));
}

#[test]
fn test_client_expressions_and_route_attributes_are_read() {
    assert_eq!(Some("app(Store::memory())".to_string()), test_client_app_expr("let c = TestClient::new(app(Store::memory()));"));
    assert_eq!(None, test_client_app_expr("TestClient::new(other())"));
    let routes = declared_routes("#[get(\"/posts\")]\n  #[delete(\"/posts/{id}\")]\nfn x() {}\n");
    assert_eq!(vec![DeclaredRoute { method: "GET", pattern: "/posts".into() }, DeclaredRoute { method: "DELETE", pattern: "/posts/{id}".into() }], routes);
    let fields = struct_fields("pub struct NewPost {\n    pub title: String,\n    weird line\n}\n", "NewPost").unwrap();
    assert_eq!(vec![Ok(("title".to_string(), "String".to_string())), Err("weird line".to_string())], fields);
    assert_eq!(None, struct_fields("pub struct NewPost {\n    pub a: i64,\n", "NewPost"), "unterminated");
    assert_eq!((Some((7, 4)), None, None), (migration_number("0007_x.sql"), migration_number("x.sql"), migration_number("0007x.sql")));
}

#[test]
fn diffs_are_unified_with_context() {
    let diff = diff::unified("a.rs", Some("1\n2\n3\n4\n5\n6\n7\n8\n9\n"), "1\n2\n3\n4\nfive\n6\n7\n8\n9\n");
    assert_eq!("--- a/a.rs\n+++ b/a.rs\n@@ -2,7 +2,7 @@\n 2\n 3\n 4\n-5\n+five\n 6\n 7\n 8\n", diff);
    assert_eq!("", diff::unified("a.rs", Some("same\n"), "same\n"));
    assert_eq!("--- /dev/null\n+++ b/new.rs\n@@ -0,0 +1,1 @@\n+x\n", diff::unified("new.rs", None, "x\n"));
}

// ── model ────────────────────────────────────────────────────────────────────

#[test]
fn a_model_writes_its_type_migration_and_wiring() {
    let dir = api_project("model");
    std::fs::create_dir_all(dir.join("migrations")).unwrap();
    std::fs::write(dir.join("migrations/0003_older.sql"), "SELECT 1;").unwrap();
    let plan = plan(&*dir, &model("BlogPost", &["title:String", "views:i64", "draft:bool", "note:Option<String>"])).unwrap();
    assert_eq!("planned", plan.status());
    let file = change(&plan, "src/models/blog_post.rs");
    assert_eq!(ChangeKind::Create, file.kind);
    for expected in ["#[table(name = \"blog_posts\")]", "#[primary_key(auto_increment)]\n    pub id: i64,", "pub note: Option<String>,", "async fn round_trips_through_sqlite", "\"/migrations\""] {
        assert!(file.content.contains(expected), "{expected}: {}", file.content);
    }
    let up = change(&plan, "migrations/0004_create_blog_posts.sql");
    assert!(up.content.contains("id    INTEGER PRIMARY KEY AUTOINCREMENT,\n    title TEXT NOT NULL,\n    views INTEGER NOT NULL,\n    draft INTEGER NOT NULL,\n    note  TEXT\n);"), "{}", up.content);
    assert!(!up.content.split("\n").filter(|l| l.starts_with("--")).any(|l| l.contains(';')), "no `;` in comments: the migrator splits on it");
    assert_eq!("DROP TABLE blog_posts;\n", change(&plan, "migrations/0004_create_blog_posts.down.sql").content);
    assert!(change(&plan, "src/models/mod.rs").content.contains("pub mod blog_post;\n\npub use blog_post::BlogPost;\n"));
    assert!(change(&plan, "src/lib.rs").content.contains("pub mod models;\npub mod routes;"));
    assert_eq!(ChangeKind::Edit, change(&plan, "Cargo.toml").kind);
    assert!(change(&plan, "Cargo.toml").diff.contains("+rust-web-server = { version = \"18.28\", features = [\"serde\", \"macros\", \"model-sqlite\"] }"));
    assert!(plan.notes.iter().any(|n| n.contains(".model::<models::BlogPost>()")));

    // Written, then the same request changes nothing.
    let written = plan.apply(&*dir).unwrap();
    assert_eq!(6, written.len(), "{written:?}");
    let again = super::plan(&*dir, &model("BlogPost", &["title:String", "views:i64", "draft:bool", "note:Option<String>"])).unwrap();
    assert_eq!("unchanged", again.status(), "{}", again.render(true));
    assert!(again.changes.iter().any(|c| c.path == "migrations/0004_create_blog_posts.sql"), "the existing migration, not a new number");

    // Other fields: a conflict with a diff, nothing written.
    let before = read(&dir, "src/models/blog_post.rs");
    let other = super::plan(&*dir, &model("BlogPost", &["title:String"])).unwrap();
    assert!(other.has_conflicts());
    assert!(change(&other, "src/models/blog_post.rs").diff.contains("-    pub views: i64,"), "{}", other.render(false));
    assert!(other.apply(&*dir).unwrap_err().message.contains("not written"));
    assert_eq!(before, read(&dir, "src/models/blog_post.rs"));
    assert!(other.to_json(false).contains(r#""status":"conflict""#));
}

#[test]
fn a_model_follows_the_database_in_cargo_toml() {
    let cargo = CARGO.replace("\"macros\"] }\nserde", "\"macros\", \"model-postgres\"] }\nserde");
    let dir = project("model_pg", &[("Cargo.toml", &cargo), ("src/lib.rs", "pub mod models;\n"), ("src/models/mod.rs", "pub mod note;\n\npub use note::Note;\n")]);
    let plan = plan(&*dir, &model("Post", &["views:i64", "score:f64", "draft:bool", "rank:i32"])).unwrap();
    let up = &change(&plan, "migrations/0001_create_posts.sql").content;
    assert!(up.contains("BIGSERIAL PRIMARY KEY") && up.contains("BIGINT NOT NULL") && up.contains("DOUBLE PRECISION NOT NULL") && up.contains("BOOLEAN NOT NULL") && up.contains("rank  INTEGER NOT NULL"), "{up}");
    assert!(!change(&plan, "src/models/post.rs").content.contains("sqlite"), "no SQLite round trip for PostgreSQL");
    assert_eq!(ChangeKind::Unchanged, change(&plan, "Cargo.toml").kind);
    assert!(change(&plan, "src/models/mod.rs").content.contains("pub mod note;\npub mod post;\n\npub use note::Note;\npub use post::Post;\n"));

    let mysql = super::plan(&*dir, &Generate::Model { name: "Post".into(), fields: fields(&["views:i64", "rank:i32"]), db: Some(Db::Mysql) }).unwrap();
    assert!(change(&mysql, "migrations/0001_create_posts.sql").content.contains("BIGINT AUTO_INCREMENT PRIMARY KEY"));
    assert!(change(&mysql, "Cargo.toml").content.contains("\"model-postgres\", \"model-mysql\"]"), "the asked-for backend is added");

    let both = project("model_both", &[("Cargo.toml", &cargo.replace("\"model-postgres\"", "\"model-postgres\", \"model-mysql\""))]);
    assert!(super::plan(&*both, &model("Post", &["a:i64"])).unwrap_err().message.contains("--db"));
}

#[test]
fn a_model_needs_a_field_and_a_project() {
    let dir = api_project("model_errors");
    let err = plan(&*dir, &model("Post", &[])).unwrap_err();
    assert!(err.usage && err.message.contains("at least one field"));
    assert!(plan(&*dir, &model("Model", &["a:i64"])).unwrap_err().message.contains("clash"));
    let empty = project("no_cargo", &[("README.md", "")]);
    let err = plan(&*empty, &model("Post", &["a:i64"])).unwrap_err();
    assert!(!err.usage && err.message.contains("Cargo.toml"));
    let bad_manifest = project("bad_manifest", &[("Cargo.toml", CARGO), ("rws.app.toml", "[paths]\nmodels = \"../x\"\n")]);
    assert!(plan(&*bad_manifest, &model("Post", &["a:i64"])).unwrap_err().message.contains("rws.app.toml"));
}

// ── route ────────────────────────────────────────────────────────────────────

#[test]
fn a_route_writes_typed_handlers_and_registers_them() {
    let dir = api_project("route");
    let plan = plan(&*dir, &Generate::Route { resource: "posts".into(), fields: fields(&["title:String", "done:bool"]), actions: RouteAction::ALL.to_vec(), prefix: "/api/".into() }).unwrap();
    let file = &change(&plan, "src/routes/posts.rs").content;
    for expected in [
        "#[get(\"/api/posts\")]\npub fn list()",
        "#[get(\"/api/posts/{id}\")]\npub fn show(Path(id): Path<i64>)",
        "#[post(\"/api/posts\")]",
        "#[put(\"/api/posts/{id}\")]",
        "#[delete(\"/api/posts/{id}\")]\npub fn remove",
        "pub struct NewPost {\n    pub title: String,\n    pub done: bool,\n}",
        "pub fn register(app: AppWithState<crate::Store>) -> AppWithState<crate::Store> {\n    app.route(list).route(show).route(create).route(update).route(remove)\n}",
        "use rust_web_server::{delete, get, post, put};",
    ] {
        assert!(file.contains(expected), "{expected}:\n{file}");
    }
    assert!(change(&plan, "src/routes/mod.rs").content.ends_with("pub mod posts;\npub mod todos;\n"));
    assert!(change(&plan, "src/lib.rs").content.contains("routes::posts::register(routes::todos::register("));
    plan.apply(&*dir).unwrap();
    let again = super::plan(&*dir, &Generate::Route { resource: "posts".into(), fields: fields(&["title:String", "done:bool"]), actions: RouteAction::ALL.to_vec(), prefix: "/api".into() }).unwrap();
    assert_eq!("unchanged", again.status(), "{}", again.render(true));
}

#[test]
fn a_route_for_an_async_app_with_some_methods_and_the_models_fields() {
    let dir = project(
        "route_async",
        &[
            ("Cargo.toml", CARGO),
            ("src/lib.rs", ASYNC_LIB),
            ("src/routes/mod.rs", "pub mod health;\npub mod notes;\n"),
            ("src/models/post.rs", "pub struct Post {\n    #[primary_key(auto_increment)]\n    pub id: i64,\n    pub title: String,\n    pub views: Option<i64>,\n}\n"),
        ],
    );
    let actions = RouteAction::parse_list("show,list").unwrap();
    assert_eq!(vec![RouteAction::List, RouteAction::Show], actions);
    let plan = plan(&*dir, &Generate::Route { resource: "posts".into(), fields: Vec::new(), actions, prefix: String::new() }).unwrap();
    let file = &change(&plan, "src/routes/posts.rs").content;
    assert!(file.contains("pub fn register(app: AsyncAppWithState<crate::Store>) -> AsyncAppWithState<crate::Store> {\n    app.route(list).route(show)\n}"), "{file}");
    assert!(file.contains("pub views: Option<i64>,") && !file.contains("NewPost") && !file.contains("#[post"), "{file}");
    assert!(file.contains("use rust_web_server::get;\n") && file.contains("AppError::NotFound"), "{file}");
    assert!(plan.notes.iter().any(|n| n.contains("fields taken from src/models/post.rs")));
    assert!(change(&plan, "src/lib.rs").content.contains("let app = routes::posts::register(routes::health::register(app));"));
}

#[test]
fn a_route_is_refused_without_an_http_app_or_with_bad_input() {
    let mcp = project("route_mcp", &[("Cargo.toml", CARGO), ("src/lib.rs", MCP_LIB), ("rws.app.toml", "[paths]\nroutes = \"src/tools\"\n")]);
    let err = plan(&*mcp, &route("posts")).unwrap_err();
    assert!(!err.usage && err.message.contains("pub fn app("), "{}", err.message);
    let dir = api_project("route_bad");
    assert!(plan(&*dir, &route("paths")).unwrap_err().message.contains("clash"), "Path is imported by the module");
    assert!(plan(&*dir, &route("../posts")).unwrap_err().usage);
    let prefix = Generate::Route { resource: "posts".into(), fields: Vec::new(), actions: RouteAction::ALL.to_vec(), prefix: "api".into() };
    assert!(plan(&*dir, &prefix).unwrap_err().message.contains("--prefix"));
    assert!(RouteAction::parse_list("list,patch").unwrap_err().message.contains("patch"));
    // An app() with no register call to wrap: a note, not a guess.
    let bare = project("route_bare", &[("Cargo.toml", CARGO), ("src/lib.rs", "pub fn app() -> AppWithState<()> {\n    App::with_state(())\n}\n")]);
    let plan = plan(&*bare, &route("posts")).unwrap();
    assert!(change(&plan, "src/routes/mod.rs").kind == ChangeKind::Create);
    assert!(change(&plan, "src/lib.rs").content.starts_with("pub mod routes;\n\npub fn app()"));
    assert!(plan.notes.iter().any(|n| n.contains("let app = routes::posts::register(app);")), "{:?}", plan.notes);
}

// ── migration ────────────────────────────────────────────────────────────────

#[test]
fn a_migration_is_numbered_after_the_others() {
    let dir = project("migration", &[("Cargo.toml", CARGO), ("migrations/00009_a.sql", ""), ("migrations/00009_a.down.sql", ""), ("migrations/notes.txt", "")]);
    let plan = plan(&*dir, &Generate::Migration { name: "add_index".into() }).unwrap();
    assert_eq!(vec!["migrations/00010_add_index.sql", "migrations/00010_add_index.down.sql"], plan.changes.iter().map(|c| c.path.as_str()).collect::<Vec<_>>());
    assert!(change(&plan, "migrations/00010_add_index.sql").content.ends_with("SELECT 1;\n"));
    plan.apply(&*dir).unwrap();
    assert_eq!("unchanged", super::plan(&*dir, &Generate::Migration { name: "add_index".into() }).unwrap().status());
    assert!(super::plan(&*dir, &Generate::Migration { name: "Add Index".into() }).unwrap_err().usage);
}

// ── test ─────────────────────────────────────────────────────────────────────

#[test]
fn a_test_hits_the_generated_routes() {
    let dir = api_project("test");
    plan(&*dir, &Generate::Route { resource: "posts".into(), fields: fields(&["title:String", "score:f64"]), actions: RouteAction::ALL.to_vec(), prefix: String::new() })
        .unwrap()
        .apply(&*dir)
        .unwrap();
    let plan = plan(&*dir, &Generate::Test { resource: "posts".into(), app_expr: None }).unwrap();
    let test = &change(&plan, "tests/posts.rs").content;
    for expected in [
        "use blog_app::*;",
        "TestClient::new(app())",
        "client.post(\"/posts\").body_json(&json!({ \"title\": \"sample title\", \"score\": 1.5 }).to_string())",
        "assert_eq!(json!(2.5), json_of(&updated)[\"score\"]);",
        "client.get(\"/posts/not-a-number\")",
        "client.post(\"/posts\").body_json(\"not json\")",
    ] {
        assert!(test.contains(expected), "{expected}:\n{test}");
    }
}

#[test]
fn a_test_finds_how_to_build_an_app_that_takes_arguments() {
    let route = "#[get(\"/api/notes\")]\npub fn list() {}\n#[delete(\"/api/notes/{id}\")]\npub fn remove() {}\n";
    let files = [("Cargo.toml", CARGO), ("src/lib.rs", ASYNC_LIB), ("src/routes/notes.rs", route)];
    let dir = project("test_args", &files);
    let err = plan(&*dir, &Generate::Test { resource: "notes".into(), app_expr: None }).unwrap_err();
    assert!(err.usage && err.message.contains("--app-expr"), "{}", err.message);
    std::fs::create_dir_all(dir.join("tests")).unwrap();
    std::fs::write(dir.join("tests/other.rs"), "let client = TestClient::new(app(Store::memory()));").unwrap();
    let plan = plan(&*dir, &Generate::Test { resource: "notes".into(), app_expr: None }).unwrap();
    let test = &change(&plan, "tests/notes.rs").content;
    assert!(test.contains("TestClient::new(app(Store::memory()))") && test.contains("fn notes_are_listed()") && test.contains("client.delete(\"/api/notes/999999999\")"), "{test}");
    assert!(!test.contains("client.post"), "no create route: {test}");
    let bad = super::plan(&*dir, &Generate::Test { resource: "notes".into(), app_expr: Some("std::process::exit(1)".into()) });
    assert!(bad.unwrap_err().message.contains("--app-expr"));
}

#[test]
fn a_test_needs_the_routes_first() {
    let dir = api_project("test_missing");
    let err = plan(&*dir, &Generate::Test { resource: "posts".into(), app_expr: None }).unwrap_err();
    assert!(!err.usage && err.message.contains("rws gen route posts"), "{}", err.message);
    std::fs::write(dir.join("src/routes/todos.rs"), "pub fn nothing() {}\n").unwrap();
    assert!(plan(&*dir, &Generate::Test { resource: "todos".into(), app_expr: None }).unwrap_err().message.contains("by hand"));
}

// ── the command line ─────────────────────────────────────────────────────────

fn cli(args: &[&str]) -> (i32, String, String) {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(&args, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

#[test]
fn the_command_line_writes_plans_and_refuses_conflicts() {
    let dir = api_project("cli");
    let d = dir.as_str();
    let (code, out, _) = cli(&["--help"]);
    assert_eq!((EXIT_OK, true), (code, out.contains("Usage: rws gen")));

    let (code, out, _) = cli(&["migration", "first", "--dir", d, "--dry-run"]);
    assert_eq!(EXIT_OK, code);
    assert!(out.contains("create     migrations/0001_first.sql") && out.contains("+SELECT 1;"), "{out}");
    assert!(!dir.join("migrations").exists(), "a dry run writes nothing");

    let (code, out, _) = cli(&["migration", "first", "--dir", d]);
    assert_eq!(EXIT_OK, code, "{out}");
    assert!(dir.join("migrations/0001_first.sql").is_file());
    let (code, out, _) = cli(&["migration", "first", "--dir", d]);
    assert_eq!((EXIT_OK, true), (code, out.contains("nothing to do")));

    std::fs::write(dir.join("migrations/0001_first.sql"), "CREATE TABLE mine (id INTEGER);\n").unwrap();
    let (code, out, err) = cli(&["migration", "first", "--dir", d]);
    assert_eq!(EXIT_CONFLICT, code);
    assert!(out.contains("conflict   migrations/0001_first.sql") && out.contains("-CREATE TABLE mine (id INTEGER);"), "{out}");
    assert!(err.contains("not written"));
    assert_eq!("CREATE TABLE mine (id INTEGER);\n", read(&dir, "migrations/0001_first.sql"));

    let (code, out, _) = cli(&["model", "Post", "title:String", "--json", "--dir", d]);
    assert_eq!(EXIT_OK, code);
    assert!(out.starts_with(r#"{"command":"rws gen model Post title:String","status":"written","written":true,"changes":[{"path":"src/models/post.rs","action":"create","diff":"--- /dev/null"#), "{out}");
}

#[test]
fn the_command_line_answers_usage_errors_with_exit_2() {
    let dir = api_project("cli_usage");
    let d = dir.as_str();
    for args in [
        vec!["model"],
        vec!["model", "Post", "title:u8", "--dir", d],
        vec!["route", "posts", "--db", "sqlite", "--dir", d],
        vec!["migration", "a", "b", "--dir", d],
        vec!["scaffold", "x"],
        vec!["test", "posts", "--nope"],
        vec!["route", "posts", "--methods"],
        vec![],
    ] {
        let (code, _, err) = cli(&args);
        assert_eq!(EXIT_USAGE, code, "{args:?}: {err}");
        assert!(err.starts_with("rws gen: "), "{args:?}: {err}");
    }
    let (code, out, _) = cli(&["model", "Post", "--json", "--dir", d]);
    assert_eq!((EXIT_USAGE, true), (code, out.starts_with(r#"{"status":"usage","error":"a model needs"#)), "{out}");
    let (code, _, err) = cli(&["test", "posts", "--dir", d]);
    assert_eq!((EXIT_CONFLICT, true), (code, err.contains("rws gen route posts")), "refused is 1: {err}");
    assert_eq!(Some(&["model".to_string()][..]), requested(&["rws".into(), "gen".into(), "model".into()]));
    assert_eq!(None, requested(&["rws".into(), "ctl".into()]));
}
