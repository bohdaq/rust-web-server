//! Tests of `wasm_plugin` (a split crate) that need the facade or repository fixtures:
//! a crate's tests run in the crate's directory, without the crates above it.
#![allow(unused_imports, dead_code)]

use crate::wasm_plugin::*;
use crate::test_client::TestClient;
use crate::application::Application;
use crate::header::Header;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

/// A WAT string literal holding `bytes`.
fn wat_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("\\{b:02x}")).collect()
}

/// A plugin whose hooks return fixed output (`None`: pass) and whose other
/// functions are `extra` (WAT).
fn plugin(request_output: Option<&str>, response_output: Option<&str>, extra: &str) -> Vec<u8> {
    let hook = |name: &str, at: u32, output: Option<&str>| match output {
        None => format!(r#"(func (export "{name}") (param i32 i32) (result i64) (i64.const 0))"#),
        Some(out) => format!(
            r#"(data (i32.const {at}) "{}")
               (func (export "{name}") (param i32 i32) (result i64)
                 (i64.or (i64.shl (i64.const {at}) (i64.const 32)) (i64.const {})))"#,
            wat_bytes(out.as_bytes()),
            out.len()
        ),
    };
    let response = if response_output.is_some() { hook("rws_on_response", 2048, response_output) } else { String::new() };
    let wat = format!(
        r#"(module
             (memory (export "memory") 2)
             (global $heap (mut i32) (i32.const 8192))
             (func (export "rws_alloc") (param $len i32) (result i32)
               (local $p i32)
               (local.set $p (global.get $heap))
               (global.set $heap (i32.add (global.get $heap) (local.get $len)))
               (local.get $p))
             {}
             {response}
             {extra})"#,
        hook("rws_on_request", 1024, request_output)
    );
    wat::parse_str(&wat).unwrap_or_else(|e| panic!("bad test WAT: {e}\n{wat}"))
}

/// The app behind the plugin: echoes the request line and `x-*` headers.
fn echo() -> impl Application + Clone {
    crate::app::App::with_state(()).get("/*rest", |request, _, _, _| {
        let mut text = format!("{} {}\n", request.method, request.request_uri);
        for h in request.headers.iter().filter(|h| h.name.to_ascii_lowercase().starts_with("x-")) {
            text.push_str(&format!("{}: {}\n", h.name.to_ascii_lowercase(), h.value));
        }
        let mut response = Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, &text);
        response.headers.push(Header::new("Server-Timing", "app;dur=1"));
        response
    })
}

fn client(layer: WasmLayer) -> TestClient<impl Application> {
    TestClient::new(crate::middleware::WithMiddleware::new(echo()).wrap(layer))
}

fn errors_of(plugin: &str, kind: &str) -> bool {
    metrics::prometheus_text().contains(&format!(r#"rws_wasm_plugin_errors_total{{plugin="{plugin}",kind="{kind}"}}"#))
}

#[test]
fn the_config_file_section_parses_and_counts_as_middleware() {
    let toml = r#"
[[upstream]]
name = "u"
backends = ["http://127.0.0.1:9"]

[[route]]
[route.match]
path = "/"
[route.action]
type = "proxy"
upstream = "u"
[route.middleware.wasm]
module = "plugins/tag.wasm"
fuel = 1000
memory_mb = 4
config = "hello"
"#;
    let config = crate::proxy_config::ProxyConfig::from_str(toml);
    let mw = &config.routes[0].middleware;
    assert_eq!(
        mw.wasm,
        Some(crate::proxy_config::WasmConfig { module: "plugins/tag.wasm".into(), fuel: 1000, memory_mb: 4, config: "hello".into() })
    );
    assert!(!mw.is_empty(), "a plugin must keep the route off the middleware-free fast path");
    let offline = crate::config_check::check_str_offline(toml);
    assert!(offline.errors.is_empty(), "{:?}", offline.errors);
    let online = crate::config_check::check_str(toml);
    assert!(online.errors.iter().any(|e| e.contains("plugins/tag.wasm")), "{:?}", online.errors);
    let typo = toml.replace("memory_mb = 4", "memory = 4");
    assert!(crate::config_check::check_str_offline(&typo).errors.iter().any(|e| e.contains("memory: unknown key")));
}
