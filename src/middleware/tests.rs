use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use crate::app::App;
use crate::application::Application;
use crate::core::New;
use crate::http::VERSION;
use crate::mime_type::MimeType;
use crate::middleware::{Middleware, WithMiddleware};
use crate::range::Range;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
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

// ── Helpers ───────────────────────────────────────────────────────────────────

struct CountingMiddleware {
    count: Arc<AtomicU32>,
}

impl Middleware for CountingMiddleware {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        self.count.fetch_add(1, Ordering::Relaxed);
        next.execute(request, connection)
    }
}

struct ShortCircuitMiddleware {
    status: i16,
}

impl Middleware for ShortCircuitMiddleware {
    fn handle(&self, _request: &Request, _connection: &ConnectionInfo, _next: &dyn Application) -> Result<Response, String> {
        let mut r = Response::new();
        r.status_code = self.status;
        r.reason_phrase = "Short-Circuit".to_string();
        Ok(r)
    }
}

struct AddHeaderMiddleware {
    name: &'static str,
    value: &'static str,
}

impl Middleware for AddHeaderMiddleware {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        let mut response = next.execute(request, connection)?;
        response.headers.push(crate::header::Header { name: self.name.to_string(), value: self.value.to_string() });
        Ok(response)
    }
}

struct OrderRecordingMiddleware {
    id: &'static str,
    log: Arc<std::sync::Mutex<Vec<String>>>,
}

impl Middleware for OrderRecordingMiddleware {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        self.log.lock().unwrap().push(format!("{}-before", self.id));
        let resp = next.execute(request, connection)?;
        self.log.lock().unwrap().push(format!("{}-after", self.id));
        Ok(resp)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn middleware_runs_before_inner_app() {
    let ran = Arc::new(AtomicBool::new(false));
    let ran_clone = Arc::clone(&ran);

    struct FlagMiddleware(Arc<AtomicBool>);
    impl Middleware for FlagMiddleware {
        fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
            self.0.store(true, Ordering::Relaxed);
            next.execute(request, connection)
        }
    }

    let app = WithMiddleware::new(App::new()).wrap(FlagMiddleware(ran_clone));
    app.execute(&get("/healthz"), &conn()).unwrap();
    assert!(ran.load(Ordering::Relaxed));
}

#[test]
fn middleware_can_short_circuit() {
    let app = WithMiddleware::new(App::new()).wrap(ShortCircuitMiddleware { status: 418i16 });
    let resp = app.execute(&get("/healthz"), &conn()).unwrap();
    assert_eq!(418, resp.status_code);
}

#[test]
fn multiple_middlewares_run_in_order() {
    let log: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(std::sync::Mutex::new(vec![]));
    let app = WithMiddleware::new(App::new())
        .wrap(OrderRecordingMiddleware { id: "A", log: Arc::clone(&log) })
        .wrap(OrderRecordingMiddleware { id: "B", log: Arc::clone(&log) });

    app.execute(&get("/healthz"), &conn()).unwrap();

    let order = log.lock().unwrap().clone();
    assert_eq!(order, vec!["A-before", "B-before", "B-after", "A-after"]);
}

#[test]
fn middleware_can_add_response_header() {
    let app = WithMiddleware::new(App::new())
        .wrap(AddHeaderMiddleware { name: "X-Custom", value: "injected" });

    let resp = app.execute(&get("/healthz"), &conn()).unwrap();
    let found = resp.headers.iter().any(|h| h.name == "X-Custom" && h.value == "injected");
    assert!(found, "X-Custom header not found in response headers");
}

#[test]
fn counting_middleware_counts_requests() {
    let count = Arc::new(AtomicU32::new(0));
    let app = WithMiddleware::new(App::new()).wrap(CountingMiddleware { count: Arc::clone(&count) });

    for _ in 0..5 {
        app.execute(&get("/healthz"), &conn()).unwrap();
    }
    assert_eq!(5, count.load(Ordering::Relaxed));
}

#[test]
fn inner_app_still_serves_requests_normally() {
    let app = WithMiddleware::new(App::new()).wrap(CountingMiddleware { count: Arc::new(AtomicU32::new(0)) });
    let resp = app.execute(&get("/healthz"), &conn()).unwrap();
    assert_eq!(200, resp.status_code);
}

#[test]
fn second_middleware_short_circuits_does_not_reach_inner_app() {
    let count = Arc::new(AtomicU32::new(0));
    let app = WithMiddleware::new(App::new())
        .wrap(CountingMiddleware { count: Arc::clone(&count) })
        .wrap(ShortCircuitMiddleware { status: 503i16 });

    let resp = app.execute(&get("/healthz"), &conn()).unwrap();
    assert_eq!(503, resp.status_code);
    // The first middleware ran (it called next which hit the short-circuit layer)
    assert_eq!(1, count.load(Ordering::Relaxed));
}

#[test]
fn no_middleware_passes_through_to_inner_app() {
    let app = WithMiddleware::new(App::new());
    let resp = app.execute(&get("/healthz"), &conn()).unwrap();
    assert_eq!(200, resp.status_code);
}

#[test]
fn middleware_sees_request_method_and_uri() {
    struct AssertMiddleware;
    impl Middleware for AssertMiddleware {
        fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
            assert_eq!("GET", request.method);
            assert_eq!("/healthz", request.request_uri);
            next.execute(request, connection)
        }
    }

    let app = WithMiddleware::new(App::new()).wrap(AssertMiddleware);
    app.execute(&get("/healthz"), &conn()).unwrap();
}
