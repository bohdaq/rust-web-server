//! Async integration tests for the model layer (SQLite in-memory).
//!
//! Run with:
//! ```bash
//! cargo test --no-default-features --features http2,macros,model-sqlite -- model
//! ```

#[cfg(all(test, feature = "model-sqlite"))]
mod model_tests {
    use crate::model::{
        DbPool, HasMany, Model, ModelRow, Order, QueryBuilder, Repository, Value,
    };
    use crate::model::repository::ModelRepository;

    // ── Test helpers ──────────────────────────────────────────────────────────

    async fn test_pool() -> DbPool {
        let pool = DbPool::memory().await.expect("open :memory: pool");
        pool.execute(
            "CREATE TABLE IF NOT EXISTS users (\
             id INTEGER PRIMARY KEY AUTOINCREMENT, \
             name TEXT NOT NULL, \
             email TEXT NOT NULL UNIQUE, \
             age INTEGER\
             )",
            &[],
        )
        .await
        .expect("create users table");
        pool.execute(
            "CREATE TABLE IF NOT EXISTS posts (\
             id INTEGER PRIMARY KEY AUTOINCREMENT, \
             title TEXT NOT NULL, \
             user_id INTEGER NOT NULL\
             )",
            &[],
        )
        .await
        .expect("create posts table");
        pool
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

    // ── Tests ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_01_create_table_via_raw_sql() {
        let pool = test_pool().await;
        let rows = pool
            .query_rows("SELECT name FROM sqlite_master WHERE type='table' AND name='users'", &[])
            .await
            .expect("query sqlite_master");
        assert!(!rows.is_empty(), "users table should exist");
    }

    #[tokio::test]
    async fn test_02_save_inserts_new_row_with_pk() {
        let pool = test_pool().await;
        let repo: ModelRepository<User, i64> = ModelRepository::new(&pool);

        let alice = new_user("Alice", "alice@test.com", Some(30));
        let saved = repo.save(&alice).await.expect("save alice");

        assert!(saved.id > 0, "saved user should have a generated PK");
        assert_eq!(saved.name, "Alice");
        assert_eq!(saved.email, "alice@test.com");
        assert_eq!(saved.age, Some(30));
    }

    #[tokio::test]
    async fn test_03_find_by_id_returns_correct_row() {
        let pool = test_pool().await;
        let repo: ModelRepository<User, i64> = ModelRepository::new(&pool);

        let saved = repo.save(&new_user("Bob", "bob@test.com", None)).await.expect("save bob");
        let found = repo.find_by_id(saved.id).await.expect("find by id");

        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.name, "Bob");
        assert_eq!(found.age, None);
    }

    #[tokio::test]
    async fn test_04_find_all_returns_all_rows() {
        let pool = test_pool().await;
        let repo: ModelRepository<User, i64> = ModelRepository::new(&pool);

        repo.save(&new_user("Carol", "carol@test.com", Some(25))).await.unwrap();
        repo.save(&new_user("Dave", "dave@test.com", Some(35))).await.unwrap();

        let all = repo.find_all().await.expect("find all");
        assert!(all.len() >= 2);
    }

    #[tokio::test]
    async fn test_05_save_on_existing_entity_updates() {
        let pool = test_pool().await;
        let repo: ModelRepository<User, i64> = ModelRepository::new(&pool);

        let saved = repo.save(&new_user("Eve", "eve@test.com", Some(20))).await.unwrap();
        let mut updated = saved.clone();
        updated.name = "Eve Updated".to_owned();
        updated.age = Some(21);

        let result = repo.save(&updated).await.expect("update eve");
        assert_eq!(result.id, saved.id);
        assert_eq!(result.name, "Eve Updated");
        assert_eq!(result.age, Some(21));
    }

    #[tokio::test]
    async fn test_06_delete_by_id_removes_row() {
        let pool = test_pool().await;
        let repo: ModelRepository<User, i64> = ModelRepository::new(&pool);

        let saved = repo.save(&new_user("Frank", "frank@test.com", None)).await.unwrap();
        repo.delete_by_id(saved.id).await.expect("delete frank");

        let found = repo.find_by_id(saved.id).await.expect("find after delete");
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_07_count_returns_correct_count() {
        let pool = test_pool().await;
        let repo: ModelRepository<User, i64> = ModelRepository::new(&pool);

        let before = repo.count().await.expect("count before");
        repo.save(&new_user("Grace", "grace@test.com", Some(28))).await.unwrap();
        let after = repo.count().await.expect("count after");

        assert_eq!(after, before + 1);
    }

    #[tokio::test]
    async fn test_08_exists_by_id_returns_correct_boolean() {
        let pool = test_pool().await;
        let repo: ModelRepository<User, i64> = ModelRepository::new(&pool);

        let saved = repo.save(&new_user("Hank", "hank@test.com", None)).await.unwrap();

        assert!(repo.exists_by_id(saved.id).await.expect("exists true"));
        assert!(!repo.exists_by_id(99999).await.expect("exists false"));
    }

    #[tokio::test]
    async fn test_09_query_builder_where_order_limit_offset() {
        let pool = test_pool().await;
        pool.execute("INSERT INTO users (name, email, age) VALUES (?, ?, ?)", &[Value::Text("Q1".into()), Value::Text("q1@t.com".into()), Value::Int(25)]).await.unwrap();
        pool.execute("INSERT INTO users (name, email, age) VALUES (?, ?, ?)", &[Value::Text("Q2".into()), Value::Text("q2@t.com".into()), Value::Int(30)]).await.unwrap();
        pool.execute("INSERT INTO users (name, email, age) VALUES (?, ?, ?)", &[Value::Text("Q3".into()), Value::Text("q3@t.com".into()), Value::Int(35)]).await.unwrap();

        // where_eq
        let users: Vec<User> = QueryBuilder::new(&pool)
            .where_eq("name", "Q1")
            .fetch_all().await.expect("where_eq Q1");
        assert!(!users.is_empty());
        assert_eq!(users[0].name, "Q1");

        // filter
        let users: Vec<User> = QueryBuilder::new(&pool)
            .filter("age >= ?", vec![Value::Int(30)])
            .fetch_all().await.expect("filter age >= 30");
        assert!(users.iter().all(|u| u.age.unwrap_or(0) >= 30));

        // order_by + limit
        let users: Vec<User> = QueryBuilder::new(&pool)
            .filter("name LIKE ?", vec![Value::Text("Q%".into())])
            .order_by("age", Order::Desc)
            .limit(2)
            .fetch_all().await.expect("order limit");
        assert!(users.len() <= 2);
        if users.len() == 2 {
            assert!(users[0].age >= users[1].age, "should be DESC order");
        }

        // count
        let n = QueryBuilder::<User>::new(&pool)
            .filter("name LIKE ?", vec![Value::Text("Q%".into())])
            .count().await.expect("count Q users");
        assert!(n >= 3);

        // fetch_one
        let one: Option<User> = QueryBuilder::new(&pool)
            .where_eq("name", "Q2")
            .fetch_one().await.expect("fetch_one Q2");
        assert!(one.is_some());
        assert_eq!(one.unwrap().name, "Q2");

        // delete
        QueryBuilder::<User>::new(&pool)
            .where_eq("name", "Q3")
            .delete().await.expect("delete Q3");
        let check: Option<User> = QueryBuilder::new(&pool)
            .where_eq("name", "Q3")
            .fetch_one().await.expect("check Q3 deleted");
        assert!(check.is_none(), "Q3 should be deleted");
    }

    #[tokio::test]
    async fn test_10_raw_sql_query_and_execute() {
        let pool = test_pool().await;
        pool.execute(
            "INSERT INTO users (name, email, age) VALUES (?, ?, ?)",
            &[Value::Text("Raw".into()), Value::Text("raw@test.com".into()), Value::Int(42)],
        ).await.expect("raw insert");

        let users: Vec<User> = pool
            .query("SELECT * FROM users WHERE name = ?", &[Value::Text("Raw".into())])
            .await.expect("typed query");
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].name, "Raw");

        let rows = pool
            .query_raw("SELECT name, age FROM users WHERE name = ?", &[Value::Text("Raw".into())])
            .await.expect("raw query");
        assert_eq!(rows.len(), 1);
        let name: String = rows[0].get("name").unwrap();
        assert_eq!(name, "Raw");
        let age: i64 = rows[0].get("age").unwrap();
        assert_eq!(age, 42);

        let affected = pool
            .execute(
                "UPDATE users SET age = ? WHERE name = ?",
                &[Value::Int(99), Value::Text("Raw".into())],
            ).await.expect("raw execute update");
        assert_eq!(affected, 1);
    }

    #[tokio::test]
    async fn test_11_transaction_commits_on_ok() {
        let pool = test_pool().await;
        let result = pool.transaction(|mut tx| async move {
            tx.execute(
                "INSERT INTO users (name, email, age) VALUES (?, ?, ?)",
                &[Value::Text("Tx1".into()), Value::Text("tx1@test.com".into()), Value::Int(10)],
            ).await?;
            tx.commit().await?;
            Ok(42i32)
        }).await;
        assert_eq!(result.expect("transaction should commit"), 42);

        let rows = pool
            .query_rows("SELECT * FROM users WHERE name = ?", &[Value::Text("Tx1".into())])
            .await.expect("find tx1");
        assert_eq!(rows.len(), 1, "committed row should be visible");
    }

    #[tokio::test]
    async fn test_11b_transaction_rolls_back_on_err() {
        let pool = test_pool().await;
        let result: Result<(), _> = pool.transaction(|mut tx| async move {
            tx.execute(
                "INSERT INTO users (name, email, age) VALUES (?, ?, ?)",
                &[Value::Text("TxRB".into()), Value::Text("txrb@test.com".into()), Value::Int(5)],
            ).await?;
            Err(crate::model::DbError::new("intentional rollback"))
        }).await;
        assert!(result.is_err());

        let rows = pool
            .query_rows("SELECT * FROM users WHERE name = ?", &[Value::Text("TxRB".into())])
            .await.expect("find txrb");
        assert!(rows.is_empty(), "rolled-back row should not be visible");
    }

    #[tokio::test]
    async fn test_12_has_many_load_returns_related_records() {
        let pool = test_pool().await;
        pool.execute(
            "INSERT INTO users (name, email, age) VALUES (?, ?, ?)",
            &[Value::Text("HMUser".into()), Value::Text("hmu@test.com".into()), Value::Int(20)],
        ).await.unwrap();

        let rows = pool.query_rows("SELECT last_insert_rowid() AS id", &[]).await.unwrap();
        let user_id: i64 = rows[0].get("id").unwrap();

        pool.execute("INSERT INTO posts (title, user_id) VALUES (?, ?)", &[Value::Text("Post A".into()), Value::Int(user_id)]).await.unwrap();
        pool.execute("INSERT INTO posts (title, user_id) VALUES (?, ?)", &[Value::Text("Post B".into()), Value::Int(user_id)]).await.unwrap();

        let has_many: HasMany<Post> = HasMany::new(Value::Int(user_id), "user_id");
        let posts = has_many.load(&pool).await.expect("load posts");
        assert_eq!(posts.len(), 2);
        let titles: Vec<&str> = posts.iter().map(|p| p.title.as_str()).collect();
        assert!(titles.contains(&"Post A"));
        assert!(titles.contains(&"Post B"));
    }

    #[tokio::test]
    async fn test_13_migration_runner_creates_table_and_tracks_versions() {
        use std::io::Write;

        let pool = test_pool().await;
        let dir = tempdir_path();
        std::fs::create_dir_all(&dir).unwrap();

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

        pool.migrate(&dir).await.expect("run migrations");

        let rows = pool
            .query_rows("SELECT version FROM _schema_migrations ORDER BY version", &[])
            .await.expect("query migrations");
        let versions: Vec<String> = rows.iter().map(|r| r.get("version").unwrap()).collect();
        assert!(versions.contains(&"0001_create_items.sql".to_owned()));
        assert!(versions.contains(&"0002_add_col.sql".to_owned()));

        // idempotent
        pool.migrate(&dir).await.expect("re-run migrations idempotent");
        let rows2 = pool
            .query_rows("SELECT version FROM _schema_migrations ORDER BY version", &[])
            .await.expect("query migrations again");
        assert_eq!(rows2.len(), 2);

        // migration_status
        let status = pool.migration_status(&dir).await.expect("migration status");
        assert_eq!(status.len(), 2);
        assert!(status.iter().all(|s| s.applied));

        let _ = std::fs::remove_dir_all(&dir);
    }

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

    fn tempdir_path() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("/tmp/rws_model_test_migrations_{}", n)
    }
}
