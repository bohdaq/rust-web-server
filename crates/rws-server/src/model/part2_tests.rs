//! 18.30.0, part 2: eager loading (`with`, `Relation`), many-to-many
//! (`BelongsToMany`), scopes, Rust seeders, read replicas and the schema
//! dump — on every backend `backends.rs` provides.

use crate::model::eager::{distinct_keys, key_of};
use crate::model::Value;

#[test]
fn keys_tell_values_apart_and_skip_null() {
    assert_eq!(None, key_of(Value::Null));
    assert_ne!(key_of(Value::Int(1)), key_of(Value::Text("1".into())));
    assert_eq!(key_of(Value::Int(1)), key_of(Value::Bool(true)));
    let keys = distinct_keys(&[Value::Int(2), Value::Null, Value::Int(1), Value::Int(2), Value::Text("a".into())]);
    assert_eq!(vec![Value::Int(2), Value::Int(1), Value::Text("a".into())], keys);
}

#[cfg(feature = "macros")]
mod on_backends {
    use crate::model::backends::{self, TestBackend};
    use crate::model::{
        BelongsToMany, DbPool, Order, QueryBuilder, Relation, RelationKind, Repository, Seeders, Timestamp, Value,
    };
    use crate::test_env::TempPath;

    #[derive(rws_macros::Model, rws_macros::Factory, Debug, Clone, PartialEq)]
    #[table(name = "e_authors")]
    struct Author {
        #[primary_key(auto_increment)]
        id: i64,
        name: String,
    }

    #[derive(rws_macros::Model, Debug, Clone, PartialEq)]
    #[table(name = "e_posts")]
    struct Post {
        #[primary_key(auto_increment)]
        id: i64,
        title: String,
        author_id: Option<i64>,
        published: bool,
    }

    #[derive(rws_macros::Model, Debug, Clone, PartialEq)]
    #[table(name = "e_comments")]
    #[soft_delete]
    struct Comment {
        #[primary_key(auto_increment)]
        id: i64,
        post_id: i64,
        body: String,
        deleted_at: Option<Timestamp>,
    }

    #[derive(rws_macros::Model, Debug, Clone, PartialEq)]
    #[table(name = "e_profiles")]
    struct Profile {
        #[primary_key(auto_increment)]
        id: i64,
        author_id: i64,
        bio: String,
    }

    #[derive(rws_macros::Model, rws_macros::Factory, Debug, Clone, PartialEq)]
    #[table(name = "e_labels")]
    struct Label {
        #[primary_key(auto_increment)]
        id: i64,
        name: String,
    }

    impl Post {
        fn comments() -> Relation<Post, Comment> {
            Relation::has_many("post_id")
        }
        fn author() -> Relation<Post, Author> {
            Relation::belongs_to("author_id")
        }
        fn labels() -> Relation<Post, Label> {
            Relation::belongs_to_many("e_post_labels", "post_id", "label_id")
        }
        fn label_links(&self) -> BelongsToMany<Label> {
            BelongsToMany::new(Value::Int(self.id), "e_post_labels", "post_id", "label_id")
        }
    }

    impl Author {
        fn profile() -> Relation<Author, Profile> {
            Relation::has_one("author_id")
        }
        fn posts() -> Relation<Author, Post> {
            Relation::has_many("author_id")
        }
    }

    const SCHEMA: &str = "\
        CREATE TABLE e_authors (id {pk}, name {text} NOT NULL UNIQUE){engine};\
        CREATE TABLE e_posts (id {pk}, title {text} NOT NULL, author_id BIGINT, published BOOLEAN NOT NULL, \
          FOREIGN KEY (author_id) REFERENCES e_authors (id) ON DELETE SET NULL){engine};\
        CREATE TABLE e_comments (id {pk}, post_id BIGINT NOT NULL, body {text} NOT NULL, deleted_at {ts}){engine};\
        CREATE TABLE e_profiles (id {pk}, author_id BIGINT NOT NULL, bio {text} NOT NULL){engine};\
        CREATE TABLE e_labels (id {pk}, name {text} NOT NULL){engine};\
        CREATE TABLE e_post_labels (post_id BIGINT NOT NULL, label_id BIGINT NOT NULL, PRIMARY KEY (post_id, label_id)){engine};\
        CREATE INDEX e_posts_by_title ON e_posts (title, published)";

    async fn dbs() -> Vec<TestBackend> {
        let mut out = Vec::new();
        for db in backends::all().await {
            db.create(SCHEMA).await;
            out.push(db);
        }
        out
    }

    async fn author(pool: &DbPool, name: &str) -> Author {
        Author::repository(pool).save(&Author { id: 0, name: name.into() }).await.unwrap()
    }

    async fn post(pool: &DbPool, title: &str, author_id: Option<i64>, published: bool) -> Post {
        Post::repository(pool).save(&Post { id: 0, title: title.into(), author_id, published }).await.unwrap()
    }

    async fn comment(pool: &DbPool, post_id: i64, body: &str) -> Comment {
        Comment::repository(pool).save(&Comment { id: 0, post_id, body: body.into(), deleted_at: None }).await.unwrap()
    }

    fn bodies(comments: &[Comment]) -> Vec<&str> {
        comments.iter().map(|c| c.body.as_str()).collect()
    }

    #[test]
    fn relations_say_what_they_are() {
        assert_eq!(RelationKind::HasMany, Post::comments().kind());
        assert_eq!(("post_id", None), (Post::comments().foreign_key(), Post::comments().join_table()));
        assert_eq!(RelationKind::BelongsTo, Post::author().kind());
        assert_eq!(Some(("e_post_labels", "label_id")), Post::labels().join_table());
        assert_eq!("post_id", Post::labels().foreign_key());
        assert!(format!("{:?}", Author::profile()).contains("HasOne"));
    }

    #[tokio::test]
    async fn with_loads_each_relation_for_the_whole_result_set() {
        for db in dbs().await {
            let pool = &db.pool;
            let (ann, bo) = (author(pool, "ann").await, author(pool, "bo").await);
            let first = post(pool, "first", Some(ann.id), true).await;
            let second = post(pool, "second", Some(bo.id), true).await;
            let orphan = post(pool, "orphan", None, false).await;
            comment(pool, first.id, "a").await;
            comment(pool, first.id, "b").await;
            let gone = comment(pool, second.id, "deleted").await;
            Comment::repository(pool).delete_by_id(gone.id).await.unwrap();
            Profile::repository(pool).save(&Profile { id: 0, author_id: ann.id, bio: "hi".into() }).await.unwrap();

            let posts = Post::query(pool)
                .order_by("id", Order::Asc)
                .with(Post::comments())
                .with(Post::author())
                .with(Post::comments()) // named twice, loaded once
                .fetch_all()
                .await
                .unwrap_or_else(|e| panic!("[{}] {e}", db.name));
            assert_eq!(vec!["first", "second", "orphan"], posts.iter().map(|p| p.title.as_str()).collect::<Vec<_>>(), "[{}]", db.name);
            assert_eq!(vec!["a", "b"], bodies(posts[0].related(&Post::comments())), "[{}]", db.name);
            assert!(posts[1].related(&Post::comments()).is_empty(), "[{}] soft-deleted comments are left out", db.name);
            assert!(posts[1].is_loaded(&Post::comments()) && !posts[1].is_loaded(&Post::labels()));
            assert_eq!(Some("ann"), posts[0].one(&Post::author()).map(|a| a.name.as_str()), "[{}]", db.name);
            assert_eq!(Some("bo"), posts[1].one(&Post::author()).map(|a| a.name.as_str()));
            assert_eq!(None, posts[2].one(&Post::author()), "[{}] a NULL key belongs to nothing", db.name);
            assert_eq!(&orphan, posts[2].model());
            assert!(format!("{:?}", posts[0]).contains("e_comments"));

            // has_one, and has_many back the other way.
            let authors = Author::query(pool).order_by("id", Order::Asc).with(Author::profile()).with(Author::posts()).fetch_all().await.unwrap();
            assert_eq!(Some("hi"), authors[0].one(&Author::profile()).map(|p| p.bio.as_str()), "[{}]", db.name);
            assert_eq!(None, authors[1].one(&Author::profile()));
            assert_eq!(vec![second.id], authors[1].related(&Author::posts()).iter().map(|p| p.id).collect::<Vec<_>>());

            // fetch_one, paginate, an empty result, take.
            let one = Post::query(pool).where_eq("title", "first").with(Post::comments()).fetch_one().await.unwrap().expect("first");
            assert_eq!(2, one.related(&Post::comments()).len());
            let page = Post::query(pool).order_by("id", Order::Asc).with(Post::author()).paginate(2, 2).await.unwrap();
            assert_eq!((1, 3, 2), (page.items.len(), page.total_items, page.total_pages), "[{}]", db.name);
            assert_eq!("orphan", page.items[0].title);
            assert!(Post::query(pool).where_eq("title", "none").with(Post::comments()).fetch_all().await.unwrap().is_empty());
            let mut loaded = Post::query(pool).where_eq("id", first.id).with(Post::comments()).fetch_one().await.unwrap().unwrap();
            assert_eq!(2, loaded.take(&Post::comments()).len());
            assert!(loaded.related(&Post::comments()).is_empty());
            assert_eq!(first, loaded.into_inner());

            // One owner at a time, and many without a query builder.
            assert_eq!(vec!["a", "b"], bodies(&Post::comments().load(pool, &first).await.unwrap()));
            let per_post = Post::comments().load_for(pool, &[second.clone(), first.clone(), first.clone()]).await.unwrap();
            assert_eq!((0, 2, 2), (per_post[0].len(), per_post[1].len(), per_post[2].len()), "[{}]", db.name);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn nested_relations_load_one_query_per_level() {
        for db in dbs().await {
            let pool = &db.pool;
            let (ann, bo) = (author(pool, "ann").await, author(pool, "bo").await);
            let a1 = post(pool, "a1", Some(ann.id), true).await;
            let a2 = post(pool, "a2", Some(ann.id), true).await;
            let b1 = post(pool, "b1", Some(bo.id), true).await;
            comment(pool, a1.id, "on a1").await;
            comment(pool, a2.id, "on a2, first").await;
            comment(pool, a2.id, "on a2, second").await;
            Profile::repository(pool).save(&Profile { id: 0, author_id: bo.id, bio: "bo's".into() }).await.unwrap();

            // authors → posts → (comments, author → profile)
            let authors = Author::query(pool)
                .order_by("id", Order::Asc)
                .with_nested(Author::posts(), |posts| {
                    posts.with(Post::comments()).with_nested(Post::author(), |author| author.with(Author::profile())).with(Post::comments())
                })
                .with(Author::profile())
                .fetch_all()
                .await
                .unwrap_or_else(|e| panic!("[{}] {e}", db.name));
            let ann_posts = authors[0].related_loaded(&Author::posts());
            assert_eq!(vec!["a1", "a2"], ann_posts.iter().map(|p| p.title.as_str()).collect::<Vec<_>>(), "[{}]", db.name);
            assert_eq!(vec!["on a1"], bodies(ann_posts[0].related(&Post::comments())), "[{}]", db.name);
            assert_eq!(vec!["on a2, first", "on a2, second"], bodies(ann_posts[1].related(&Post::comments())));
            // The plain rows are there too.
            assert_eq!(2, authors[0].related(&Author::posts()).len());
            let bo_posts = authors[1].related_loaded(&Author::posts());
            assert_eq!((1, b1.id), (bo_posts.len(), bo_posts[0].id));
            assert!(bo_posts[0].related(&Post::comments()).is_empty());
            // Three levels down: the post's author's profile.
            let back = bo_posts[0].related_loaded(&Post::author());
            assert_eq!(Some("bo's"), back[0].one(&Author::profile()).map(|p| p.bio.as_str()), "[{}]", db.name);
            assert_eq!(Some("bo's"), authors[1].one(&Author::profile()).map(|p| p.bio.as_str()));
            // Loaded without nesting: no nested rows.
            let plain = Author::query(pool).with(Author::posts()).fetch_all().await.unwrap();
            assert!(plain[0].related_loaded(&Author::posts()).is_empty() && !plain[0].related(&Author::posts()).is_empty());
            // From a QueryBuilder too, and on an empty result.
            assert!(Author::query(pool).where_eq("name", "nobody").with_nested(Author::posts(), |p| p.with(Post::comments())).fetch_all().await.unwrap().is_empty());
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn eager_loading_batches_large_result_sets() {
        for db in dbs().await {
            let pool = &db.pool;
            let ann = author(pool, "ann").await;
            let n = crate::model::eager::IN_BATCH + 3;
            for i in 0..n {
                post(pool, &format!("p{i}"), Some(ann.id), i % 2 == 0).await;
            }
            let posts = Post::query(pool).with(Post::author()).fetch_all().await.unwrap();
            assert_eq!(n, posts.len());
            assert!(posts.iter().all(|p| p.one(&Post::author()).map(|a| a.id) == Some(ann.id)), "[{}]", db.name);
            let authors = Author::query(pool).with(Author::posts()).fetch_all().await.unwrap();
            assert_eq!(n, authors[0].related(&Author::posts()).len(), "[{}]", db.name);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn belongs_to_many_attaches_detaches_and_syncs() {
        for db in dbs().await {
            let pool = &db.pool;
            let p = post(pool, "tagged", None, true).await;
            let other = post(pool, "other", None, true).await;
            let labels = Label::factory().create_many(4, pool).await.unwrap();
            let id = |i: usize| labels[i].id;

            let links = p.label_links();
            assert_eq!(2, links.attach(pool, [id(0), id(1), id(1)]).await.unwrap_or_else(|e| panic!("[{}] {e}", db.name)));
            assert_eq!(1, links.attach(pool, [id(1), id(2)]).await.unwrap(), "[{}] linked once", db.name);
            assert_eq!(0, links.attach(pool, Vec::<i64>::new()).await.unwrap());
            other.label_links().attach(pool, [id(0)]).await.unwrap();
            let loaded: Vec<i64> = links.load(pool).await.unwrap().iter().map(|l| l.id).collect();
            assert_eq!(vec![id(0), id(1), id(2)], loaded, "[{}]", db.name);
            assert_eq!(3, links.ids(pool).await.unwrap().len());

            assert_eq!(1, links.detach(pool, [id(1), id(3)]).await.unwrap(), "[{}]", db.name);
            let changes = links.sync(pool, [id(2), id(3)]).await.unwrap();
            assert_eq!(vec![Value::Int(id(0))], changes.detached, "[{}]", db.name);
            assert_eq!(vec![Value::Int(id(3))], changes.attached);
            assert_eq!(vec![id(2), id(3)], links.load(pool).await.unwrap().iter().map(|l| l.id).collect::<Vec<_>>());
            assert_eq!(crate::model::SyncChanges::default(), links.sync(pool, [id(3), id(2)]).await.unwrap(), "already in sync");

            // Eager: each post its own labels, a shared label cloned into both.
            other.label_links().attach(pool, [id(3)]).await.unwrap();
            let posts = Post::query(pool).order_by("id", Order::Asc).with(Post::labels()).fetch_all().await.unwrap();
            let names = |i: usize| posts[i].related(&Post::labels()).iter().map(|l| l.id).collect::<Vec<_>>();
            assert_eq!((vec![id(2), id(3)], vec![id(0), id(3)]), (names(0), names(1)), "[{}]", db.name);

            assert_eq!(2, links.detach_all(pool).await.unwrap());
            assert!(links.load(pool).await.unwrap().is_empty());
            assert_eq!(2, other.label_links().ids(pool).await.unwrap().len(), "[{}] other links kept", db.name);
            db.finish().await;
        }
    }

    // ── Scopes ────────────────────────────────────────────────────────────────

    fn published(q: QueryBuilder<'_, Post>) -> QueryBuilder<'_, Post> {
        q.where_eq("published", true)
    }

    fn titled<'a>(titles: &'a [&'a str]) -> impl FnOnce(QueryBuilder<'_, Post>) -> QueryBuilder<'_, Post> + 'a {
        move |q| q.where_in("title", titles.iter().copied())
    }

    trait PostScopes {
        fn published(self) -> Self;
        fn without_author(self) -> Self;
    }

    impl PostScopes for QueryBuilder<'_, Post> {
        fn published(self) -> Self {
            published(self)
        }
        fn without_author(self) -> Self {
            self.where_null("author_id")
        }
    }

    #[tokio::test]
    async fn scopes_compose() {
        for db in dbs().await {
            let pool = &db.pool;
            let ann = author(pool, "ann").await;
            post(pool, "a", Some(ann.id), true).await;
            post(pool, "b", None, true).await;
            post(pool, "c", None, false).await;
            let titles = |posts: Vec<Post>| posts.into_iter().map(|p| p.title).collect::<Vec<_>>();
            let q = || Post::query(pool).order_by("title", Order::Asc);
            assert_eq!(vec!["a", "b"], titles(q().scope(published).fetch_all().await.unwrap()), "[{}]", db.name);
            assert_eq!(vec!["b"], titles(q().published().without_author().fetch_all().await.unwrap()), "[{}]", db.name);
            assert_eq!(vec!["a", "c"], titles(q().scope(titled(&["a", "c", "z"])).fetch_all().await.unwrap()), "[{}]", db.name);
            assert_eq!(1, q().scope(titled(&["a", "c"])).scope(published).count().await.unwrap());
            assert_eq!(0, q().where_in("title", Vec::<String>::new()).count().await.unwrap(), "[{}] IN () matches nothing", db.name);
            assert_eq!(vec!["a"], titles(q().where_not_null("author_id").fetch_all().await.unwrap()));
            // A scope with eager loading after it.
            let loaded = q().published().with(Post::author()).fetch_all().await.unwrap();
            assert_eq!(Some(ann.id), loaded[0].one(&Post::author()).map(|a| a.id), "[{}]", db.name);
            db.finish().await;
        }
    }

    // ── Seeds ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn rust_seeders_run_after_the_sql_files() {
        let dir = TempPath::new("model_rust_seeds");
        std::fs::create_dir_all(&*dir).unwrap();
        std::fs::write(dir.join("01_labels.sql"), "INSERT INTO e_labels (name) VALUES ('from-sql')").unwrap();
        let seeders = Seeders::new()
            .add("authors", |pool: DbPool| async move {
                Author::factory().create_many(3, &pool).await?;
                Ok(())
            })
            .add("labels", |pool: DbPool| async move {
                // Sees what the SQL seed wrote.
                let n = QueryBuilder::<Label>::new(&pool).count().await?;
                Label::factory().with(move |l| l.name = format!("after-{n}")).create(&pool).await?;
                Ok(())
            });
        assert_eq!((vec!["authors", "labels"], 2, false), (seeders.names(), seeders.len(), seeders.is_empty()));
        assert!(format!("{seeders:?}").contains("authors"));
        for db in dbs().await {
            let pool = &db.pool;
            let ran = pool.seed_with(&dir.to_string_lossy(), &seeders).await.unwrap_or_else(|e| panic!("[{}] {e}", db.name));
            assert_eq!(vec!["01_labels.sql", "authors", "labels"], ran, "[{}]", db.name);
            assert_eq!(3, Author::repository(pool).count().await.unwrap());
            let names: Vec<String> = Label::query(pool).order_by("id", Order::Asc).fetch_all().await.unwrap().into_iter().map(|l| l.name).collect();
            assert_eq!(vec!["from-sql", "after-1"], names, "[{}]", db.name);

            // Seeders only: a missing directory is no SQL seeds.
            assert_eq!(vec!["authors", "labels"], pool.seed_with("/nonexistent/seeds", &seeders).await.unwrap());
            // Without seeders it is still an error, as for seed().
            assert!(pool.seed_with("/nonexistent/seeds", &Seeders::new()).await.is_err());
            // A failing seeder names itself.
            let failing = Seeders::new().add("broken", |pool: DbPool| async move {
                pool.execute("INSERT INTO no_such_table VALUES (1)", &[]).await?;
                Ok(())
            });
            let err = pool.seed_with("/nonexistent/seeds", &failing).await.expect_err("fails");
            assert!(err.0.contains("seeder 'broken' failed"), "[{}] {err}", db.name);
            db.finish().await;
        }
    }

    // ── Read replicas ─────────────────────────────────────────────────────────

    /// Two SQLite databases standing in for a primary and a replica that
    /// replication hasn't reached: what each statement saw shows where it
    /// went.
    #[cfg(feature = "model-sqlite")]
    #[tokio::test]
    async fn reads_go_to_replicas_and_writes_to_the_primary() {
        let (primary, replica) = (DbPool::memory().await.unwrap(), DbPool::memory().await.unwrap());
        for db in [&primary, &replica] {
            db.execute("CREATE TABLE e_labels (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)", &[]).await.unwrap();
        }
        replica.execute("INSERT INTO e_labels (name) VALUES ('replicated')", &[]).await.unwrap();
        let pool = DbPool::with_replicas(primary.clone(), vec![replica.clone()]).unwrap();
        assert!(matches!(pool, DbPool::Replicated(_)));
        assert_eq!(1, pool.replicas().len());

        // save writes to the primary and reads the row back from it.
        let saved = Label::repository(&pool).save(&Label { id: 0, name: "new".into() }).await.unwrap();
        assert_eq!("new", saved.name);
        assert_eq!(1, Label::repository(&primary).count().await.unwrap());
        // Reads see the replica.
        let names: Vec<String> = Label::query(&pool).fetch_all().await.unwrap().into_iter().map(|l| l.name).collect();
        assert_eq!(vec!["replicated"], names);
        assert_eq!(1, Label::repository(&pool).count().await.unwrap());
        assert_eq!("replicated", Label::repository(&pool).find_by_id(1).await.unwrap().unwrap().name);
        // …unless asked for the primary.
        assert_eq!("new", Label::query(pool.primary()).fetch_one().await.unwrap().unwrap().name);
        // Updates and deletes go to the primary.
        Label::query(&pool).where_eq("name", "new").update("name", "renamed").await.unwrap();
        assert_eq!("renamed", Label::query(&primary).fetch_one().await.unwrap().unwrap().name);
        assert_eq!("replicated", Label::query(&replica).fetch_one().await.unwrap().unwrap().name);
        // Transactions, reads in them included, are the primary's.
        let mut tx = pool.begin().await.unwrap();
        assert_eq!("renamed", tx.query::<Label>("SELECT * FROM e_labels", &[]).await.unwrap()[0].name);
        tx.commit().await.unwrap();
        // So are migrations and the schema.
        let dir = TempPath::new("model_replica_migrations");
        std::fs::create_dir_all(&*dir).unwrap();
        std::fs::write(dir.join("0001_t.sql"), "CREATE TABLE t (id INTEGER PRIMARY KEY)").unwrap();
        pool.migrate(&dir.to_string_lossy()).await.unwrap();
        assert!(primary.schema().await.unwrap().table("t").is_some());
        assert!(replica.schema().await.unwrap().table("t").is_none());
        assert!(pool.schema().await.unwrap().table("t").is_some());
        assert!(pool.migration_status(&dir.to_string_lossy()).await.unwrap()[0].applied);

        // Round robin over several replicas; nesting flattens.
        let third = DbPool::memory().await.unwrap();
        let wider = DbPool::with_replicas(pool.clone(), vec![third]).unwrap();
        assert_eq!(2, wider.replicas().len());
        assert!(matches!(wider.primary(), DbPool::Sqlite(_)));
        let picks: Vec<*const DbPool> = (0..4).map(|_| wider.replica() as *const DbPool).collect();
        assert_eq!(picks[0], picks[2]);
        assert_ne!(picks[0], picks[1]);
        // No replicas: everything on the primary; a plain pool is its own.
        let alone = DbPool::with_replicas(primary.clone(), Vec::new()).unwrap();
        assert_eq!(1, Label::query(&alone).count().await.unwrap());
        assert!(primary.replicas().is_empty());
        assert!(std::ptr::eq(&primary, primary.replica()) && std::ptr::eq(&primary, primary.primary()));
        assert!(format!("{wider:?}").contains("Replicas"));
        pool.close().await;
    }

    #[tokio::test]
    async fn a_replicated_pool_works_on_every_backend() {
        for db in dbs().await {
            let pool = DbPool::with_replicas(db.pool.clone(), vec![db.pool.clone()]).unwrap();
            assert_eq!(db.backend(), pool.backend());
            let ann = author(&pool, "ann").await;
            let p = post(&pool, "x", Some(ann.id), true).await;
            p.label_links().attach(&pool, [Label::factory().create(&pool).await.unwrap().id]).await.unwrap();
            let loaded = Post::query(&pool).with(Post::author()).with(Post::labels()).fetch_all().await.unwrap();
            assert_eq!((Some(ann.id), 1), (loaded[0].one(&Post::author()).map(|a| a.id), loaded[0].related(&Post::labels()).len()), "[{}]", db.name);
            assert!(pool.schema().await.unwrap().table("e_posts").is_some(), "[{}]", db.name);
            db.finish().await;
        }
    }

    // ── Schema ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn schema_lists_columns_keys_indexes_and_foreign_keys() {
        for db in dbs().await {
            let schema = db.pool.schema().await.unwrap_or_else(|e| panic!("[{}] {e}", db.name));
            assert_eq!(db.backend(), schema.backend);
            let names: Vec<&str> = schema.tables.iter().map(|t| t.name.as_str()).collect();
            assert_eq!(vec!["e_authors", "e_comments", "e_labels", "e_post_labels", "e_posts", "e_profiles"], names, "[{}]", db.name);

            let posts = schema.table("E_POSTS").unwrap();
            assert_eq!(vec!["id", "title", "author_id", "published"], posts.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>());
            assert_eq!(vec!["id".to_string()], posts.primary_key, "[{}]", db.name);
            assert!(posts.column("id").unwrap().primary_key && !posts.column("title").unwrap().primary_key);
            assert!(!posts.column("title").unwrap().nullable && posts.column("author_id").unwrap().nullable, "[{}]", db.name);
            assert!(posts.column("author_id").unwrap().data_type.to_ascii_lowercase().contains("int"), "[{}]", db.name);
            let by_title = posts.indexes.iter().find(|i| i.name == "e_posts_by_title").unwrap_or_else(|| panic!("[{}] {:?}", db.name, posts.indexes));
            assert_eq!((vec!["title".to_string(), "published".to_string()], false, false), (by_title.columns.clone(), by_title.unique, by_title.primary));
            let fk = posts.foreign_keys.first().unwrap_or_else(|| panic!("[{}] no foreign key", db.name));
            assert_eq!((vec!["author_id".to_string()], "e_authors", vec!["id".to_string()]), (fk.columns.clone(), fk.references_table.as_str(), fk.references_columns.clone()), "[{}]", db.name);
            assert_eq!("SET NULL", fk.on_delete, "[{}]", db.name);
            assert!(["NO ACTION", "RESTRICT"].contains(&fk.on_update.as_str()), "[{}] {}", db.name, fk.on_update);

            let authors = schema.table("e_authors").unwrap();
            assert!(authors.indexes.iter().any(|i| i.unique && !i.primary && i.columns == ["name"]), "[{}] {:?}", db.name, authors.indexes);
            let links = schema.table("e_post_labels").unwrap();
            assert_eq!(vec!["post_id".to_string(), "label_id".to_string()], links.primary_key, "[{}]", db.name);
            assert!(links.columns.iter().all(|c| c.primary_key));
            assert!(schema.table("nothing").is_none());

            let json = schema.to_json();
            assert!(json.starts_with(&format!(r#"{{"backend":"{}","tables":[{{"name":"e_authors","columns":[{{"name":"id","type":"#, db.backend().name())), "{json}");
            assert!(json.contains(r#""primary_key":["post_id","label_id"]"#), "[{}] {json}", db.name);
            assert!(json.contains(r#""references":{"table":"e_authors","columns":["id"]},"on_update":"#), "[{}] {json}", db.name);
            assert!(json.contains(r#"{"name":"e_posts_by_title","columns":["title","published"],"unique":false,"primary":false}"#), "[{}] {json}", db.name);
            db.finish().await;
        }
    }
}
