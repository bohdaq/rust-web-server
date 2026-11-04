//! `/healthz`: the readiness probe (`deploy/app.yaml`).

use rust_web_server::async_state::AsyncAppWithState;
use rust_web_server::get;
use rust_web_server::handler::{Json, State};
use rust_web_server::response::{StatusCodeReasonPhrase, STATUS_CODE_REASON_PHRASE};
use serde_json::{json, Value};

use crate::store::Store;

/// `200 {"status":"ok"}`, or `503` while the database doesn't answer.
#[get("/healthz")]
pub async fn health(State(store): State<Store>) -> Result<Json<Value>, (&'static StatusCodeReasonPhrase, Json<Value>)> {
    if store.ready().await {
        Ok(Json(json!({ "status": "ok" })))
    } else {
        Err((STATUS_CODE_REASON_PHRASE.n503_service_unavailable, Json(json!({ "status": "database unavailable" }))))
    }
}

pub fn register(app: AsyncAppWithState<Store>) -> AsyncAppWithState<Store> {
    app.route(health)
}
