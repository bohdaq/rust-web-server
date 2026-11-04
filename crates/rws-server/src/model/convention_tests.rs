//! 18.30.0: keys beyond `i64`, `#[timestamps]`, `#[soft_delete]`,
//! `#[version]`, and the pieces under them (`Timestamp`, `Uuid`, the wire
//! decoders, `DbError::kind`) — on every backend `backends.rs` provides.

use crate::model::decode::{mysql_temporal, pg_value};
use crate::model::{DbError, DbErrorKind, FromColumn, Timestamp, ToColumn, Uuid, Value};

// ── Timestamp ─────────────────────────────────────────────────────────────────

#[test]
fn timestamps_parse_every_form_the_backends_write() {
    let t = Timestamp::from_unix_micros(1_793_795_696_500_000);
    for text in [
        "2026-11-04T12:34:56.5Z",
        "2026-11-04 12:34:56.500000",
        "2026-11-04 12:34:56.5+00",
        "2026-11-04 12:34:56.5+00:00",
        "2026-11-04t12:34:56,5z",
        "2026-11-04T13:34:56.5+01:00",
        "2026-11-04T11:34:56.5-0100",
        "2026-11-04T12:34:56.500000999Z",
    ] {
        assert_eq!(Ok(t), Timestamp::parse(text).map_err(|e| e.0), "{text}");
    }
    assert_eq!("2026-11-04T12:34:56.500000Z", t.to_string());
    assert_eq!(Timestamp::from_unix_seconds(1_793_750_400), Timestamp::parse("2026-11-04").unwrap());
    assert_eq!(Timestamp::from_unix_seconds(0), Timestamp::parse("1970-01-01T00:00Z").unwrap());
    assert_eq!("1969-12-31T23:59:59.999999Z", Timestamp::from_unix_micros(-1).to_rfc3339());
    assert_eq!("2000-02-29T00:00:00.000000Z", Timestamp::parse("2000-02-29").unwrap().to_rfc3339());
    assert_eq!(1_793_795_696, t.unix_seconds());
}

#[test]
fn timestamps_reject_what_isnt_one() {
    for bad in ["", "2026-13-01", "2026-02-30", "2026-11-04T25:00", "2026-11-04X12:00", "2026-11-04T12:00:00.", "2026-11-04T12:00:00Q", "yesterday", "2026-11-04T12:00+24:00"] {
        let err = Timestamp::parse(bad).expect_err(bad);
        assert!(err.0.contains("timestamp"), "{bad}: {}", err.0);
    }
    assert!(Timestamp::from_column(Value::Null).is_err());
    assert!(Timestamp::from_column(Value::Bool(true)).is_err());
    assert_eq!(Timestamp::from_unix_seconds(5), Timestamp::from_column(Value::Int(5)).unwrap());
}

#[test]
fn timestamps_bind_per_backend() {
    let t = Timestamp::from_unix_micros(1_793_795_696_000_001);
    assert_eq!(Value::Text("2026-11-04T12:34:56.000001Z".into()), t.to_column());
    #[cfg(feature = "model-sqlite")]
    assert_eq!(Value::Text("2026-11-04T12:34:56.000001Z".into()), t.to_value(crate::model::Backend::Sqlite));
    #[cfg(feature = "model-mysql")]
    assert_eq!(Value::Text("2026-11-04 12:34:56.000001".into()), t.to_value(crate::model::Backend::MySql));
    let now = Timestamp::now();
    assert!(now > t || now.unix_seconds() < 1_793_795_696, "the clock moves on");
}

// ── Uuid ──────────────────────────────────────────────────────────────────────

#[test]
fn uuids_parse_format_and_generate() {
    let text = "67e55044-10b1-426f-9247-bb680e5fe0c8";
    let id = Uuid::parse(text).unwrap();
    assert_eq!(text, id.to_string());
    assert_eq!(id, Uuid::parse("67E5504410B1426F9247BB680E5FE0C8").unwrap());
    assert_eq!(4, id.version());
    for bad in ["", "67e55044-10b1-426f-9247-bb680e5fe0c", "67e55044_10b1_426f_9247_bb680e5fe0c8", "zze55044-10b1-426f-9247-bb680e5fe0c8"] {
        assert!(Uuid::parse(bad).is_err(), "{bad:?}");
    }
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    assert_ne!(a, b);
    assert_eq!((4, 0x80), (a.version(), a.as_bytes()[8] & 0xc0));
    let v7 = Uuid::now_v7();
    assert_eq!((7, 0x80), (v7.version(), v7.as_bytes()[8] & 0xc0));
    let millis = u64::from_be_bytes([0, 0, v7.as_bytes()[0], v7.as_bytes()[1], v7.as_bytes()[2], v7.as_bytes()[3], v7.as_bytes()[4], v7.as_bytes()[5]]);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
    assert!(now.abs_diff(millis) < 60_000, "v7 carries the time");
    assert!(Uuid::default().is_nil() && Uuid::NIL.to_string() == "00000000-0000-0000-0000-000000000000");
    assert_eq!(id, Uuid::from_column(id.to_column()).unwrap());
    assert_eq!(id, Uuid::from_column(Value::Bytes(id.as_bytes().to_vec())).unwrap());
    assert!(Uuid::from_column(Value::Int(1)).is_err());
    assert_eq!(format!("Uuid({text})"), format!("{id:?}"));
}

// ── Wire decoders ─────────────────────────────────────────────────────────────

#[test]
fn postgres_temporal_and_uuid_values_decode_from_binary_and_text() {
    // 2000-01-01T00:00:00.000001Z: one microsecond after PostgreSQL's epoch.
    assert_eq!(Some(Value::Text("2000-01-01T00:00:00.000001Z".into())), pg_value("TIMESTAMPTZ", true, &1i64.to_be_bytes()));
    let micros = (1_793_795_696_500_000i64 - 946_684_800_000_000).to_be_bytes();
    assert_eq!(Some(Value::Text("2026-11-04T12:34:56.500000Z".into())), pg_value("TIMESTAMP", true, &micros));
    assert_eq!(Some(Value::Text("infinity".into())), pg_value("TIMESTAMPTZ", true, &i64::MAX.to_be_bytes()));
    assert_eq!(Some(Value::Text("1999-12-31".into())), pg_value("DATE", true, &(-1i32).to_be_bytes()));
    let id = Uuid::parse("67e55044-10b1-426f-9247-bb680e5fe0c8").unwrap();
    assert_eq!(Some(Value::Text(id.to_string())), pg_value("UUID", true, id.as_bytes()));
    assert_eq!(None, pg_value("UUID", true, &[1, 2, 3]), "the wrong length isn't a value");
    assert_eq!(None, pg_value("TIMESTAMPTZ", true, &[0; 4]));
    assert_eq!(Some(Value::Text("2026-11-04T12:34:56.500000Z".into())), pg_value("TIMESTAMPTZ", false, b"2026-11-04 12:34:56.5+00"));
    assert_eq!(Some(Value::Text("2026-11-04".into())), pg_value("DATE", false, b"2026-11-04"));
}

#[test]
fn mysql_temporal_values_decode_from_binary_and_text() {
    // len 11: 2026-11-04 12:34:56.500000
    let mut full = vec![11u8];
    full.extend_from_slice(&2026u16.to_le_bytes());
    full.extend_from_slice(&[11, 4, 12, 34, 56]);
    full.extend_from_slice(&500_000u32.to_le_bytes());
    assert_eq!(Some(Value::Text("2026-11-04T12:34:56.500000Z".into())), mysql_temporal("DATETIME", &full));
    let mut seconds = vec![7u8];
    seconds.extend_from_slice(&2026u16.to_le_bytes());
    seconds.extend_from_slice(&[11, 4, 12, 34, 56]);
    assert_eq!(Some(Value::Text("2026-11-04T12:34:56.000000Z".into())), mysql_temporal("TIMESTAMP", &seconds));
    let mut date = vec![4u8];
    date.extend_from_slice(&2026u16.to_le_bytes());
    date.extend_from_slice(&[11, 4]);
    assert_eq!(Some(Value::Text("2026-11-04".into())), mysql_temporal("DATE", &date));
    assert_eq!(Some(Value::Text("2026-11-04T00:00:00.000000Z".into())), mysql_temporal("DATETIME", &date));
    assert_eq!(Some(Value::Text("0000-00-00 00:00:00".into())), mysql_temporal("DATETIME", &[0]));
    assert_eq!(Some(Value::Text("2026-11-04T12:34:56.000000Z".into())), mysql_temporal("DATETIME", b"2026-11-04 12:34:56"));
    assert_eq!(Some(Value::Text("not a date".into())), mysql_temporal("DATETIME", b"not a date"));
}

// ── DbError ───────────────────────────────────────────────────────────────────

#[test]
fn db_errors_say_what_they_are_about() {
    assert_eq!(DbErrorKind::StaleVersion, DbError::stale_version("x").kind());
    assert!(DbError::stale_version("x").is_stale_version());
    assert!(DbError::not_found("users id=1").is_not_found());
    assert_eq!(DbErrorKind::Other, DbError::new("syntax error near stale version: ").kind());
    assert_eq!("DbError: not found: users id=1", DbError::not_found("users id=1").to_string());
}

// ── On every backend ─────────────────────────────────────────────────────────

#[cfg(feature = "macros")]
mod on_backends {
    use crate::model::backends::{self, TestBackend};
    use crate::model::{HasMany, Model, ModelRepository, QueryBuilder, Repository, Timestamp, Uuid, Value};

    #[derive(rws_macros::Model, Debug, Clone, PartialEq)]
    #[table(name = "articles")]
    #[timestamps]
    #[soft_delete]
    struct Article {
        #[primary_key(auto_increment)]
        id: i64,
        title: String,
        author_id: i64,
        #[version]
        lock_version: i64,
        created_at: Timestamp,
        updated_at: Timestamp,
        deleted_at: Option<Timestamp>,
    }

    #[derive(rws_macros::Model, Debug, Clone, PartialEq)]
    #[table(name = "tags")]
    struct Tag {
        #[primary_key]
        slug: String,
        label: String,
    }

    #[derive(rws_macros::Model, Debug, Clone, PartialEq)]
    #[table(name = "tokens")]
    struct Token {
        #[primary_key]
        id: Uuid,
        owner: String,
    }

    #[derive(rws_macros::Model, Debug, Clone, PartialEq)]
    #[table(name = "counters")]
    struct Counter {
        #[primary_key(auto_increment)]
        id: i32,
        n: i32,
    }

    const SCHEMA: &str = "\
        CREATE TABLE articles (id {pk}, title {text} NOT NULL, author_id BIGINT NOT NULL, lock_version BIGINT NOT NULL, \
          created_at {ts} NOT NULL, updated_at {ts} NOT NULL, deleted_at {ts}){engine};\
        CREATE TABLE tags (slug {text} PRIMARY KEY, label {text} NOT NULL){engine};\
        CREATE TABLE tokens (id {uuid} PRIMARY KEY, owner {text} NOT NULL){engine};\
        CREATE TABLE counters (id {pk32}, n INTEGER NOT NULL){engine}";

    async fn dbs() -> Vec<TestBackend> {
        let mut out = Vec::new();
        for db in backends::all().await {
            db.create(SCHEMA).await;
            out.push(db);
        }
        out
    }

    fn article(title: &str) -> Article {
        Article {
            id: 0,
            title: title.into(),
            author_id: 7,
            lock_version: 0,
            created_at: Timestamp::default(),
            updated_at: Timestamp::default(),
            deleted_at: None,
        }
    }

    async fn pause() {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }

    #[test]
    fn the_derive_names_the_convention_columns() {
        assert_eq!(Some("created_at"), Article::created_at_column());
        assert_eq!(Some("updated_at"), Article::updated_at_column());
        assert_eq!(Some("deleted_at"), Article::soft_delete_column());
        assert_eq!(Some("lock_version"), Article::version_column());
        assert_eq!((None, None, None), (Tag::created_at_column(), Tag::soft_delete_column(), Tag::version_column()));
        // repository() is keyed by the key's type (i64 for any integer).
        let _: fn(&crate::model::DbPool) -> ModelRepository<'_, Tag, String> = Tag::repository;
        let _: fn(&crate::model::DbPool) -> ModelRepository<'_, Token, Uuid> = Token::repository;
        let _: fn(&crate::model::DbPool) -> ModelRepository<'_, Counter, i64> = Counter::repository;
    }

    #[tokio::test]
    async fn save_sets_the_timestamps_and_keeps_created_at() {
        for db in dbs().await {
            let repo = Article::repository(&db.pool);
            let before = Timestamp::now();
            let mut draft = article("Hello");
            draft.created_at = Timestamp::from_unix_seconds(1); // ignored: the repository writes it
            let saved = repo.save(&draft).await.unwrap_or_else(|e| panic!("[{}] insert: {e}", db.name));
            assert_eq!(saved.created_at, saved.updated_at, "[{}]", db.name);
            assert!(saved.created_at >= before && saved.created_at <= Timestamp::now(), "[{}] {} not now", db.name, saved.created_at);
            assert_eq!((1, None), (saved.lock_version, saved.deleted_at), "[{}]", db.name);

            pause().await;
            let mut edited = saved.clone();
            edited.title = "Hello again".into();
            edited.created_at = Timestamp::from_unix_seconds(2);
            let updated = repo.save(&edited).await.unwrap();
            assert_eq!(saved.created_at, updated.created_at, "[{}] created_at is never written again", db.name);
            assert!(updated.updated_at > saved.updated_at, "[{}] updated_at moves", db.name);
            assert_eq!(("Hello again", 2), (updated.title.as_str(), updated.lock_version));

            // A bulk update sets updated_at and the version too.
            pause().await;
            QueryBuilder::<Article>::new(&db.pool).where_eq("id", saved.id).update("title", "Bulk").await.unwrap();
            let bulk = repo.find_by_id(saved.id).await.unwrap().unwrap();
            assert_eq!(("Bulk", 3), (bulk.title.as_str(), bulk.lock_version), "[{}]", db.name);
            assert!(bulk.updated_at > updated.updated_at);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn saving_a_stale_copy_fails_and_changes_nothing() {
        for db in dbs().await {
            let repo = Article::repository(&db.pool);
            let saved = repo.save(&article("v1")).await.unwrap();
            let (mut mine, mut theirs) = (saved.clone(), saved.clone());
            theirs.title = "theirs".into();
            let theirs = repo.save(&theirs).await.unwrap();
            assert_eq!(2, theirs.lock_version);

            mine.title = "mine".into();
            let err = repo.save(&mine).await.expect_err("stale");
            assert!(err.is_stale_version(), "[{}] {err}", db.name);
            assert!(err.0.contains("lock_version 2") && err.0.contains("this copy has 1"), "[{}] {err}", db.name);
            let row = repo.find_by_id(saved.id).await.unwrap().unwrap();
            assert_eq!(("theirs", 2), (row.title.as_str(), row.lock_version), "[{}] nothing overwritten", db.name);

            // Reload, apply again: saved.
            let mut fresh = row.clone();
            fresh.title = "mine".into();
            assert_eq!(3, repo.save(&fresh).await.unwrap().lock_version);

            // A row deleted for real: not found, not stale.
            repo.force_delete_by_id(saved.id).await.unwrap();
            assert!(repo.save(&fresh).await.unwrap_err().is_not_found(), "[{}]", db.name);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn soft_deleted_rows_are_hidden_until_restored() {
        for db in dbs().await {
            let repo = Article::repository(&db.pool);
            let kept = repo.save(&article("kept")).await.unwrap();
            let gone = repo.save(&article("gone")).await.unwrap();
            repo.delete_by_id(gone.id).await.unwrap();

            assert_eq!(None, repo.find_by_id(gone.id).await.unwrap(), "[{}]", db.name);
            assert!(!repo.exists_by_id(gone.id).await.unwrap());
            assert_eq!(1, repo.count().await.unwrap(), "[{}]", db.name);
            assert_eq!(vec![kept.id], repo.find_all().await.unwrap().iter().map(|a| a.id).collect::<Vec<_>>());
            let trashed = repo.find_by_id_with_deleted(gone.id).await.unwrap().expect("still in the table");
            let deleted_at = trashed.deleted_at.expect("deleted_at set");
            assert!(deleted_at >= gone.created_at, "[{}]", db.name);

            let q = || QueryBuilder::<Article>::new(&db.pool);
            assert_eq!(1, q().count().await.unwrap());
            assert_eq!(2, q().with_deleted().count().await.unwrap(), "[{}]", db.name);
            assert_eq!(vec!["gone"], q().only_deleted().fetch_all().await.unwrap().iter().map(|a| a.title.as_str()).collect::<Vec<_>>());
            let posts = HasMany::<Article>::new(Value::Int(7), "author_id").load(&db.pool).await.unwrap();
            assert_eq!(1, posts.len(), "[{}] relations skip deleted rows", db.name);

            // Deleting again keeps the first deletion time.
            pause().await;
            repo.delete_by_id(gone.id).await.unwrap();
            assert_eq!(Some(deleted_at), repo.find_by_id_with_deleted(gone.id).await.unwrap().unwrap().deleted_at, "[{}]", db.name);

            assert!(repo.restore(gone.id).await.unwrap(), "[{}]", db.name);
            assert!(!repo.restore(gone.id).await.unwrap(), "not deleted any more");
            assert_eq!(2, repo.count().await.unwrap());

            // The query builder: delete is soft, force_delete is not.
            q().where_eq("title", "kept").delete().await.unwrap();
            assert_eq!(1, q().count().await.unwrap(), "[{}]", db.name);
            assert_eq!(1, q().restore().await.unwrap());
            assert_eq!(0, q().where_eq("title", "nothing").force_delete().await.unwrap());
            assert_eq!(1, q().where_eq("title", "kept").force_delete().await.unwrap(), "[{}]", db.name);
            repo.force_delete_by_id(gone.id).await.unwrap();
            assert_eq!(0, q().with_deleted().count().await.unwrap(), "[{}]", db.name);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn restore_needs_a_soft_delete_column() {
        for db in dbs().await {
            let err = Tag::repository(&db.pool).restore("x".to_string()).await.expect_err("no #[soft_delete]");
            assert!(err.0.contains("#[soft_delete]"), "{err}");
            assert!(QueryBuilder::<Tag>::new(&db.pool).restore().await.is_err());
            assert_eq!(0, QueryBuilder::<Tag>::new(&db.pool).only_deleted().count().await.unwrap(), "[{}]", db.name);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn string_keys_insert_when_new_and_update_when_there() {
        for db in dbs().await {
            let repo = Tag::repository(&db.pool);
            let rust = repo.save(&Tag { slug: "rust".into(), label: "Rust".into() }).await.unwrap_or_else(|e| panic!("[{}] {e}", db.name));
            assert_eq!("rust", rust.slug);
            let renamed = repo.save(&Tag { slug: "rust".into(), label: "Rust lang".into() }).await.unwrap();
            assert_eq!("Rust lang", renamed.label);
            assert_eq!(1, repo.count().await.unwrap(), "[{}] updated, not inserted again", db.name);
            assert_eq!(Some(renamed.clone()), repo.find_by_id("rust".to_string()).await.unwrap());
            assert!(repo.exists_by_id("rust".to_string()).await.unwrap());
            assert!(!repo.exists_by_id("go".to_string()).await.unwrap());

            let err = repo.save(&Tag { slug: String::new(), label: "?".into() }).await.expect_err("empty key");
            assert!(err.0.contains("empty"), "{err}");
            assert!(repo.insert(&renamed).await.is_err(), "[{}] insert never updates", db.name);

            repo.save_all(&[Tag { slug: "go".into(), label: "Go".into() }, Tag { slug: "zig".into(), label: "Zig".into() }]).await.unwrap();
            repo.delete_all_by_id(&["go".to_string(), "rust".to_string()]).await.unwrap();
            assert_eq!(vec!["zig"], repo.find_all().await.unwrap().iter().map(|t| t.slug.as_str()).collect::<Vec<_>>(), "[{}]", db.name);
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn uuid_keys_are_generated_for_nil_and_kept_otherwise() {
        for db in dbs().await {
            let repo = Token::repository(&db.pool);
            let generated = repo.save(&Token { id: Uuid::NIL, owner: "ann".into() }).await.unwrap_or_else(|e| panic!("[{}] {e}", db.name));
            assert_eq!(7, generated.id.version(), "[{}]", db.name);
            let chosen = Uuid::new_v4();
            let kept = repo.save(&Token { id: chosen, owner: "bo".into() }).await.unwrap();
            assert_eq!(chosen, kept.id);
            let moved = repo.save(&Token { id: chosen, owner: "cy".into() }).await.unwrap();
            assert_eq!("cy", moved.owner);
            assert_eq!(2, repo.count().await.unwrap(), "[{}]", db.name);
            assert_eq!(Some("ann".to_string()), repo.find_by_id(generated.id).await.unwrap().map(|t| t.owner));
            repo.delete_by_id(generated.id).await.unwrap();
            assert_eq!(None, repo.find_by_id(generated.id).await.unwrap());
            db.finish().await;
        }
    }

    #[tokio::test]
    async fn i32_keys_work_like_i64_ones() {
        for db in dbs().await {
            let repo: ModelRepository<Counter, i32> = ModelRepository::new(&db.pool);
            let one = repo.save(&Counter { id: 0, n: 1 }).await.unwrap_or_else(|e| panic!("[{}] {e}", db.name));
            assert!(one.id > 0, "[{}]", db.name);
            let bumped = repo.save(&Counter { n: 2, ..one.clone() }).await.unwrap();
            assert_eq!((one.id, 2), (bumped.id, bumped.n));
            assert_eq!(Some(bumped), repo.find_by_id(one.id).await.unwrap());
            repo.delete_by_id(one.id).await.unwrap();
            assert_eq!(0, repo.count().await.unwrap(), "[{}]", db.name);
            db.finish().await;
        }
    }
}
