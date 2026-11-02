//! Tests of rws-server's `application` that need the facade (proxy routes,
//! `rws --check`, the admin API, ACME, the `routes!` macro).
#![allow(unused_imports, dead_code)]

use crate::app::App;
use crate::core::New;
use crate::middleware::{Middleware, WithMiddleware};
use crate::response::Response;
use crate::server::{Address, ConnectionInfo};
use crate::application::Application;
use crate::request::Request;

fn conn() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 1 },
        server: Address { ip: "127.0.0.1".to_string(), port: 2 },
        request_size: 1024,
        sni_hostname: None,
        tls: false,
    }
}

fn req(method: &str, uri: &str) -> Request {
    Request { method: method.to_string(), request_uri: uri.to_string(), http_version: "HTTP/1.1".to_string(), headers: vec![], body: Default::default() }
}

/// A user layer that doesn't declare itself non-blocking.
struct UserLayer;

impl Middleware for UserLayer {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        next.execute(request, connection)
    }
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
