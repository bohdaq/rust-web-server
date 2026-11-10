//! A full-stack app on rust-web-server: a JSON API under `/api` (typed
//! handlers), accounts under `/auth` (sign-up, login, password reset,
//! email confirmation; src/accounts.rs), the lit frontend (built into
//! `frontend/dist`) served as static files, and PostgreSQL when `PGHOST` is
//! set — an in-memory store otherwise, so `cargo run` and `cargo test` need
//! no database.
//!
//! The conventional rws layout (`rws.app.toml`): this file builds the app
//! (`app()`), `src/routes/` holds the handlers, `src/models/` the
//! `#[derive(Model)]` types, `migrations/` the SQL, `src/main.rs` only
//! serves it, `tests/` tests it. See AGENTS.md.

pub mod accounts;
pub mod models;
pub mod routes;
pub mod store;

use rust_web_server::async_state::AsyncAppWithState;
use rust_web_server::auth_flows::CurrentUser;
use rust_web_server::dev_mcp::DevTools;
use rust_web_server::model_admin::ModelAdmin;
use rust_web_server::entry_point::RwsEnv;
use rust_web_server::error_pages::ErrorPages;
use rust_web_server::flash::FlashLayer;
use rust_web_server::middleware::WithMiddleware;
use rust_web_server::prelude::*;

pub use accounts::Accounts;
pub use models::Note;
pub use store::Store;

/// The app: every route. Requests no route matches fall through to static
/// files from the working directory (see `main`).
pub fn app(store: Store) -> AsyncAppWithState<Store> {
    let app = App::with_async_state(store);
    let app = routes::health::register(app);
    let app = routes::account::register(app);
    routes::notes::register(app)
}

/// What `main` serves: `app()` behind the account flows (`/auth`), flash
/// messages, error pages (details with `RWS_ENV=dev` or `test`), with
/// PostgreSQL the model admin at `/admin` for the accounts in
/// `ADMIN_EMAILS` (comma-separated) and, in development, the mail
/// previews at `/_mail`.
pub fn site(store: Store, accounts: &Accounts) -> WithMiddleware<AsyncAppWithState<Store>> {
    let admin = store.pool().cloned().map(|pool| {
        let admins: Vec<String> = std::env::var("ADMIN_EMAILS").unwrap_or_default().split(',').map(|e| e.trim().to_lowercase()).filter(|e| !e.is_empty()).collect();
        ModelAdmin::new(pool)
            .title("{{project-name}} admin")
            .model::<Note>()
            .table("rws_users")
            .allow(move |request| CurrentUser::of(request).is_some_and(|u| admins.contains(&u.email)))
    });
    // AuthFlows first (outermost): the admin's check reads who's signed in.
    let site = WithMiddleware::new(app(store)).wrap(accounts.auth.clone());
    let site = match admin {
        Some(admin) => site.wrap(admin),
        None => site,
    };
    let site = site.wrap(FlashLayer::new());
    let site = if RwsEnv::current().is_dev() { site.wrap(Accounts::previews()) } else { site };
    site.wrap(ErrorPages::from_env())
}

/// What the dev MCP endpoint (`/mcp`, with `RWS_ENV=dev` or `MCP_TOKEN`
/// set) shows a coding agent: routes, config, this project's layout, the
/// models, and with PostgreSQL the schema and the migrations in
/// `migrations_dir` (status, run, roll back).
pub fn dev_tools(store: &Store, migrations_dir: &str) -> DevTools {
    let tools = DevTools::new().app_dir(env!("CARGO_MANIFEST_DIR")).model::<Note>();
    match store.pool() {
        Some(pool) => tools.database(pool.clone()).migrations(migrations_dir),
        None => tools,
    }
}
