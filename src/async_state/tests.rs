use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::application::Application;
use crate::async_state::AsyncAppWithState;
use crate::core::New;
use crate::http::VERSION;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::router::PathParams;
use crate::server::{Address, ConnectionInfo};

fn conn() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 16000,
    sni_hostname: None,
    }
}

fn get(uri: &str) -> Request {
    Request {
        method: METHOD.get.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    }
}

fn post(uri: &str, body: &[u8]) -> Request {
    Request {
        method: METHOD.post.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: body.to_vec(),
    }
}

fn ok_text(s: &str) -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r.content_range_list = vec![Range::get_content_range(
        s.as_bytes().to_vec(),
        MimeType::TEXT_PLAIN.to_string(),
    )];
    r
}

// ── Basic async dispatch ───────────────────────────────────────────────────────

#[tokio::test]
async fn async_get_handler_is_called() {
    let app = AsyncAppWithState::new(())
        .get("/hello", |_req, _params, _conn, _state| async { ok_text("hi") });

    let resp = app.execute(&get("/hello"), &conn()).unwrap();
    assert_eq!(200, resp.status_code);
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("hi", body);
}

#[tokio::test]
async fn async_handler_receives_path_params() {
    let app = AsyncAppWithState::new(())
        .get("/users/:id", |_req, params, _conn, _state| async move {
            let id = params.get("id").unwrap_or("?").to_string();
            ok_text(&format!("user={}", id))
        });

    let resp = app.execute(&get("/users/42"), &conn()).unwrap();
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("user=42", body);
}

#[tokio::test]
async fn async_handler_receives_state() {
    struct State { msg: String }

    let app = AsyncAppWithState::new(State { msg: "from async state".to_string() })
        .get("/msg", |_req, _params, _conn, state| async move {
            ok_text(&state.msg)
        });

    let resp = app.execute(&get("/msg"), &conn()).unwrap();
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("from async state", body);
}

#[tokio::test]
async fn async_handler_can_await() {
    async fn fetch_value() -> String {
        // simulate an async operation
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        "awaited".to_string()
    }

    let app = AsyncAppWithState::new(())
        .get("/async", |_req, _params, _conn, _state| async {
            let value = fetch_value().await;
            ok_text(&value)
        });

    let resp = app.execute(&get("/async"), &conn()).unwrap();
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("awaited", body);
}

// ── Request body and method ───────────────────────────────────────────────────

#[tokio::test]
async fn async_post_handler_reads_body() {
    let app = AsyncAppWithState::new(())
        .post("/echo", |req, _params, _conn, _state| async move {
            let body = String::from_utf8_lossy(&req.body).to_string();
            ok_text(&body)
        });

    let resp = app.execute(&post("/echo", b"hello body"), &conn()).unwrap();
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("hello body", body);
}

// ── State sharing ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn state_is_shared_across_handlers() {
    let counter = Arc::new(AtomicU32::new(0));
    let app = AsyncAppWithState::new(Arc::clone(&counter))
        .get("/inc", |_req, _params, _conn, state| async move {
            state.fetch_add(1, Ordering::Relaxed);
            ok_text("ok")
        });

    app.execute(&get("/inc"), &conn()).unwrap();
    app.execute(&get("/inc"), &conn()).unwrap();
    assert_eq!(2, counter.load(Ordering::Relaxed));
}

// ── Routing behaviour ─────────────────────────────────────────────────────────

#[tokio::test]
async fn unmatched_route_falls_through_to_app() {
    let app = AsyncAppWithState::new(())
        .get("/custom", |_, _, _, _| async { ok_text("custom") });

    let resp = app.execute(&get("/healthz"), &conn()).unwrap();
    assert_eq!(200, resp.status_code);
}

#[tokio::test]
async fn unmatched_route_returns_404() {
    let app = AsyncAppWithState::new(())
        .get("/custom", |_, _, _, _| async { ok_text("custom") });

    let resp = app.execute(&get("/does-not-exist-xyz"), &conn()).unwrap();
    assert_eq!(404, resp.status_code);
}

#[tokio::test]
async fn first_matching_route_wins() {
    let app = AsyncAppWithState::new(())
        .get("/a", |_, _, _, _| async { ok_text("first") })
        .get("/a", |_, _, _, _| async { ok_text("second") });

    let resp = app.execute(&get("/a"), &conn()).unwrap();
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("first", body);
}

#[tokio::test]
async fn wildcard_param_captures_tail() {
    let app = AsyncAppWithState::new(())
        .get("/files/*path", |_, params, _, _| async move {
            ok_text(params.get("path").unwrap_or(""))
        });

    let resp = app.execute(&get("/files/a/b/c.txt"), &conn()).unwrap();
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("a/b/c.txt", body);
}

#[tokio::test]
async fn put_patch_delete_registered() {
    let app = AsyncAppWithState::new(())
        .put("/r", |_, _, _, _| async { ok_text("put") })
        .patch("/r", |_, _, _, _| async { ok_text("patch") })
        .delete("/r", |_, _, _, _| async { ok_text("delete") });

    for (method, expected) in [("PUT", "put"), ("PATCH", "patch"), ("DELETE", "delete")] {
        let req = Request {
            method: method.to_string(),
            request_uri: "/r".to_string(),
            http_version: VERSION.http_1_1.to_string(),
            headers: vec![],
            body: vec![],
        };
        let resp = app.execute(&req, &conn()).unwrap();
        let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
        assert_eq!(expected, body, "method={}", method);
    }
}

// ── State accessor ────────────────────────────────────────────────────────────

#[tokio::test]
async fn state_accessor_returns_inner_value() {
    let app = AsyncAppWithState::new("sentinel".to_string());
    assert_eq!("sentinel", app.state());
}
