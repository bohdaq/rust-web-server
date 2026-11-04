//! Async integration tests for the model layer, on every backend the build
//! and environment have (see `backends.rs`): SQLite in memory, and
//! PostgreSQL / MySQL when `RWS_TEST_POSTGRES_URL` / `RWS_TEST_MYSQL_URL`
//! are set.
//!
//! Run with:
//! ```bash
//! cargo test -p rws-server --features http2,test-support,macros,model-sqlite -- model
//! RWS_TEST_POSTGRES_URL=postgres://… RWS_TEST_MYSQL_URL=mysql://… \
//!   cargo test -p rws-server --features http2,test-support,macros,model-sqlite,model-postgres,model-mysql -- model
//! ```

#[cfg(all(test, any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql")))]
mod model_tests {
    use crate::model::backends::{self, TestBackend};
    #[allow(unused_imports)]
    use crate::model::{
        Backend, CursorPage, DbConfig, DbPool, HasMany, Model, ModelRow, Order, Page, QueryBuilder,
        Repository, Value,
    };
    use crate::model::repository::ModelRepository;
    use crate::test_env::TempPath;

    // ── Test helpers ──────────────────────────────────────────────────────────

    const SCHEMA: &str = "CREATE TABLE users (id {pk}, name {text} NOT NULL, email {text} NOT NULL UNIQUE, age INTEGER){engine};\
                          CREATE TABLE posts (id {pk}, title {text} NOT NULL, user_id BIGINT NOT NULL){engine}";

    /// Every backend, each with the `users` and `posts` tables.
    async fn test_dbs() -> Vec<TestBackend> {
        let mut out = Vec::new();
        for db in backends::all().await {
            db.create(SCHEMA).await;
            out.push(db);
        }
        out
    }

    /// `sql` with `?` placeholders numbered for PostgreSQL.
    fn q(db: &TestBackend, sql: &str) -> String {
        match db.backend() {
            #[cfg(feature = "model-postgres")]
            Backend::Postgres => {
                let mut out = String::new();
                let mut n = 0;
                for c in sql.chars() {
                    if c == '?' {
                        n += 1;
                        out.push_str(&format!("${n}"));
                    } else {
                        out.push(c);
                    }
                }
                out
            }
            #[allow(unreachable_patterns)]
            _ => sql.to_string(),
        }
    }

    // ── Minimal Model impls (no proc-macro needed) ────────────────────────────

    #[derive(Debug, Clone, PartialEq)]
    struct User {
        id: i64,
        name: String,
        email: String,
        age: Option<i32>,
    }

    impl Model for User {
        fn table_name() -> &'static str { "users" }
        fn column_names() -> &'static [&'static str] { &["id", "name", "email", "age"] }
        fn primary_key_name() -> &'static str { "id" }
        fn primary_key_value(&self) -> Value { Value::Int(self.id) }
        fn primary_key_auto_increment() -> bool { true }
        fn from_row(row: &ModelRow) -> Result<Self, crate::model::DbError> {
            Ok(User {
                id: row.get("id")?,
                name: row.get("name")?,
                email: row.get("email")?,
                age: row.get("age")?,
            })
        }
        fn to_values(&self) -> Vec<(&'static str, Value)> {
            vec![
                ("id", Value::Int(self.id)),
                ("name", Value::Text(self.name.clone())),
                ("email", Value::Text(self.email.clone())),
                ("age", self.age.map(|a| Value::Int(a as i64)).unwrap_or(Value::Null)),
            ]
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct Post {
        id: i64,
        title: String,
        user_id: i64,
    }

    impl Model for Post {
        fn table_name() -> &'static str { "posts" }
        fn column_names() -> &'static [&'static str] { &["id", "title", "user_id"] }
        fn primary_key_name() -> &'static str { "id" }
        fn primary_key_value(&self) -> Value { Value::Int(self.id) }
        fn primary_key_auto_increment() -> bool { true }
        fn from_row(row: &ModelRow) -> Result<Self, crate::model::DbError> {
            Ok(Post {
                id: row.get("id")?,
                title: row.get("title")?,
                user_id: row.get("user_id")?,
            })
        }
        fn to_values(&self) -> Vec<(&'static str, Value)> {
            vec![
                ("id", Value::Int(self.id)),
                ("title", Value::Text(self.title.clone())),
                ("user_id", Value::Int(self.user_id)),
            ]
        }
    }

    fn new_user(name: &str, email: &str, age: Option<i32>) -> User {
        User { id: 0, name: name.to_owned(), email: email.to_owned(), age }
    }

    async fn insert_user(db: &TestBackend, name: &str, email: &str, age: i64) {
        db.pool
            .execute(&q(db, "INSERT INTO users (name, email, age) VALUES (?, ?, ?)"), &[Value::Text(name.into()), Value::Text(email.into()), Value::Int(age)])
            .await
            .unwrap_or_else(|e| panic!("[{}] insert {name}: {e}", db.name));
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    #[test]
    fn model_info_reads_the_mapping() {
        let info = crate::model::ModelInfo::of::<User>();
        assert_eq!(info.table, "users");
        assert_eq!(info.columns, &["id", "name", "email", "age"]);
        assert_eq!((info.primary_key, info.auto_increment), ("id", true));
        assert!(info.type_name.ends_with("::User"), "{}", info.type_name);
        assert_eq!(info.short_name(), "User");
        let generic = crate::model::ModelInfo { type_name: "app::Page<app::models::Post>", ..info.clone() };
        assert_eq!(generic.short_name(), "Page");
        assert_ne!(info, crate::model::ModelInfo::of::<Post>());
    }

    #[cfg(feature = "model-sqlite")]
    #[tokio::test]
    async fn test_01_create_table_via_raw_sql() {
        let pool = DbPool::memory().await.unwrap();
        pool.execute("CREATE TABLE users (id INTEGER PRIMARY KEY)", &[]).await.unwrap();
        let rows = pool
            .query_rows("SELECT name FROM sqlite_master WHERE type='table' AND name='users'", &[])
            .await
            .expect("query sqlite_master");
        assert!(!rows.is_empty(), "users table should exist");
    }

    #[tokio::test]
    async fn test_02_save_inserts_new_row_with_pk() {
        for db in test_dbs().await {
            let repo: ModelRepository<User, i64> = ModelRepository::new(&db.pool);
            let saved = repo.save(&new_user("Alice", "alice@test.com", Some(30))).await.expect("save alice");
            assert!(saved.id > 0, "[{}] saved user should have a generated PK", db.name);
            assert_eq!(saved.name, "Alice");
            assert_eq!(saved.email, "alice@test.com");
            assert_eq!(saved.age, Some(30), "[{}]", db.name);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_03_find_by_id_returns_correct_row() {
        for db in test_dbs().await {
            let repo: ModelRepository<User, i64> = ModelRepository::new(&db.pool);
            let saved = repo.save(&new_user("Bob", "bob@test.com", None)).await.expect("save bob");
            let found = repo.find_by_id(saved.id).await.expect("find by id").expect("found");
            assert_eq!(found.name, "Bob", "[{}]", db.name);
            assert_eq!(found.age, None);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_04_find_all_returns_all_rows() {
        for db in test_dbs().await {
            let repo: ModelRepository<User, i64> = ModelRepository::new(&db.pool);
            repo.save(&new_user("Carol", "carol@test.com", Some(25))).await.unwrap();
            repo.save(&new_user("Dave", "dave@test.com", Some(35))).await.unwrap();
            assert_eq!(2, repo.find_all().await.expect("find all").len(), "[{}]", db.name);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_05_save_on_existing_entity_updates() {
        for db in test_dbs().await {
            let repo: ModelRepository<User, i64> = ModelRepository::new(&db.pool);
            let saved = repo.save(&new_user("Eve", "eve@test.com", Some(20))).await.unwrap();
            let mut updated = saved.clone();
            updated.name = "Eve Updated".to_owned();
            updated.age = Some(21);
            let result = repo.save(&updated).await.expect("update eve");
            assert_eq!(result.id, saved.id, "[{}]", db.name);
            assert_eq!(result.name, "Eve Updated");
            assert_eq!(result.age, Some(21));
            assert_eq!(1, repo.count().await.unwrap(), "an update, not an insert");
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_05b_save_of_a_key_that_is_gone_is_not_found() {
        for db in test_dbs().await {
            let repo: ModelRepository<User, i64> = ModelRepository::new(&db.pool);
            let ghost = User { id: 4242, ..new_user("Ghost", "ghost@test.com", None) };
            let err = repo.save(&ghost).await.expect_err("no row 4242 to update");
            assert!(err.is_not_found(), "[{}] {err}", db.name);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_06_delete_by_id_removes_row() {
        for db in test_dbs().await {
            let repo: ModelRepository<User, i64> = ModelRepository::new(&db.pool);
            let saved = repo.save(&new_user("Frank", "frank@test.com", None)).await.unwrap();
            repo.delete_by_id(saved.id).await.expect("delete frank");
            assert!(repo.find_by_id(saved.id).await.expect("find after delete").is_none(), "[{}]", db.name);
            assert_eq!(0, db.pool.query_rows("SELECT * FROM users", &[]).await.unwrap().len(), "a real DELETE");
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_07_count_returns_correct_count() {
        for db in test_dbs().await {
            let repo: ModelRepository<User, i64> = ModelRepository::new(&db.pool);
            let before = repo.count().await.expect("count before");
            repo.save(&new_user("Grace", "grace@test.com", Some(28))).await.unwrap();
            assert_eq!(repo.count().await.expect("count after"), before + 1, "[{}]", db.name);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_08_exists_by_id_returns_correct_boolean() {
        for db in test_dbs().await {
            let repo: ModelRepository<User, i64> = ModelRepository::new(&db.pool);
            let saved = repo.save(&new_user("Hank", "hank@test.com", None)).await.unwrap();
            assert!(repo.exists_by_id(saved.id).await.expect("exists true"), "[{}]", db.name);
            assert!(!repo.exists_by_id(99999).await.expect("exists false"));
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_09_query_builder_where_order_limit_offset() {
        for db in test_dbs().await {
            let pool = &db.pool;
            insert_user(&db, "Q1", "q1@t.com", 25).await;
            insert_user(&db, "Q2", "q2@t.com", 30).await;
            insert_user(&db, "Q3", "q3@t.com", 35).await;

            let users: Vec<User> = QueryBuilder::new(pool).where_eq("name", "Q1").fetch_all().await.expect("where_eq Q1");
            assert_eq!(1, users.len(), "[{}]", db.name);
            assert_eq!(users[0].name, "Q1");

            let users: Vec<User> = QueryBuilder::new(pool).filter("age >= ?", vec![Value::Int(30)]).fetch_all().await.expect("filter");
            assert_eq!(2, users.len());
            assert!(users.iter().all(|u| u.age.unwrap_or(0) >= 30));

            let users: Vec<User> = QueryBuilder::new(pool)
                .filter("name LIKE ?", vec![Value::Text("Q%".into())])
                .order_by("age", Order::Desc)
                .limit(2)
                .fetch_all()
                .await
                .expect("order limit");
            assert_eq!(vec!["Q3", "Q2"], users.iter().map(|u| u.name.as_str()).collect::<Vec<_>>(), "[{}]", db.name);

            let users: Vec<User> = QueryBuilder::new(pool).order_by("age", Order::Asc).limit(1).offset(1).fetch_all().await.expect("offset");
            assert_eq!("Q2", users[0].name);

            let n = QueryBuilder::<User>::new(pool).filter("name LIKE ?", vec![Value::Text("Q%".into())]).count().await.expect("count");
            assert_eq!(3, n);

            let one: Option<User> = QueryBuilder::new(pool).where_eq("name", "Q2").fetch_one().await.expect("fetch_one Q2");
            assert_eq!(one.expect("Q2").name, "Q2");

            QueryBuilder::<User>::new(pool).where_eq("name", "Q2").update("age", 31i64).await.expect("update");
            let q2: User = QueryBuilder::new(pool).where_eq("name", "Q2").fetch_one().await.unwrap().unwrap();
            assert_eq!(Some(31), q2.age);

            QueryBuilder::<User>::new(pool).where_eq("name", "Q3").delete().await.expect("delete Q3");
            let check: Option<User> = QueryBuilder::new(pool).where_eq("name", "Q3").fetch_one().await.expect("check");
            assert!(check.is_none(), "[{}] Q3 should be deleted", db.name);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_10_raw_sql_query_and_execute() {
        for db in test_dbs().await {
            let pool = &db.pool;
            insert_user(&db, "Raw", "raw@test.com", 42).await;

            let users: Vec<User> = pool.query(&q(&db, "SELECT * FROM users WHERE name = ?"), &[Value::Text("Raw".into())]).await.expect("typed query");
            assert_eq!(users.len(), 1, "[{}]", db.name);
            assert_eq!(users[0].name, "Raw");

            let rows = pool.query_raw(&q(&db, "SELECT name, age FROM users WHERE name = ?"), &[Value::Text("Raw".into())]).await.expect("raw query");
            assert_eq!(rows.len(), 1);
            let name: String = rows[0].get("name").unwrap();
            assert_eq!(name, "Raw");
            let age: i64 = rows[0].get("age").unwrap();
            assert_eq!(age, 42, "[{}] an INTEGER column reads as a number", db.name);

            let affected = pool
                .execute(&q(&db, "UPDATE users SET age = ? WHERE name = ?"), &[Value::Int(99), Value::Text("Raw".into())])
                .await
                .expect("raw execute update");
            assert_eq!(affected, 1);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_11_transaction_commits_on_ok() {
        for db in test_dbs().await {
            let sql = q(&db, "INSERT INTO users (name, email, age) VALUES (?, ?, ?)");
            let result = db
                .pool
                .transaction(|mut tx| async move {
                    tx.execute(&sql, &[Value::Text("Tx1".into()), Value::Text("tx1@test.com".into()), Value::Int(10)]).await?;
                    tx.commit().await?;
                    Ok(42i32)
                })
                .await;
            assert_eq!(result.expect("transaction should commit"), 42);
            let rows = db.pool.query_rows(&q(&db, "SELECT * FROM users WHERE name = ?"), &[Value::Text("Tx1".into())]).await.expect("find tx1");
            assert_eq!(rows.len(), 1, "[{}] committed row should be visible", db.name);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_11b_transaction_rolls_back_on_err() {
        for db in test_dbs().await {
            let sql = q(&db, "INSERT INTO users (name, email, age) VALUES (?, ?, ?)");
            let result: Result<(), _> = db
                .pool
                .transaction(|mut tx| async move {
                    tx.execute(&sql, &[Value::Text("TxRB".into()), Value::Text("txrb@test.com".into()), Value::Int(5)]).await?;
                    Err(crate::model::DbError::new("intentional rollback"))
                })
                .await;
            assert!(result.is_err());
            let rows = db.pool.query_rows(&q(&db, "SELECT * FROM users WHERE name = ?"), &[Value::Text("TxRB".into())]).await.expect("find txrb");
            assert!(rows.is_empty(), "[{}] rolled-back row should not be visible", db.name);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_12_has_many_load_returns_related_records() {
        for db in test_dbs().await {
            let users: ModelRepository<User, i64> = ModelRepository::new(&db.pool);
            let posts: ModelRepository<Post, i64> = ModelRepository::new(&db.pool);
            let user = users.save(&new_user("HMUser", "hmu@test.com", Some(20))).await.unwrap();
            for title in ["Post A", "Post B"] {
                posts.save(&Post { id: 0, title: title.into(), user_id: user.id }).await.unwrap();
            }
            posts.save(&Post { id: 0, title: "Someone else's".into(), user_id: user.id + 1 }).await.unwrap();

            let loaded = HasMany::<Post>::new(Value::Int(user.id), "user_id").load(&db.pool).await.expect("load posts");
            let mut titles: Vec<&str> = loaded.iter().map(|p| p.title.as_str()).collect();
            titles.sort();
            assert_eq!(vec!["Post A", "Post B"], titles, "[{}]", db.name);
            let owner = crate::model::BelongsTo::<User>::new(Value::Int(loaded[0].user_id)).load(&db.pool).await.unwrap();
            assert_eq!(Some("HMUser".to_string()), owner.map(|u| u.name));
            db.finish().await;
        }
    }

    // ── Migrations ────────────────────────────────────────────────────────────

    fn write(dir: &TempPath, name: &str, sql: &str) {
        std::fs::write(dir.join(name), sql).unwrap();
    }

    fn migrations_dir(name: &str) -> TempPath {
        let dir = TempPath::new(name);
        std::fs::create_dir_all(&*dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn test_13_migration_runner_creates_table_and_tracks_versions() {
        for db in backends::all().await {
            let dir = migrations_dir(&format!("model_migrations_13_{}", db.name));
            write(&dir, "0001_create_items.sql", &db.ddl("CREATE TABLE items (id {pk}, name {text}){engine}"));
            write(&dir, "0002_add_col.sql", "ALTER TABLE items ADD COLUMN qty INTEGER DEFAULT 0");

            db.pool.migrate(dir.as_str()).await.unwrap_or_else(|e| panic!("[{}] run migrations: {e}", db.name));
            let rows = db.pool.query_rows("SELECT version FROM _schema_migrations ORDER BY version", &[]).await.expect("query migrations");
            let versions: Vec<String> = rows.iter().map(|r| r.get("version").unwrap()).collect();
            assert_eq!(vec!["0001_create_items.sql", "0002_add_col.sql"], versions, "[{}]", db.name);

            // idempotent
            db.pool.migrate(dir.as_str()).await.expect("re-run migrations idempotent");
            let rows2 = db.pool.query_rows("SELECT version FROM _schema_migrations ORDER BY version", &[]).await.unwrap();
            assert_eq!(rows2.len(), 2);

            let status = db.pool.migration_status(dir.as_str()).await.expect("migration status");
            assert_eq!(status.len(), 2);
            assert!(status.iter().all(|s| s.applied));
            db.pool.execute("INSERT INTO items (name) VALUES ('a')", &[]).await.expect("the migrated table");
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_13b_a_failing_migration_is_not_recorded() {
        for db in backends::all().await {
            let dir = migrations_dir(&format!("model_migrations_13b_{}", db.name));
            write(&dir, "0001_ok.sql", &db.ddl("CREATE TABLE fine (id {pk}){engine}"));
            write(&dir, "0002_broken.sql", "CREATE TABLE (oops");
            let err = db.pool.migrate(dir.as_str()).await.expect_err("a broken migration");
            assert!(err.to_string().contains("0002_broken.sql"), "[{}] {err}", db.name);
            let status = db.pool.migration_status(dir.as_str()).await.unwrap();
            assert_eq!(vec![true, false], status.iter().map(|s| s.applied).collect::<Vec<_>>(), "[{}]", db.name);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_14_rollback_last_undoes_most_recent_migration() {
        for db in backends::all().await {
            let dir = migrations_dir(&format!("model_migrations_14_{}", db.name));
            write(&dir, "0001_create_items.sql", &db.ddl("CREATE TABLE items (id {pk}, name {text}){engine}"));
            write(&dir, "0001_create_items.down.sql", "DROP TABLE items");
            write(&dir, "0002_create_tags.sql", &db.ddl("CREATE TABLE tags (id {pk}, label {text}){engine}"));
            write(&dir, "0002_create_tags.down.sql", "DROP TABLE tags");

            db.pool.migrate(dir.as_str()).await.expect("run migrations");
            db.pool.execute("INSERT INTO tags (label) VALUES ('x')", &[]).await.expect("insert into tags");

            let rolled_back = db.pool.rollback_last(dir.as_str()).await.expect("rollback last");
            assert_eq!(Some("0002_create_tags.sql".to_string()), rolled_back, "[{}]", db.name);

            // `tags` should be gone; `items` (from the still-applied migration) remains.
            assert!(db.pool.execute("SELECT 1 FROM tags", &[]).await.is_err(), "[{}]", db.name);
            db.pool.execute("INSERT INTO items (name) VALUES ('a')", &[]).await.expect("items table still exists");

            let rows = db.pool.query_rows("SELECT version FROM _schema_migrations ORDER BY version", &[]).await.unwrap();
            let versions: Vec<String> = rows.iter().map(|r| r.get("version").unwrap()).collect();
            assert_eq!(versions, vec!["0001_create_items.sql".to_string()]);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_15_rollback_n_undoes_multiple_in_reverse_order() {
        for db in backends::all().await {
            let dir = migrations_dir(&format!("model_migrations_15_{}", db.name));
            for name in ["a", "b", "c"] {
                let n = match name { "a" => 1, "b" => 2, _ => 3 };
                write(&dir, &format!("000{n}_create_{name}.sql"), &format!("CREATE TABLE {name} (id INTEGER PRIMARY KEY)"));
                write(&dir, &format!("000{n}_create_{name}.down.sql"), &format!("DROP TABLE {name}"));
            }

            db.pool.migrate(dir.as_str()).await.expect("run migrations");
            let rolled_back = db.pool.rollback(dir.as_str(), 2).await.expect("rollback 2");
            assert_eq!(vec!["0003_create_c.sql".to_string(), "0002_create_b.sql".to_string()], rolled_back, "[{}]", db.name);

            let status = db.pool.migration_status(dir.as_str()).await.expect("status");
            let applied: Vec<&str> = status.iter().filter(|s| s.applied).map(|s| s.version.as_str()).collect();
            assert_eq!(vec!["0001_create_a.sql"], applied);

            // Asking for more than what's applied stops early instead of erroring.
            let rolled_back_rest = db.pool.rollback(dir.as_str(), 5).await.expect("rollback rest");
            assert_eq!(vec!["0001_create_a.sql".to_string()], rolled_back_rest);
            assert_eq!(0, db.pool.rollback(dir.as_str(), 1).await.expect("nothing left").len());
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_16_rollback_last_is_none_when_nothing_applied() {
        for db in backends::all().await {
            let dir = migrations_dir(&format!("model_migrations_16_{}", db.name));
            assert_eq!(None, db.pool.rollback_last(dir.as_str()).await.expect("rollback with nothing applied"), "[{}]", db.name);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_17_rollback_last_errors_when_down_file_missing() {
        for db in backends::all().await {
            let dir = migrations_dir(&format!("model_migrations_17_{}", db.name));
            write(&dir, "0001_create_items.sql", "CREATE TABLE items (id INTEGER PRIMARY KEY)");
            // No 0001_create_items.down.sql on purpose.
            db.pool.migrate(dir.as_str()).await.expect("run migrations");

            let err = db.pool.rollback_last(dir.as_str()).await.expect_err("missing down file should error");
            assert!(err.to_string().contains("0001_create_items.down.sql"), "[{}] {err}", db.name);

            // Still applied: a failed rollback attempt must not mark it undone.
            let status = db.pool.migration_status(dir.as_str()).await.expect("status");
            assert!(status[0].applied);
            assert!(!status[0].has_down);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn seeds_run_in_order_every_time_and_stop_at_a_failure() {
        for db in test_dbs().await {
            let dir = migrations_dir(&format!("model_seeds_{}", db.name));
            write(&dir, "01_users.sql", "INSERT INTO users (name, email, age) VALUES ('Ann', 'ann@example.com', 30)");
            write(&dir, "02_more.sql", "INSERT INTO users (name, email, age) VALUES ('Bo', 'bo@example.com', 40); UPDATE users SET age = 31 WHERE name = 'Ann'");
            let ran = db.pool.seed(dir.as_str()).await.unwrap_or_else(|e| panic!("[{}] seed: {e}", db.name));
            assert_eq!(vec!["01_users.sql", "02_more.sql"], ran);
            let repo: ModelRepository<User, i64> = ModelRepository::new(&db.pool);
            assert_eq!(2, repo.count().await.unwrap());
            let ann: User = QueryBuilder::new(&db.pool).where_eq("name", "Ann").fetch_one().await.unwrap().unwrap();
            assert_eq!(Some(31), ann.age, "[{}] the second file ran after the first", db.name);

            // Seeds aren't recorded: a second run inserts again — and here
            // fails on the unique email, rolling that file back.
            let err = db.pool.seed(dir.as_str()).await.expect_err("duplicate email");
            assert!(err.to_string().contains("01_users.sql"), "[{}] {err}", db.name);
            assert_eq!(2, repo.count().await.unwrap());
            assert!(db.pool.seed("/no/such/seeds").await.unwrap_err().to_string().contains("seeds directory"));
            db.finish().await;
        }
    }

    // ── SQLite pool specifics ────────────────────────────────────────────────

    #[cfg(feature = "model-sqlite")]
    #[tokio::test]
    async fn db_pool_memory_single_connection_sees_correct_data() {
        let pool = DbPool::memory().await.expect("create memory pool");
        pool.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)", &[]).await.unwrap();
        pool.execute("INSERT INTO items (name) VALUES (?)", &[Value::Text("apple".into())]).await.unwrap();

        let rows = pool.query_rows("SELECT name FROM items", &[]).await.unwrap();
        assert_eq!(1, rows.len());
        let name: String = rows[0].get("name").unwrap();
        assert_eq!("apple", name);
    }

    #[cfg(feature = "model-sqlite")]
    #[tokio::test]
    async fn db_pool_memory_is_isolated_per_call() {
        let pool_a = DbPool::memory().await.unwrap();
        let pool_b = DbPool::memory().await.unwrap();
        pool_a.execute("CREATE TABLE t (x INTEGER)", &[]).await.unwrap();
        pool_a.execute("INSERT INTO t VALUES (42)", &[]).await.unwrap();
        pool_b.execute("CREATE TABLE t (x INTEGER)", &[]).await.unwrap();
        let rows = pool_b.query_rows("SELECT x FROM t", &[]).await.unwrap();
        assert!(rows.is_empty(), "pool_b should be a separate empty database");
    }

    #[cfg(feature = "model-sqlite")]
    #[tokio::test]
    async fn sqlite_columns_of_other_declared_types_read_as_stored() {
        let pool = DbPool::memory().await.unwrap();
        pool.execute("CREATE TABLE t (at DATETIME, d DATE, n DATETIME)", &[]).await.unwrap();
        pool.execute("INSERT INTO t VALUES ('2026-11-04T12:00:00Z', '2026-11-04', 1700000000)", &[]).await.unwrap();
        let rows = pool.query_rows("SELECT * FROM t", &[]).await.unwrap();
        assert_eq!(Value::Text("2026-11-04T12:00:00Z".into()), rows[0].get::<Value>("at").unwrap());
        assert_eq!(Value::Text("2026-11-04".into()), rows[0].get::<Value>("d").unwrap());
        assert_eq!(Value::Int(1_700_000_000), rows[0].get::<Value>("n").unwrap());
    }

    #[tokio::test]
    async fn connect_takes_a_database_url() {
        #[cfg(feature = "model-sqlite")]
        {
            let pool = DbPool::connect("sqlite::memory:").await.expect("sqlite URL");
            assert_eq!(Backend::Sqlite, pool.backend());
            pool.execute("CREATE TABLE t (x INTEGER)", &[]).await.unwrap();
        }
        let err = DbPool::connect("oracle://scott:tiger@db/x").await.expect_err("unknown scheme");
        assert!(err.to_string().contains("expected sqlite:"), "{err}");
        assert!(!err.to_string().contains("tiger"), "the password isn't repeated: {err}");
        assert_eq!("postgres://app:***@db:5432/x?sslmode=require", crate::model::pool::redact_url("postgres://app:s3cr%40t@db:5432/x?sslmode=require"));
        assert_eq!("sqlite:app.db", crate::model::pool::redact_url("sqlite:app.db"));
    }

    // ── Backend selection ─────────────────────────────────────────────────────

    #[cfg(feature = "model-sqlite")]
    #[tokio::test]
    async fn test_18_pool_backend_reports_sqlite_for_memory_pool() {
        let pool = DbPool::memory().await.unwrap();
        assert_eq!(Backend::Sqlite, pool.backend());
    }

    #[cfg(feature = "model-sqlite")]
    #[test]
    fn test_19_backend_parse_accepts_compiled_in_name_case_insensitively() {
        assert_eq!(Backend::Sqlite, Backend::parse("sqlite").unwrap());
        assert_eq!(Backend::Sqlite, Backend::parse("SQLite").unwrap());
    }

    #[cfg(all(feature = "model-sqlite", not(feature = "model-postgres")))]
    #[test]
    fn test_20_backend_parse_rejects_unknown_or_not_compiled_in_name() {
        let err = Backend::parse("postgres").expect_err("model-postgres isn't compiled into this test binary");
        assert!(err.to_string().contains("postgres"));
        assert!(err.to_string().contains("sqlite"), "error should list what IS compiled in");
    }

    #[test]
    fn test_20b_backend_parse_rejects_an_unknown_name() {
        let err = Backend::parse("oracle").expect_err("not a backend");
        assert!(err.to_string().contains("oracle"), "{err}");
    }

    // test_21/22/23 all read and write RWS_DB_BACKEND/RWS_DB_NAME — hold the
    // shared env-var lock (see CLAUDE.md) so they can't interleave with each
    // other under `cargo test`'s default parallelism.

    #[cfg(all(feature = "model-sqlite", not(feature = "model-postgres"), not(feature = "model-mysql")))]
    #[test]
    fn test_21_dbconfig_from_env_infers_sqlite_when_unambiguous() {
        let _g = crate::test_env::lock();
        std::env::remove_var("RWS_DB_BACKEND");
        std::env::set_var("RWS_DB_NAME", ":memory:");

        let config = DbConfig::from_env().expect("exactly one model-* feature is compiled in");
        assert_eq!(Backend::Sqlite, config.backend);

        std::env::remove_var("RWS_DB_NAME");
    }

    #[cfg(feature = "model-sqlite")]
    #[test]
    fn test_22_dbconfig_from_env_respects_explicit_backend_override() {
        let _g = crate::test_env::lock();
        std::env::set_var("RWS_DB_BACKEND", "sqlite");
        std::env::set_var("RWS_DB_NAME", ":memory:");

        let config = DbConfig::from_env().expect("explicit sqlite backend is compiled in");
        assert_eq!(Backend::Sqlite, config.backend);

        std::env::remove_var("RWS_DB_BACKEND");
        std::env::remove_var("RWS_DB_NAME");
    }

    #[cfg(all(feature = "model-sqlite", not(feature = "model-mysql")))]
    #[test]
    fn test_23_dbconfig_from_env_errors_on_not_compiled_in_backend() {
        let _g = crate::test_env::lock();
        std::env::set_var("RWS_DB_BACKEND", "mysql");
        std::env::set_var("RWS_DB_NAME", ":memory:");

        let err = DbConfig::from_env().expect_err("model-mysql isn't compiled into this test binary");
        assert!(err.to_string().contains("mysql"));

        std::env::remove_var("RWS_DB_BACKEND");
        std::env::remove_var("RWS_DB_NAME");
    }

    // ── Pagination ────────────────────────────────────────────────────────────

    async fn seed_users(db: &TestBackend, count: i64, age: impl Fn(i64) -> i32) {
        for i in 1..=count {
            insert_user(db, &format!("User{}", i), &format!("u{}@t.com", i), age(i) as i64).await;
        }
    }

    #[tokio::test]
    async fn test_14_query_builder_paginate_returns_pages_in_order() {
        for db in test_dbs().await {
            let pool = &db.pool;
            seed_users(&db, 25, |_| 20).await;

            let page1: Page<User> = QueryBuilder::new(pool).order_by("id", Order::Asc).paginate(1, 10).await.expect("paginate page 1");
            assert_eq!(1, page1.page);
            assert_eq!(10, page1.per_page);
            assert_eq!(25, page1.total_items, "[{}]", db.name);
            assert_eq!(3, page1.total_pages);
            assert_eq!(10, page1.items.len());
            assert_eq!("User1", page1.items[0].name);
            assert_eq!("User10", page1.items[9].name);
            assert!(page1.has_next());
            assert!(!page1.has_prev());

            let page2: Page<User> = QueryBuilder::new(pool).order_by("id", Order::Asc).paginate(2, 10).await.expect("paginate page 2");
            assert_eq!(10, page2.items.len());
            assert_eq!("User11", page2.items[0].name);
            assert!(page2.has_next());
            assert!(page2.has_prev());

            let page3: Page<User> = QueryBuilder::new(pool).order_by("id", Order::Asc).paginate(3, 10).await.expect("paginate page 3");
            assert_eq!(5, page3.items.len(), "last page should have the 5 remaining rows");
            assert_eq!("User25", page3.items[4].name);
            assert!(!page3.has_next());
            assert!(page3.has_prev());
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_14b_query_builder_paginate_respects_filters_in_both_queries() {
        for db in test_dbs().await {
            seed_users(&db, 10, |i| if i <= 5 { 20 } else { 30 }).await;
            // Only the 5 rows with age = 30 are counted and paginated over —
            // the COUNT(*) uses the same filters as the SELECT.
            let page: Page<User> = QueryBuilder::new(&db.pool)
                .filter("age = ?", vec![Value::Int(30)])
                .order_by("id", Order::Asc)
                .paginate(1, 3)
                .await
                .expect("paginate filtered");
            assert_eq!(5, page.total_items, "[{}]", db.name);
            assert_eq!(2, page.total_pages);
            assert_eq!(3, page.items.len());
            assert!(page.items.iter().all(|u| u.age == Some(30)));
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_14c_query_builder_paginate_empty_table_returns_empty_page() {
        for db in test_dbs().await {
            let page: Page<User> = QueryBuilder::new(&db.pool).paginate(1, 10).await.expect("paginate empty");
            assert!(page.items.is_empty(), "[{}]", db.name);
            assert_eq!(0, page.total_items);
            assert_eq!(0, page.total_pages);
            assert!(!page.has_next());
            assert!(!page.has_prev());
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_15_query_builder_paginate_after_forward_iterates_every_row_exactly_once() {
        for db in test_dbs().await {
            seed_users(&db, 25, |_| 20).await;
            let mut cursor: Option<String> = None;
            let mut collected: Vec<String> = Vec::new();
            loop {
                let page: CursorPage<User> = QueryBuilder::new(&db.pool).paginate_after(cursor.as_deref(), 10).await.expect("paginate_after");
                collected.extend(page.items.iter().map(|u| u.name.clone()));
                if !page.has_next() {
                    break;
                }
                cursor = page.next_cursor.clone();
            }
            assert_eq!(25, collected.len(), "[{}]", db.name);
            assert_eq!("User1", collected[0], "keyset pagination should order by primary key ascending");
            assert_eq!("User25", collected[24]);
            let mut sorted = collected.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(25, sorted.len());
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_15b_query_builder_paginate_after_last_page_has_no_next_cursor() {
        for db in test_dbs().await {
            seed_users(&db, 5, |_| 20).await;
            let page: CursorPage<User> = QueryBuilder::new(&db.pool).paginate_after(None, 10).await.expect("paginate_after single page");
            assert_eq!(5, page.items.len(), "[{}]", db.name);
            assert!(!page.has_next());
            assert_eq!(None, page.next_cursor);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_15c_query_builder_paginate_after_rejects_non_numeric_cursor() {
        for db in test_dbs().await {
            let result = QueryBuilder::<User>::new(&db.pool).paginate_after(Some("not-a-number"), 10).await;
            assert!(result.is_err(), "[{}]", db.name);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn test_15d_query_builder_paginate_after_respects_filters() {
        for db in test_dbs().await {
            seed_users(&db, 10, |i| if i <= 5 { 20 } else { 30 }).await;
            let page: CursorPage<User> = QueryBuilder::new(&db.pool)
                .filter("age = ?", vec![Value::Int(30)])
                .paginate_after(None, 10)
                .await
                .expect("paginate_after filtered");
            assert_eq!(5, page.items.len(), "[{}]", db.name);
            assert!(page.items.iter().all(|u| u.age == Some(30)));
            db.finish().await;
        }
    }
}

#[test]
fn sql_operations_name_db_spans() {
    use crate::model::pool::sql_operation;
    assert_eq!("SELECT", sql_operation("  select * from t"));
    assert_eq!("INSERT", sql_operation("INSERT INTO t VALUES (1)"));
    assert_eq!("WITH", sql_operation("with x as (select 1) select * from x"));
    assert_eq!("query", sql_operation("EXPLAIN select 1"));
    assert_eq!("query", sql_operation(""));
}

#[cfg(feature = "model-sqlite")]
#[test]
fn queries_inside_a_traced_request_get_db_spans() {
    crate::otel::setup(crate::otel::TracingConfig { exporter: crate::otel::ExporterConfig::Discard, ..Default::default() });
    std::thread::spawn(|| {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        assert!(crate::model::pool::db_span_for_test("SELECT 1").is_none(), "no trace, no span");
        let _root = crate::otel::span("request");
        let span = crate::model::pool::db_span_for_test("SELECT 1").expect("a child span");
        drop(span);
        // The query itself still works with tracing on.
        let rows = runtime.block_on(async {
            let pool = crate::model::DbPool::memory().await.unwrap();
            pool.query_rows("SELECT 1 AS one", &[]).await.unwrap()
        });
        assert_eq!(1, rows.len());
    })
    .join()
    .unwrap();
}
