use super::{with_timeout, with_timeout_state, TimeoutLayer};
use crate::application::Application;
use crate::core::New;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::router::{PathParams, Router};
use crate::server::{Address, ConnectionInfo};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

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
        http_version: "HTTP/1.1".to_string(),
        headers: vec![],
        body: vec![],
    }
}

fn ok_text(body: &str) -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r.content_range_list = vec![Range::get_content_range(body.as_bytes().to_vec(), MimeType::TEXT_PLAIN.to_string())];
    r
}

// ── with_timeout (Router) ───────────────────────────────────────────────────

#[test]
fn with_timeout_returns_result_when_handler_is_fast() {
    let router = Router::new().get(
        "/fast",
        with_timeout(Duration::from_millis(200), |_req, _params, _conn| ok_text("fast")),
    );

    let resp = router.handle(&get("/fast"), &conn()).unwrap();
    assert_eq!(200, resp.status_code);
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("fast", body);
}

#[test]
fn with_timeout_returns_504_when_handler_is_slow() {
    let router = Router::new().get(
        "/slow",
        with_timeout(Duration::from_millis(20), |_req, _params, _conn| {
            thread::sleep(Duration::from_millis(300));
            ok_text("too late")
        }),
    );

    let resp = router.handle(&get("/slow"), &conn()).unwrap();
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n504_gateway_timeout.status_code, resp.status_code);
}

#[test]
fn with_timeout_can_be_registered_for_multiple_routes_with_different_durations() {
    let router = Router::new()
        .get("/healthz", with_timeout(Duration::from_millis(500), |_, _, _| ok_text("ok")))
        .post("/upload", with_timeout(Duration::from_secs(120), |_, _, _| ok_text("uploaded")));

    assert_eq!(200, router.handle(&get("/healthz"), &conn()).unwrap().status_code);
}

// ── with_timeout_state (AppWithState) ───────────────────────────────────────

#[derive(Clone)]
struct CloneableState {
    label: String,
}

#[test]
fn with_timeout_state_returns_result_when_handler_is_fast() {
    let state = CloneableState { label: "hi".to_string() };
    let wrapped = with_timeout_state(Duration::from_millis(200), |_req, _params, _conn, state: &CloneableState| {
        ok_text(&state.label)
    });

    let resp = wrapped(&get("/x"), &PathParams::from_map(Default::default()), &conn(), &state);
    assert_eq!(200, resp.status_code);
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("hi", body);
}

#[test]
fn with_timeout_state_returns_504_when_handler_is_slow() {
    let state = CloneableState { label: "hi".to_string() };
    let wrapped = with_timeout_state(Duration::from_millis(20), |_req, _params, _conn, _state: &CloneableState| {
        thread::sleep(Duration::from_millis(300));
        ok_text("too late")
    });

    let resp = wrapped(&get("/x"), &PathParams::from_map(Default::default()), &conn(), &state);
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n504_gateway_timeout.status_code, resp.status_code);
}

// ── TimeoutLayer (Application wrapper) ──────────────────────────────────────

struct SlowApp;
impl Application for SlowApp {
    fn execute(&self, _request: &Request, _connection: &ConnectionInfo) -> Result<Response, String> {
        thread::sleep(Duration::from_millis(300));
        Ok(ok_text("slow"))
    }
}

struct FastApp;
impl Application for FastApp {
    fn execute(&self, _request: &Request, _connection: &ConnectionInfo) -> Result<Response, String> {
        Ok(ok_text("fast"))
    }
}

#[test]
fn timeout_layer_new_passes_through_fast_application() {
    let app = TimeoutLayer::new(FastApp, Duration::from_millis(200));
    let resp = app.execute(&get("/x"), &conn()).unwrap();
    assert_eq!(200, resp.status_code);
}

#[test]
fn timeout_layer_new_returns_504_for_slow_application() {
    let app = TimeoutLayer::new(SlowApp, Duration::from_millis(20));
    let resp = app.execute(&get("/x"), &conn()).unwrap();
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n504_gateway_timeout.status_code, resp.status_code);
}

#[test]
fn timeout_layer_from_arc_wraps_a_trait_object() {
    let inner: Arc<dyn Application + Send + Sync> = Arc::new(SlowApp);
    let app = TimeoutLayer::from_arc(inner, Duration::from_millis(20));
    let resp = app.execute(&get("/x"), &conn()).unwrap();
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n504_gateway_timeout.status_code, resp.status_code);
}

#[test]
fn timeout_layer_returns_504_body_as_text() {
    let app = TimeoutLayer::new(SlowApp, Duration::from_millis(20));
    let resp = app.execute(&get("/x"), &conn()).unwrap();
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert!(body.contains("504"));
}

#[test]
fn timeout_layer_does_not_block_the_caller_for_the_slow_apps_full_duration() {
    let app = TimeoutLayer::new(SlowApp, Duration::from_millis(20));
    let start = std::time::Instant::now();
    let _ = app.execute(&get("/x"), &conn()).unwrap();
    let elapsed = start.elapsed();
    // SlowApp sleeps 300ms; the timeout is 20ms. The call must return close
    // to the timeout, not wait for the full 300ms — this is the whole point.
    assert!(elapsed < Duration::from_millis(150), "took {elapsed:?}, expected well under 150ms");
}

#[test]
fn concurrent_timeout_layer_calls_do_not_interfere() {
    let app = Arc::new(TimeoutLayer::new(FastApp, Duration::from_millis(200)));
    let counter = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];
    for _ in 0..8 {
        let app = Arc::clone(&app);
        let counter = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            let resp = app.execute(&get("/x"), &conn()).unwrap();
            if resp.status_code == 200 {
                counter.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(8, counter.load(Ordering::Relaxed));
}

// ── with_timeout_async (AsyncAppWithState) ──────────────────────────────────

#[cfg(feature = "http2")]
mod async_tests {
    use super::*;
    use crate::timeout::with_timeout_async;
    use std::sync::atomic::{AtomicBool, Ordering as AtOrd};

    #[tokio::test]
    async fn with_timeout_async_returns_result_when_future_is_fast() {
        let wrapped = with_timeout_async(Duration::from_millis(200), |_req, _params, _conn, _state: Arc<()>| async {
            ok_text("fast")
        });

        let resp = wrapped(get("/x"), PathParams::from_map(Default::default()), conn(), Arc::new(())).await;
        assert_eq!(200, resp.status_code);
    }

    #[tokio::test]
    async fn with_timeout_async_returns_504_when_future_is_slow() {
        let wrapped = with_timeout_async(Duration::from_millis(20), |_req, _params, _conn, _state: Arc<()>| async {
            tokio::time::sleep(Duration::from_millis(300)).await;
            ok_text("too late")
        });

        let resp = wrapped(get("/x"), PathParams::from_map(Default::default()), conn(), Arc::new(())).await;
        assert_eq!(*STATUS_CODE_REASON_PHRASE.n504_gateway_timeout.status_code, resp.status_code);
    }

    #[tokio::test]
    async fn with_timeout_async_actually_cancels_the_future_at_the_deadline() {
        // Proves genuine cancellation (not just a discarded result): the
        // flag is only set *after* an await point past the deadline, so if
        // tokio::time::timeout truly drops the future, it must never run.
        let ran_past_deadline = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&ran_past_deadline);

        let wrapped = with_timeout_async(Duration::from_millis(20), move |_req, _params, _conn, _state: Arc<()>| {
            let flag = Arc::clone(&flag);
            async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                flag.store(true, AtOrd::SeqCst);
                ok_text("done")
            }
        });

        let resp = wrapped(get("/x"), PathParams::from_map(Default::default()), conn(), Arc::new(())).await;
        assert_eq!(*STATUS_CODE_REASON_PHRASE.n504_gateway_timeout.status_code, resp.status_code);

        // Give the (cancelled) future's slot no further chance to run, then confirm it never did.
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(!ran_past_deadline.load(AtOrd::SeqCst), "future ran past its cancellation point");
    }
}
