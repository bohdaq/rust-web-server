//! Tests of rws-server's `authz::sql` that need the facade (proxy routes,
//! `rws --check`, the admin API, ACME, the `routes!` macro).
#![allow(unused_imports, dead_code)]

use crate::authz::sql::*;
use crate::authz::Tuple;
use crate::model::backend::Backend;
use crate::model::{DbConfig, DbPool};
use crate::authz::{Authorizer, Schema};
use crate::application::Application;
use crate::request::Request;
use crate::server::ConnectionInfo;

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
