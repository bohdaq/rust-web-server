//! rws-e2e-app: exercises rust-web-server's library-only features for the
//! stage end-to-end suite (`ci/stage/`). See README.md for the contract.

mod app;
mod common;
mod db;
mod llm;
mod s3;
mod sso;

use rust_web_server::server::Server;

use crate::common::{env, Root};

fn build(role: &str) -> Result<Root, String> {
    match role {
        "app" => {
            let migrations = env("MIGRATIONS_DIR").unwrap_or_else(|| "migrations".to_string());
            Ok(app::build(app::AppConfig::from_env(migrations)))
        }
        "idp" => Ok(match sso::IdpConfig::from_env() {
            Ok(c) => sso::idp(c),
            Err(missing) => sso::unconfigured("idp", format!("{missing} unset")),
        }),
        "sp" => Ok(match sso::SpConfig::from_env() {
            Ok(c) => sso::sp(c),
            Err(missing) => sso::unconfigured("sp", format!("{missing} unset")),
        }),
        other => Err(format!("unknown ROLE={other:?} (app, idp or sp)")),
    }
}

fn main() {
    // Plain HTTP on 0.0.0.0:8080 unless told otherwise (a gateway terminates TLS).
    for (name, value) in [("RWS_CONFIG_IP", "0.0.0.0"), ("RWS_CONFIG_PORT", "8080")] {
        if std::env::var(name).is_err() {
            std::env::set_var(name, value);
        }
    }
    let role = env("ROLE").unwrap_or_else(|| "app".to_string());
    eprintln!("[e2e] rws-e2e-app {} ROLE={role}", env!("CARGO_PKG_VERSION"));
    let app = match build(&role) {
        Ok(app) => app,
        Err(e) => {
            eprintln!("[e2e] {e}");
            std::process::exit(2);
        }
    };
    let (listener, pool) = match Server::setup() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[e2e] cannot listen: {e}");
            std::process::exit(1);
        }
    };
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("tokio runtime");
    rt.block_on(Server::run_tls(listener, pool, app));
}
