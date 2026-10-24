//! `Application::runs_inline` across the built-in wrappers: a request runs on
//! the event loop only when every layer opts in.

use crate::app::App;
use crate::application::Application;
use crate::core::New;
use crate::middleware::{Middleware, WithMiddleware};
use crate::request::Request;
use crate::response::Response;
use crate::server::{Address, ConnectionInfo};

fn conn() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 1 },
        server: Address { ip: "127.0.0.1".to_string(), port: 2 },
        request_size: 1024,
        sni_hostname: None,
    }
}

fn req(method: &str, uri: &str) -> Request {
    Request { method: method.to_string(), request_uri: uri.to_string(), http_version: "HTTP/1.1".to_string(), headers: vec![], body: vec![] }
}

/// A user layer that doesn't declare itself non-blocking.
struct UserLayer;
impl Middleware for UserLayer {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        next.execute(request, connection)
    }
}

#[test]
fn app_runs_get_and_head_inline_but_not_posts() {
    let app = App::new();
    assert!(app.runs_inline(&req("GET", "/healthz"), &conn()));
    assert!(app.runs_inline(&req("HEAD", "/index.html"), &conn()));
    assert!(!app.runs_inline(&req("POST", "/file-upload/initiate"), &conn()));
}

#[test]
fn middleware_stack_runs_inline_only_if_every_layer_opts_in() {
    let built_in = WithMiddleware::new(App::new()).wrap(crate::middleware::RateLimitLayer).wrap(crate::request_id::RequestIdLayer::new());
    assert!(built_in.runs_inline(&req("GET", "/healthz"), &conn()));
    let with_user_layer = WithMiddleware::new(App::new()).wrap(crate::middleware::RateLimitLayer).wrap(UserLayer);
    assert!(!with_user_layer.runs_inline(&req("GET", "/healthz"), &conn()));
}

#[test]
fn user_routes_never_run_inline() {
    fn handler(_: &Request, _: &crate::router::PathParams, _: &ConnectionInfo, _: &()) -> Response {
        Response::new()
    }
    let app = App::with_state(()).get("/api/users", handler);
    assert!(!app.runs_inline(&req("GET", "/api/users"), &conn()), "a route handler is user code");
    assert!(app.runs_inline(&req("GET", "/healthz"), &conn()), "unmatched requests fall through to App");
}

#[test]
fn proxy_routes_never_run_inline() {
    let config = crate::proxy_config::ProxyConfig::from_str(r#"
[[route]]
  [route.match]
  path = "/api/*"
  [route.action]
  type     = "proxy"
  upstream = "127.0.0.1:9"
"#);
    let (app, _) = crate::proxy_config::builder::build(config);
    assert!(!app.runs_inline(&req("GET", "/api/x"), &conn()));
    assert!(app.runs_inline(&req("GET", "/healthz"), &conn()));
}

#[test]
fn mcp_calls_never_run_inline() {
    let mcp = crate::mcp::McpServer::new("t", "1");
    assert!(!mcp.runs_inline(&req("POST", "/mcp"), &conn()));
    assert!(!mcp.runs_inline(&req("GET", "/mcp"), &conn()), "GET /mcp is a long-lived SSE stream");
    assert!(mcp.runs_inline(&req("GET", "/healthz"), &conn()));
}
