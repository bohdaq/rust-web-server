//! `DbTls` / `DbSslMode` and the connect options built from them.

use super::*;

#[test]
fn ssl_modes_parse_in_libpq_and_mysql_spellings() {
    for (text, mode) in [
        ("", DbSslMode::Prefer),
        ("disable", DbSslMode::Disable),
        ("DISABLED", DbSslMode::Disable),
        ("prefer", DbSslMode::Prefer),
        ("required", DbSslMode::Require),
        ("verify-ca", DbSslMode::VerifyCa),
        ("verify_ca", DbSslMode::VerifyCa),
        ("verify-full", DbSslMode::VerifyFull),
        ("VERIFY_IDENTITY", DbSslMode::VerifyFull),
    ] {
        assert_eq!(mode, DbSslMode::parse(text).unwrap(), "{text:?}");
    }
    assert!(DbSslMode::parse("on").unwrap_err().to_string().contains("verify-full"));
}

#[test]
fn tls_from_env() {
    let _g = crate::test_env::lock();
    let names = ["RWS_DB_SSL_MODE", "RWS_DB_SSL_ROOT_CERT", "RWS_DB_SSL_CLIENT_CERT", "RWS_DB_SSL_CLIENT_KEY"];
    for n in names {
        std::env::remove_var(n);
    }
    assert_eq!(DbTls::default(), DbTls::from_env().unwrap());

    std::env::set_var("RWS_DB_SSL_MODE", "verify-full");
    std::env::set_var("RWS_DB_SSL_ROOT_CERT", "/etc/db/ca.crt");
    let tls = DbTls::from_env().unwrap();
    assert_eq!(DbSslMode::VerifyFull, tls.mode);
    assert_eq!(Some("/etc/db/ca.crt"), tls.root_cert.as_deref());

    std::env::set_var("RWS_DB_SSL_CLIENT_CERT", "/etc/db/client.crt");
    assert!(DbTls::from_env().is_err(), "a client certificate needs its key");
    std::env::set_var("RWS_DB_SSL_CLIENT_KEY", "/etc/db/client.key");
    assert_eq!(Some("/etc/db/client.key"), DbTls::from_env().unwrap().client_key.as_deref());

    std::env::set_var("RWS_DB_SSL_MODE", "sometimes");
    assert!(DbTls::from_env().is_err());
    for n in names {
        std::env::remove_var(n);
    }
}

#[cfg(feature = "model-postgres")]
fn server() -> DbConfig {
    DbConfig {
        host: "db.internal".into(),
        port: 5433,
        user: "app".into(),
        // Characters a URL would need escaped: passed as they are.
        password: "p@ss:w/rd?#".into(),
        database: "shop".into(),
        pool_size: 2,
        backend: Backend::Postgres,
    }
}

#[cfg(feature = "model-postgres")]
#[test]
fn postgres_options_carry_the_tls_mode() {
    use sqlx::postgres::PgSslMode;
    let options = pool::pg_options(&server(), &DbTls::default());
    assert_eq!(("db.internal", 5433, "app", Some("shop")), (options.get_host(), options.get_port(), options.get_username(), options.get_database()));
    assert!(matches!(options.get_ssl_mode(), PgSslMode::Prefer));
    let tls = DbTls { mode: DbSslMode::VerifyFull, root_cert: Some("/etc/db/ca.crt".into()), ..DbTls::default() };
    assert!(matches!(pool::pg_options(&server(), &tls).get_ssl_mode(), PgSslMode::VerifyFull));
}

/// Against a real PostgreSQL with TLS from a private CA, when
/// `RWS_DB_TEST_TLS_CA` (that CA), `RWS_DB_TEST_TLS_OTHER_CA` (another one)
/// and `RWS_DB_TEST_TLS_PORT` are set; the server is `localhost`, user
/// `postgres`, password `p@ss:w/rd?#`, database `shop`.
/// `cargo test -p rws-server --features "test-support model-postgres model-sqlite" -- --ignored live_postgres_tls`
#[cfg(feature = "model-postgres")]
#[tokio::test]
#[ignore]
async fn live_postgres_tls() {
    let (Ok(ca), Ok(other), Ok(port)) = (std::env::var("RWS_DB_TEST_TLS_CA"), std::env::var("RWS_DB_TEST_TLS_OTHER_CA"), std::env::var("RWS_DB_TEST_TLS_PORT")) else {
        panic!("set RWS_DB_TEST_TLS_CA, RWS_DB_TEST_TLS_OTHER_CA and RWS_DB_TEST_TLS_PORT");
    };
    let config = DbConfig { host: "localhost".into(), port: port.parse().unwrap(), user: "postgres".into(), database: "shop".into(), ..server() };
    let tls = |mode, root: &str| DbTls { mode, root_cert: Some(root.to_string()), ..DbTls::default() };
    let pool = DbPool::new_with_tls(config.clone(), tls(DbSslMode::VerifyFull, &ca)).await.expect("verify-full with the right CA");
    let rows = pool.query_rows("SELECT ssl::text AS ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()", &[]).await.unwrap();
    assert_eq!("true", rows[0].get::<String>("ssl").unwrap());
    assert!(DbPool::new_with_tls(config.clone(), tls(DbSslMode::VerifyCa, &other)).await.is_err(), "another CA must be refused");
    let wrong_host = DbConfig { host: "127.0.0.1".into(), ..config.clone() };
    assert!(DbPool::new_with_tls(wrong_host.clone(), tls(DbSslMode::VerifyCa, &ca)).await.is_ok(), "verify-ca doesn't check the name");
    let no_ip_name = DbConfig { host: "127.0.0.2".into(), ..config.clone() };
    assert!(DbPool::new_with_tls(no_ip_name, tls(DbSslMode::VerifyFull, &ca)).await.is_err(), "verify-full checks the name");
    // The default (prefer) still connects, as before.
    DbPool::new(config).await.expect("prefer");
}
