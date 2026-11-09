//! `/api/account`: the signed-in account (`401` without a session). The
//! account routes themselves (`/auth/signup`, `/auth/login`, …) come from
//! `AuthFlows` (src/accounts.rs).

use rust_web_server::async_state::AsyncAppWithState;
use rust_web_server::auth_flows::{signed_in, CurrentUser};
use rust_web_server::get;
use rust_web_server::handler::Json;
use serde_json::{json, Value};

use crate::store::Store;

#[get("/")]
pub async fn me(user: CurrentUser) -> Json<Value> {
    Json(json!({ "id": user.id, "email": user.email, "confirmed": user.confirmed }))
}

pub fn register(app: AsyncAppWithState<Store>) -> AsyncAppWithState<Store> {
    app.group("/api/account", |account| account.guard(signed_in).route(me))
}
