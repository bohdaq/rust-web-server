//! Unit tests for the model/DB layer, using SQLite in-memory.
//!
//! Run with:
//! ```bash
//! cargo test --no-default-features --features http1,macros,model-sqlite -- model
//! ```

#[cfg(all(test, feature = "model-sqlite"))]
mod model_tests {
    use crate::model::{
        DbConfig, DbConnection, HasMany, Model, ModelRow, Order, QueryBuilder,
        Repository, Value,
    };
    use crate::model::repository::ModelRepository;

    // ── Test helpers ──────────────────────────────────────────────────────────

    fn test_db() -> DbConnection {
        let config = DbConfig {
            host: "localhost".into(),
            port: 5432,
            user: "".into(),
            password: "".into(),
            database: ":memory:".into(),
            pool_size: 1,
        };
        let mut conn = DbConnection::open(&config).expect("failed to open in-memory SQLite");
        conn.execute(
            "CREATE TABLE IF NOT EXISTS users (\
             id INTEGER PRIMARY KEY AUTOINCREMENT, \
             name TEXT NOT NULL, \
             email TEXT NOT NULL UNIQUE, \
             age INTEGER\
             )",
            &[],
        )
        .expect("create users table");
        conn.execute(
            "CREATE TABLE IF NOT EXISTS posts (\
             id INTEGER PRIMARY KEY AUTOINCREMENT, \
             title TEXT NOT NULL, \
             user_id INTEGER NOT NULL\
             )",
            &[],
        )
        .expect("create posts table");
        conn
    }

    // ── Minimal Model impls for testing (without proc-macro) ─────────────────

    #[derive(Debug, Clone, PartialEq)]
    struct User {
        id: i64,
        name: String,
        email: String,
        age: Option<i32>,
    }

    impl Model for User {
        fn table_name() -> &'static str {
            "users"
        }
        fn column_names() -> &'static [&'static str] {
            &["id", "name", "email", "age"]
        }
        fn primary_key_name() -> &'static str {
            "id"
        }
        fn primary_key_value(&self) -> Value {
            Value::Int(self.id)
        }
        fn primary_key_auto_increment() -> bool {
            true
        }
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
                ("age", self.age.as_ref().map(|a| Value::Int(*a as i64)).unwrap_or(Value::Null)),
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
        fn table_name() -> &'static str {
            "posts"
        }
        fn column_names() -> &'static [&'static str] {
            &["id", "title", "user_id"]
        }
        fn primary_key_name() -> &'static str {
            "id"
        }
        fn primary_key_value(&self) -> Value {
            Value::Int(self.id)
        }
        fn primary_key_auto_increment() -> bool {
            true
        }
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
        User {
            id: 0,
            name: name.to_owned(),
            email: email.to_owned(),
            age,
        }
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_01_create_table_via_raw_sql() {
        let mut db = test_db();
        // Tables were created in test_db(); just verify they exist.
        let rows = db
            .query_rows("SELECT name FROM sqlite_master WHERE type='table' AND name='users'", &[])
            .expect("query sqlite_master");
        assert!(!rows.is_empty(), "users table should exist");
    }

    #[test]
    fn test_02_save_inserts_new_row_with_pk() {
        let mut db = test_db();
        let mut repo: ModelRepository<User, i64> = ModelRepository::new(&mut db);

        let alice = new_user("Alice", "alice@test.com", Some(30));
        let saved = repo.save(&alice).expect("save alice");

        assert!(saved.id > 0, "saved user should have a generated PK");
        assert_eq!(saved.name, "Alice");
        assert_eq!(saved.email, "alice@test.com");
        assert_eq!(saved.age, Some(30));
    }

    #[test]
    fn test_03_find_by_id_returns_correct_row() {
        let mut db = test_db();
        let mut repo: ModelRepository<User, i64> = ModelRepository::new(&mut db);

        let saved = repo.save(&new_user("Bob", "bob@test.com", None)).expect("save bob");
        let found = repo.find_by_id(saved.id).expect("find by id");

        assert!(found.is_some(), "should find the inserted row");
        let found = found.unwrap();
        assert_eq!(found.name, "Bob");
        assert_eq!(found.age, None);
    }

    #[test]
    fn test_04_find_all_returns_all_rows() {
        let mut db = test_db();
        let mut repo: ModelRepository<User, i64> = ModelRepository::new(&mut db);

        repo.save(&new_user("Carol", "carol@test.com", Some(25))).unwrap();
        repo.save(&new_user("Dave", "dave@test.com", Some(35))).unwrap();

        let all = repo.find_all().expect("find all");
        assert!(all.len() >= 2, "should return at least 2 rows");
    }

    #[test]
    fn test_05_save_on_existing_entity_updates() {
        let mut db = test_db();
        let mut repo: ModelRepository<User, i64> = ModelRepository::new(&mut db);

        let saved = repo.save(&new_user("Eve", "eve@test.com", Some(20))).unwrap();
        let mut updated = saved.clone();
        updated.name = "Eve Updated".to_owned();
        updated.age = Some(21);

        let result = repo.save(&updated).expect("update eve");
        assert_eq!(result.id, saved.id, "PK should not change on update");
        assert_eq!(result.name, "Eve Updated");
        assert_eq!(result.age, Some(21));
    }

    #[test]
    fn test_06_delete_by_id_removes_row() {
        let mut db = test_db();
        let mut repo: ModelRepository<User, i64> = ModelRepository::new(&mut db);

        let saved = repo.save(&new_user("Frank", "frank@test.com", None)).unwrap();
        repo.delete_by_id(saved.id).expect("delete frank");

        let found = repo.find_by_id(saved.id).expect("find after delete");
        assert!(found.is_none(), "deleted row should not be found");
    }

    #[test]
    fn test_07_count_returns_correct_count() {
        let mut db = test_db();
        let mut repo: ModelRepository<User, i64> = ModelRepository::new(&mut db);

        let before = repo.count().expect("count before");
        repo.save(&new_user("Grace", "grace@test.com", Some(28))).unwrap();
        let after = repo.count().expect("count after");

        assert_eq!(after, before + 1);
    }

    #[test]
    fn test_08_exists_by_id_returns_correct_boolean() {
        let mut db = test_db();
        let mut repo: ModelRepository<User, i64> = ModelRepository::new(&mut db);

        let saved = repo.save(&new_user("Hank", "hank@test.com", None)).unwrap();

        assert!(repo.exists_by_id(saved.id).expect("exists true"));
        assert!(!repo.exists_by_id(99999).expect("exists false"));
    }

    #[test]
    fn test_09_query_builder_where_order_limit_offset() {
        let mut db = test_db();
        // Insert a few rows directly.
        db.execute("INSERT INTO users (name, email, age) VALUES (?, ?, ?)", &[Value::Text("Q1".into()), Value::Text("q1@t.com".into()), Value::Int(25)]).unwrap();
        db.execute("INSERT INTO users (name, email, age) VALUES (?, ?, ?)", &[Value::Text("Q2".into()), Value::Text("q2@t.com".into()), Value::Int(30)]).unwrap();
        db.execute("INSERT INTO users (name, email, age) VALUES (?, ?, ?)", &[Value::Text("Q3".into()), Value::Text("q3@t.com".into()), Value::Int(35)]).unwrap();

        // where_eq
        let users: Vec<User> = QueryBuilder::new(&mut db)
            .where_eq("name", "Q1")
            .fetch_all()
            .expect("where_eq Q1");
        assert!(!users.is_empty());
        assert_eq!(users[0].name, "Q1");

        // filter
        let users: Vec<User> = QueryBuilder::new(&mut db)
            .filter("age >= ?", vec![Value::Int(30)])
            .fetch_all()
            .expect("filter age >= 30");
        assert!(users.iter().all(|u| u.age.unwrap_or(0) >= 30));

        // order_by + limit
        let users: Vec<User> = QueryBuilder::new(&mut db)
            .filter("name LIKE ?", vec![Value::Text("Q%".into())])
            .order_by("age", Order::Desc)
            .limit(2)
            .fetch_all()
            .expect("order limit");
        assert!(users.len() <= 2);
        if users.len() == 2 {
            assert!(users[0].age >= users[1].age, "should be DESC order");
        }

        // offset
        let all: Vec<User> = QueryBuilder::new(&mut db)
            .filter("name LIKE ?", vec![Value::Text("Q%".into())])
            .order_by("age", Order::Asc)
            .fetch_all()
            .expect("all Q users");
        let with_offset: Vec<User> = QueryBuilder::new(&mut db)
            .filter("name LIKE ?", vec![Value::Text("Q%".into())])
            .order_by("age", Order::Asc)
            .limit(100)
            .offset(1)
            .fetch_all()
            .expect("Q users offset 1");
        assert_eq!(with_offset.len(), all.len().saturating_sub(1));

        // count
        let n = QueryBuilder::<User>::new(&mut db)
            .filter("name LIKE ?", vec![Value::Text("Q%".into())])
            .count()
            .expect("count Q users");
        assert!(n >= 3);

        // fetch_one
        let one: Option<User> = QueryBuilder::new(&mut db)
            .where_eq("name", "Q2")
            .fetch_one()
            .expect("fetch_one Q2");
        assert!(one.is_some());
        assert_eq!(one.unwrap().name, "Q2");

        // delete
        QueryBuilder::<User>::new(&mut db)
            .where_eq("name", "Q3")
            .delete()
            .expect("delete Q3");
        let check: Option<User> = QueryBuilder::new(&mut db)
            .where_eq("name", "Q3")
            .fetch_one()
            .expect("check Q3 deleted");
        assert!(check.is_none(), "Q3 should be deleted");
    }

    #[test]
    fn test_10_raw_sql_query_and_execute() {
        let mut db = test_db();
        db.execute(
            "INSERT INTO users (name, email, age) VALUES (?, ?, ?)",
            &[Value::Text("Raw".into()), Value::Text("raw@test.com".into()), Value::Int(42)],
        )
        .expect("raw insert");

        // Typed query
        let users: Vec<User> = db
            .query("SELECT * FROM users WHERE name = ?", &[Value::Text("Raw".into())])
            .expect("typed query");
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].name, "Raw");

        // Untyped rows
        let rows = db
            .query_raw("SELECT name, age FROM users WHERE name = ?", &[Value::Text("Raw".into())])
            .expect("raw query");
        assert_eq!(rows.len(), 1);
        let name: String = rows[0].get("name").expect("get name");
        assert_eq!(name, "Raw");
        let age: i64 = rows[0].get("age").expect("get age");
        assert_eq!(age, 42);

        // Execute (update)
        let affected = db
            .execute(
                "UPDATE users SET age = ? WHERE name = ?",
                &[Value::Int(99), Value::Text("Raw".into())],
            )
            .expect("raw execute update");
        assert_eq!(affected, 1);
    }

    #[test]
    fn test_11_transaction_commits_on_ok() {
        let mut db = test_db();
        let result = db.transaction(|conn| {
            conn.execute(
                "INSERT INTO users (name, email, age) VALUES (?, ?, ?)",
                &[Value::Text("Tx1".into()), Value::Text("tx1@test.com".into()), Value::Int(10)],
            )?;
            Ok(42i32)
        });
        assert_eq!(result.expect("transaction should commit"), 42);

        let rows = db
            .query_rows("SELECT * FROM users WHERE name = ?", &[Value::Text("Tx1".into())])
            .expect("find tx1");
        assert_eq!(rows.len(), 1, "committed row should be visible");
    }

    #[test]
    fn test_11b_transaction_rolls_back_on_err() {
        let mut db = test_db();
        let result: Result<(), _> = db.transaction(|conn| {
            conn.execute(
                "INSERT INTO users (name, email, age) VALUES (?, ?, ?)",
                &[Value::Text("TxRB".into()), Value::Text("txrb@test.com".into()), Value::Int(5)],
            )?;
            Err(crate::model::DbError::new("intentional rollback"))
        });
        assert!(result.is_err(), "transaction should return Err");

        let rows = db
            .query_rows("SELECT * FROM users WHERE name = ?", &[Value::Text("TxRB".into())])
            .expect("find txrb");
        assert!(rows.is_empty(), "rolled-back row should not be visible");
    }

    #[test]
    fn test_12_has_many_load_returns_related_records() {
        let mut db = test_db();
        // Insert a user.
        db.execute(
            "INSERT INTO users (name, email, age) VALUES (?, ?, ?)",
            &[Value::Text("HasManyUser".into()), Value::Text("hmu@test.com".into()), Value::Int(20)],
        )
        .unwrap();
        let user_id = db.last_insert_rowid();

        // Insert posts for that user.
        db.execute(
            "INSERT INTO posts (title, user_id) VALUES (?, ?)",
            &[Value::Text("Post A".into()), Value::Int(user_id)],
        )
        .unwrap();
        db.execute(
            "INSERT INTO posts (title, user_id) VALUES (?, ?)",
            &[Value::Text("Post B".into()), Value::Int(user_id)],
        )
        .unwrap();

        // HasMany load.
        let has_many: HasMany<Post> = HasMany::new(Value::Int(user_id), "user_id");
        let posts = has_many.load(&mut db).expect("load posts");
        assert_eq!(posts.len(), 2);
        let titles: Vec<&str> = posts.iter().map(|p| p.title.as_str()).collect();
        assert!(titles.contains(&"Post A"));
        assert!(titles.contains(&"Post B"));
    }

    #[test]
    fn test_13_migration_runner_creates_table_and_tracks_versions() {
        use std::io::Write;

        let mut db = test_db();

        // Create a temporary directory with migration files.
        let dir = tempdir_path();
        std::fs::create_dir_all(&dir).unwrap();

        // Write two migration files.
        let f1 = format!("{}/0001_create_items.sql", dir);
        let f2 = format!("{}/0002_add_col.sql", dir);
        {
            let mut file = std::fs::File::create(&f1).unwrap();
            writeln!(file, "CREATE TABLE IF NOT EXISTS items (id INTEGER PRIMARY KEY, name TEXT)").unwrap();
        }
        {
            let mut file = std::fs::File::create(&f2).unwrap();
            writeln!(file, "ALTER TABLE items ADD COLUMN qty INTEGER DEFAULT 0").unwrap();
        }

        // Run migrations.
        db.migrate(&dir).expect("run migrations");

        // Verify _schema_migrations table was created and populated.
        let rows = db
            .query_rows("SELECT version FROM _schema_migrations ORDER BY version", &[])
            .expect("query migrations");
        let versions: Vec<String> = rows.iter().map(|r| r.get("version").unwrap()).collect();
        assert!(versions.contains(&"0001_create_items.sql".to_owned()));
        assert!(versions.contains(&"0002_add_col.sql".to_owned()));

        // Running again should be a no-op (idempotent).
        db.migrate(&dir).expect("re-run migrations idempotent");
        let rows2 = db
            .query_rows("SELECT version FROM _schema_migrations ORDER BY version", &[])
            .expect("query migrations again");
        assert_eq!(rows2.len(), 2, "should still have exactly 2 migration records");

        // migration_status
        let status = db.migration_status(&dir).expect("migration status");
        assert_eq!(status.len(), 2);
        assert!(status.iter().all(|s| s.applied));

        // Clean up.
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn tempdir_path() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("/tmp/rws_model_test_migrations_{}", n)
    }

    // ── In-memory constructors ─────────────────────────────────────────────────

    #[test]
    fn db_config_memory_has_memory_path_and_pool_size_one() {
        let cfg = DbConfig::memory();
        assert_eq!(":memory:", cfg.database);
        assert_eq!(1, cfg.pool_size);
    }

    #[test]
    fn db_connection_memory_opens_and_accepts_ddl() {
        let mut conn = DbConnection::memory().expect("open :memory:");
        conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT NOT NULL)", &[])
            .expect("create table");
        let rows = conn.execute("INSERT INTO t (v) VALUES (?1)", &[Value::Text("hi".into())])
            .expect("insert");
        assert_eq!(1, rows);
    }

    #[test]
    fn db_connection_memory_is_isolated_per_call() {
        // Each DbConnection::memory() call must return an independent database.
        let mut a = DbConnection::memory().unwrap();
        let mut b = DbConnection::memory().unwrap();
        a.execute("CREATE TABLE t (x INTEGER)", &[]).unwrap();
        a.execute("INSERT INTO t VALUES (42)", &[]).unwrap();
        // b sees a completely separate schema — CREATE on b must not fail.
        b.execute("CREATE TABLE t (x INTEGER)", &[]).unwrap();
        let rows_b = b.query_rows("SELECT x FROM t", &[]).unwrap();
        assert!(rows_b.is_empty(), "b should see an empty table, not a's data");
    }

    #[test]
    fn db_pool_memory_single_connection_sees_correct_data() {
        use crate::model::DbPool;
        let pool = DbPool::memory().expect("create memory pool");

        {
            let mut conn = pool.get().expect("checkout");
            conn.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)", &[]).unwrap();
            conn.execute("INSERT INTO items (name) VALUES (?1)", &[Value::Text("apple".into())]).unwrap();
        } // connection returned to pool here

        {
            let mut conn = pool.get().expect("re-checkout same connection");
            let rows = conn.query_rows("SELECT name FROM items", &[]).unwrap();
            assert_eq!(1, rows.len());
            let name: String = rows[0].get("name").unwrap();
            assert_eq!("apple", name);
        }
    }

    #[test]
    fn db_pool_memory_exhaustion_returns_clear_error() {
        use crate::model::DbPool;
        let pool = DbPool::memory().expect("create memory pool");

        // Hold the only connection.
        let _conn = pool.get().expect("first checkout");

        // Second get() must return an error, not silently open an empty database.
        let result = pool.get();
        assert!(result.is_err(), "second get() on exhausted memory pool must fail");
        let msg = result.err().unwrap().0;
        assert!(msg.contains("exhausted"), "error message should mention exhaustion: {}", msg);
    }
}
