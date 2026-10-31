use super::*;
use crate::authz::{Authorizer, Schema};

fn memory_store() -> SqlStore {
    SqlStore::new(run(async { DbPool::memory().await }).unwrap().unwrap()).unwrap()
}

fn file_config(path: &str) -> DbConfig {
    DbConfig {
        host: String::new(),
        port: 0,
        user: String::new(),
        password: String::new(),
        database: format!("{path}?mode=rwc"),
        pool_size: 2,
        backend: Backend::Sqlite,
    }
}

fn tuple(text: &str) -> Tuple {
    Tuple::parse(text).unwrap()
}

#[test]
fn tuples_are_written_read_listed_and_deleted() {
    let store = memory_store();
    assert!(store.write(&tuple("doc:1#owner@user:a")).unwrap());
    assert!(!store.write(&tuple("doc:1#owner@user:a")).unwrap(), "already there");
    assert!(store.write(&tuple("doc:1#viewer@group:eng#member")).unwrap());
    assert!(store.write(&tuple("doc:2#owner@user:a")).unwrap());
    let object = ObjectRef::parse("doc:1").unwrap();
    assert_eq!(store.read(&object, "owner").unwrap(), [Subject::parse("user:a").unwrap()]);
    assert_eq!(store.read(&object, "viewer").unwrap(), [Subject::parse("group:eng#member").unwrap()]);
    let by_a = TupleFilter { subject: Some(Subject::parse("user:a").unwrap()), ..Default::default() };
    assert_eq!(store.list(&by_a, 100).unwrap().iter().map(ToString::to_string).collect::<Vec<_>>(), ["doc:1#owner@user:a", "doc:2#owner@user:a"]);
    assert_eq!(store.list(&TupleFilter::default(), 2).unwrap().len(), 2);
    assert!(store.delete(&tuple("doc:1#owner@user:a")).unwrap());
    assert!(store.read(&object, "owner").unwrap().is_empty(), "a write clears the read cache");
    assert!(!store.delete(&tuple("doc:1#owner@user:a")).unwrap());
}

#[test]
fn checks_run_against_the_database_and_tuples_outlive_the_store() {
    let path = crate::test_env::TempPath::new("authz_sql_tuples.db");
    let schema = || {
        Schema::new()
            .relation("group", "member", "user | group#member").unwrap()
            .relation("doc", "editor", "user | group#member").unwrap()
            .relation("doc", "viewer", "user | editor").unwrap()
    };
    {
        let authz = Authorizer::new(schema(), SqlStore::open(file_config(path.as_str())).unwrap()).unwrap();
        for t in ["group:eng#member@user:gina", "doc:42#editor@group:eng#member"] {
            authz.write_str(t).unwrap();
        }
        assert!(authz.check("user:gina", "viewer", "doc:42").unwrap());
    }
    // Opened again (as after a restart): the table exists, the tuples too.
    let authz = Authorizer::new(schema(), SqlStore::open(file_config(path.as_str())).unwrap()).unwrap();
    assert!(authz.check("user:gina", "viewer", "doc:42").unwrap());
    assert!(!authz.check("user:eve", "viewer", "doc:42").unwrap());
}

#[test]
fn reads_are_cached_briefly() {
    let store = memory_store();
    store.write(&tuple("doc:1#owner@user:a")).unwrap();
    let object = ObjectRef::parse("doc:1").unwrap();
    assert_eq!(store.read(&object, "owner").unwrap().len(), 1);
    // Behind the store's back (another process): seen only after READ_CACHE.
    let pool = store.pool.clone();
    run(async move { pool.execute("DELETE FROM rws_authz_tuples", &[]).await }).unwrap().unwrap();
    assert_eq!(store.read(&object, "owner").unwrap().len(), 1, "cached");
    std::thread::sleep(READ_CACHE + Duration::from_millis(50));
    assert!(store.read(&object, "owner").unwrap().is_empty());
}

#[test]
fn the_config_file_can_keep_tuples_in_sql() {
    let _g = crate::test_env::lock();
    let path = crate::test_env::TempPath::new("authz_sql_config.db");
    std::env::set_var("RWS_DB_BACKEND", "sqlite");
    std::env::set_var("RWS_DB_NAME", format!("{}?mode=rwc", path.as_str()));
    let toml = "[authz]\nstore = \"sql\"\ntuples = [\"doc:1#owner@user:a\"]\n[authz.type.doc]\nowner = \"user\"\n\n[[route]]\n[route.match]\npath = \"/\"\n[route.action]\ntype = \"respond\"\nbody = \"x\"\n";
    let report = crate::config_check::check_str_offline(toml);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    let authorizer = crate::proxy_config::builder::config_authorizer(&crate::proxy_config::ProxyConfig::from_str(toml)).unwrap().unwrap();
    assert!(authorizer.check("user:a", "owner", "doc:1").unwrap());
    std::env::remove_var("RWS_DB_BACKEND");
    std::env::remove_var("RWS_DB_NAME");
}
