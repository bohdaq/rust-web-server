use std::collections::BTreeMap;

use super::*;
use crate::test_env::TempPath;

const POST: &str = r#"//! A post.
use rust_web_server::Model;

#[derive(Clone, Debug, Model)]
#[table(name = "posts")]
#[timestamps]
pub struct Post {
    #[primary_key(auto_increment)]
    pub id: i64,
    pub title: String,
    #[column(name = "view_count")]
    pub views: i64,
    pub published: bool,
    pub summary: Option<String>,
    pub cover: Option<rust_web_server::attachment::Attachment>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    #[ignore]
    pub cache: Vec<u8>,
    pub odd: HashMap<String, i64>,
}

/// Not a model.
pub struct Helper {
    pub x: i64,
}
"#;

const TAG: &str = "#[derive(Model)]\npub struct Tag {\n    #[primary_key]\n    pub slug: String,\n    pub label: String,\n}\n";

fn project(name: &str, cargo_features: &str) -> TempPath {
    let dir = TempPath::new(&format!("from_models_{name}"));
    std::fs::create_dir_all(dir.join("src/models")).unwrap();
    std::fs::create_dir_all(dir.join("migrations")).unwrap();
    std::fs::write(dir.join("Cargo.toml"), format!("[package]\nname = \"x\"\n\n[dependencies]\nrust-web-server = {{ version = \"18\", features = [{cargo_features}] }}\n")).unwrap();
    std::fs::write(dir.join("src/models/post.rs"), POST).unwrap();
    std::fs::write(dir.join("src/models/tag.rs"), TAG).unwrap();
    std::fs::write(dir.join("src/models/mod.rs"), "pub mod post;\npub mod tag;\n").unwrap();
    std::fs::write(dir.join("migrations/0001_create_notes.sql"), "CREATE TABLE notes (id INTEGER);").unwrap();
    dir
}

fn existing(tables: &[(&str, &[&str])]) -> ExistingColumns {
    tables.iter().map(|(t, cs)| (t.to_string(), cs.iter().map(|c| c.to_string()).collect())).collect()
}

#[test]
fn models_are_read_from_source() {
    let models = models_in(POST, "src/models/post.rs", Db::Postgres);
    assert_eq!(1, models.len(), "Helper isn't a model");
    let post = &models[0];
    assert_eq!(("Post", "posts"), (post.type_name.as_str(), post.table.as_str()));
    let names: Vec<&str> = post.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(vec!["id", "title", "view_count", "published", "summary", "cover", "created_at", "updated_at", "odd"], names);
    assert!(post.columns[0].auto_increment);
    assert_eq!(Some("TIMESTAMPTZ NOT NULL"), post.columns[6].sql.as_deref());
    assert_eq!(Some("TEXT"), post.columns[5].sql.as_deref());
    assert_eq!(None, post.columns[8].sql, "no SQL type for a HashMap");
    let tag = &models_in(TAG, "tag.rs", Db::Mysql)[0];
    assert_eq!(("tag", true, false), (tag.table.as_str(), tag.columns[0].primary_key, tag.columns[0].auto_increment));
}

#[test]
fn missing_tables_are_created() {
    let dir = project("create", "\"model-postgres\"");
    let plan = plan_model_migration(&*dir, "sync_models", &BTreeMap::new(), None).unwrap();
    let up = &plan.changes.iter().find(|c| c.path == "migrations/0002_sync_models.sql").unwrap().content;
    assert!(up.contains("CREATE TABLE posts (\n    id BIGSERIAL PRIMARY KEY,\n    title TEXT NOT NULL,\n    view_count BIGINT NOT NULL,"), "{up}");
    assert!(up.contains("CREATE TABLE tag (\n    slug TEXT NOT NULL PRIMARY KEY,\n    label TEXT NOT NULL\n);"), "{up}");
    assert!(!up.contains("cache"), "ignored fields have no column");
    let down = &plan.changes.iter().find(|c| c.path == "migrations/0002_sync_models.down.sql").unwrap().content;
    assert!(down.contains("DROP TABLE tag;\nDROP TABLE posts;"), "{down}");
    assert!(plan.notes.iter().any(|n| n.contains("posts.odd") && n.contains("HashMap")), "{:?}", plan.notes);
    // The same again changes nothing.
    plan.apply(&*dir).unwrap();
    let again = plan_model_migration(&*dir, "sync_models", &BTreeMap::new(), None).unwrap();
    assert!(again.is_unchanged());
}

#[test]
fn missing_columns_are_added_never_dropped() {
    let dir = project("alter", "\"model-sqlite\"");
    let db = existing(&[
        ("posts", &["id", "title", "VIEW_COUNT", "created_at", "updated_at", "legacy"]),
        ("tag", &["slug", "label"]),
        ("_schema_migrations", &["version"]),
    ]);
    let plan = plan_model_migration(&*dir, "add_post_fields", &db, None).unwrap();
    let up = &plan.changes.iter().find(|c| c.path.ends_with("_add_post_fields.sql")).unwrap().content;
    assert!(up.contains("ALTER TABLE posts ADD COLUMN published INTEGER NOT NULL DEFAULT 0;"), "{up}");
    assert!(up.contains("ALTER TABLE posts ADD COLUMN summary TEXT;"), "{up}");
    assert!(up.contains("ALTER TABLE posts ADD COLUMN cover TEXT;"), "{up}");
    assert!(!up.contains("CREATE TABLE") && !up.contains("legacy") && !up.contains("DROP"), "{up}");
    let down = &plan.changes.iter().find(|c| c.path.ends_with("_add_post_fields.down.sql")).unwrap().content;
    assert!(down.contains("ALTER TABLE posts DROP COLUMN cover;\nALTER TABLE posts DROP COLUMN summary;\nALTER TABLE posts DROP COLUMN published;"), "{down}");
    assert!(plan.notes.iter().any(|n| n.contains("posts.legacy") && n.contains("by hand")), "{:?}", plan.notes);
}

#[test]
fn nothing_to_do_when_the_database_matches() {
    let dir = project("same", "\"model-mysql\"");
    let db = existing(&[
        ("posts", &["id", "title", "view_count", "published", "summary", "cover", "created_at", "updated_at", "odd"]),
        ("tag", &["slug", "label"]),
    ]);
    let plan = plan_model_migration(&*dir, "noop", &db, None).unwrap();
    assert!(plan.changes.is_empty());
    assert!(plan.notes.iter().any(|n| n.contains("no migration")));
}

#[test]
fn refusals() {
    let dir = project("refusals", "");
    assert!(plan_model_migration(&*dir, "Bad Name", &BTreeMap::new(), None).is_err());
    std::fs::remove_file(dir.join("src/models/post.rs")).unwrap();
    std::fs::remove_file(dir.join("src/models/tag.rs")).unwrap();
    let err = plan_model_migration(&*dir, "empty", &BTreeMap::new(), None).unwrap_err();
    assert!(err.message.contains("no #[derive(Model)]"), "{err}");
    assert!(plan_model_migration("/no/such/project", "x", &BTreeMap::new(), None).is_err());
}

#[test]
fn types_map_per_database() {
    assert_eq!((Some("BOOLEAN NOT NULL".to_string()), false), sql_for("bool", Db::Postgres));
    assert_eq!((Some("DATETIME(6)".to_string()), true), sql_for("Option<Timestamp>", Db::Mysql));
    assert_eq!((Some("UUID NOT NULL".to_string()), false), sql_for("rust_web_server::model::Uuid", Db::Postgres));
    assert_eq!((None, false), sql_for("Vec<u8>", Db::Sqlite));
    assert_eq!(None, default_for("Timestamp", Db::Sqlite));
    assert_eq!(Some("FALSE"), default_for("bool", Db::Postgres));
}

/// The command line against a real SQLite database: what it lacks becomes
/// the migration, and after applying it there's nothing left to do.
#[cfg(feature = "model-sqlite")]
#[test]
fn the_command_reads_the_database() {
    let dir = project("cli", "\"model-sqlite\"");
    let db_file = dir.join("app.db");
    let url = format!("sqlite:{}?mode=rwc", db_file.display());
    let args = |name: &str, extra: &[&str]| -> Vec<String> {
        let mut a: Vec<String> = ["migration", name, "--from-models", "--database-url", &url, "--dir", dir.to_str().unwrap()].iter().map(|s| s.to_string()).collect();
        a.extend(extra.iter().map(|s| s.to_string()));
        a
    };
    // The database has `posts` with two columns, no `tag`.
    crate::dev_mcp::db::run({
        let url = url.clone();
        async move {
            let pool = crate::model::DbPool::connect(&url).await?;
            pool.execute("CREATE TABLE posts (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL)", &[]).await?;
            pool.close().await;
            Ok(())
        }
    })
    .unwrap();

    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(super::super::EXIT_OK, super::super::run(&args("sync", &["--json"]), &mut out, &mut err), "{}", String::from_utf8_lossy(&err));
    let up = std::fs::read_to_string(dir.join("migrations/0002_sync.sql")).unwrap();
    assert!(up.contains("CREATE TABLE tag") && up.contains("ALTER TABLE posts ADD COLUMN published INTEGER NOT NULL DEFAULT 0;"), "{up}");

    // Applied, the database matches: nothing more.
    crate::dev_mcp::db::run({
        let (url, migrations) = (url.clone(), dir.join("migrations").display().to_string());
        async move {
            let pool = crate::model::DbPool::connect(&url).await?;
            // 0001 created `notes` outside this test's models; skip it.
            pool.execute("CREATE TABLE IF NOT EXISTS _schema_migrations (version TEXT PRIMARY KEY, applied_at TEXT)", &[]).await?;
            pool.execute("INSERT INTO _schema_migrations (version, applied_at) VALUES ('0001_create_notes', '')", &[]).await?;
            pool.migrate(&migrations).await?;
            pool.close().await;
            Ok(())
        }
    })
    .unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(super::super::EXIT_OK, super::super::run(&args("sync_again", &[]), &mut out, &mut err));
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("no migration"), "{text}");
    assert!(!dir.join("migrations/0003_sync_again.sql").exists());

    // Usage: only for migrations; the URL only with --from-models.
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(super::super::EXIT_USAGE, super::super::run(&["model".into(), "X".into(), "a:i64".into(), "--from-models".into()], &mut out, &mut err));
    assert_eq!(super::super::EXIT_USAGE, super::super::run(&["migration".into(), "x".into(), "--database-url".into(), "sqlite::memory:".into()], &mut out, &mut err));
}
