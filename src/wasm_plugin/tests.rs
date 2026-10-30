use super::*;
use crate::test_client::TestClient;

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

#[test]
fn pass_modify_and_respond() {
    let pass = client(WasmLayer::new("pass", &plugin(None, None, "")).unwrap());
    let res = pass.get("/a?b=1").header("X-In", "1").send();
    assert_eq!((res.status(), res.body_text()), (200, "GET /a?b=1\nx-in: 1\n"));

    let modify = client(WasmLayer::new("modify", &plugin(Some("set x-user: alice\nremove x-in\nuri /rewritten\n"), None, "")).unwrap());
    let res = modify.get("/a").header("X-In", "1").send();
    assert_eq!(res.body_text(), "GET /rewritten\nx-user: alice\n");

    let deny = client(WasmLayer::new("deny", &plugin(Some("respond 403\ncontent-type: application/json\nx-why: blocked\n\n{\"error\":\"no\"}"), None, "")).unwrap());
    let res = deny.get("/secret").send();
    assert_eq!(res.status(), 403);
    assert_eq!(res.body_text(), r#"{"error":"no"}"#);
    assert_eq!(res.header("x-why"), Some("blocked"));
    assert!(res.header("content-type").is_some_and(|c| c.starts_with("application/json")));
}

#[test]
fn the_response_hook_changes_status_and_headers() {
    let layer = WasmLayer::new("resp", &plugin(None, Some("set x-plugin: ran\nremove server-timing\nstatus 202\n"), "")).unwrap();
    let res = client(layer).get("/").send();
    assert_eq!(res.status(), 202);
    assert_eq!(res.header("x-plugin"), Some("ran"));
    assert_eq!(res.header("server-timing"), None);
}

#[test]
fn config_reaches_rws_init() {
    // rws_init remembers where its config is; the request hook returns it
    // as its output.
    let wat = r#"(module
        (memory (export "memory") 2)
        (global $heap (mut i32) (i32.const 8192))
        (global $cfg (mut i32) (i32.const 0))
        (global $cfg_len (mut i32) (i32.const 0))
        (func (export "rws_alloc") (param $len i32) (result i32)
          (local $p i32)
          (local.set $p (global.get $heap))
          (global.set $heap (i32.add (global.get $heap) (local.get $len)))
          (local.get $p))
        (func (export "rws_init") (param $p i32) (param $l i32)
          (global.set $cfg (local.get $p)) (global.set $cfg_len (local.get $l)))
        (func (export "rws_on_request") (param i32 i32) (result i64)
          (i64.or (i64.shl (i64.extend_i32_u (global.get $cfg)) (i64.const 32)) (i64.extend_i32_u (global.get $cfg_len)))))"#;
    let layer = WasmLayer::new("cfg", &wat::parse_str(wat).unwrap()).unwrap().config("set x-from-config: yes");
    assert_eq!(client(layer).get("/").send().body_text(), "GET /\nx-from-config: yes\n");
}

#[test]
fn every_request_gets_a_fresh_instance() {
    // Traps on its second call within one instance.
    let wat = r#"(module
        (memory (export "memory") 2)
        (global $calls (mut i32) (i32.const 0))
        (func (export "rws_alloc") (param i32) (result i32) (i32.const 8192))
        (func (export "rws_on_request") (param i32 i32) (result i64)
          (if (i32.gt_u (global.get $calls) (i32.const 0)) (then unreachable))
          (global.set $calls (i32.add (global.get $calls) (i32.const 1)))
          (i64.const 0)))"#;
    let c = client(WasmLayer::new("fresh", &wat::parse_str(wat).unwrap()).unwrap());
    for _ in 0..3 {
        assert_eq!(c.get("/").send().status(), 200);
    }
}

fn errors_of(plugin: &str, kind: &str) -> bool {
    metrics::prometheus_text().contains(&format!(r#"rws_wasm_plugin_errors_total{{plugin="{plugin}",kind="{kind}"}}"#))
}

#[test]
fn cpu_is_bounded_by_fuel() {
    let wat = r#"(module
        (memory (export "memory") 1)
        (func (export "rws_alloc") (param i32) (result i32) (i32.const 1024))
        (func (export "rws_on_request") (param i32 i32) (result i64)
          (loop $forever (br $forever))
          (i64.const 0)))"#;
    let layer = WasmLayer::new("spin", &wat::parse_str(wat).unwrap()).unwrap().fuel(100_000);
    let started = std::time::Instant::now();
    let res = client(layer).get("/").send();
    assert_eq!(res.status(), 500);
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "fuel stopped the loop");
    assert!(errors_of("spin", "fuel"), "{}", metrics::prometheus_text());
}

#[test]
fn memory_is_bounded() {
    // Asks for 64 MiB more; the limit is 2 MiB.
    let wat = r#"(module
        (memory (export "memory") 1)
        (func (export "rws_alloc") (param i32) (result i32) (i32.const 1024))
        (func (export "rws_on_request") (param i32 i32) (result i64)
          (drop (memory.grow (i32.const 1024)))
          (i64.const 0)))"#;
    let layer = WasmLayer::new("hog", &wat::parse_str(wat).unwrap()).unwrap().memory_mb(2);
    assert_eq!(client(layer).get("/").send().status(), 500);
    assert!(errors_of("hog", "memory"), "{}", metrics::prometheus_text());

    // A module whose initial memory is already over the limit.
    let big = r#"(module
        (memory (export "memory") 100)
        (func (export "rws_alloc") (param i32) (result i32) (i32.const 1024))
        (func (export "rws_on_request") (param i32 i32) (result i64) (i64.const 0)))"#;
    let layer = WasmLayer::new("big", &wat::parse_str(big).unwrap()).unwrap().memory_mb(1);
    assert_eq!(client(layer).get("/").send().status(), 500);
    assert!(errors_of("big", "memory"), "{}", metrics::prometheus_text());
}

#[test]
fn traps_and_bad_output_answer_500() {
    let unreachable = r#"(module
        (memory (export "memory") 1)
        (func (export "rws_alloc") (param i32) (result i32) (i32.const 1024))
        (func (export "rws_on_request") (param i32 i32) (result i64) unreachable))"#;
    assert_eq!(client(WasmLayer::new("boom", &wat::parse_str(unreachable).unwrap()).unwrap()).get("/").send().status(), 500);
    assert!(errors_of("boom", "trap"));

    let outside = r#"(module
        (memory (export "memory") 1)
        (func (export "rws_alloc") (param i32) (result i32) (i32.const 1024))
        (func (export "rws_on_request") (param i32 i32) (result i64) (i64.const 0x7fff000000000010)))"#;
    assert_eq!(client(WasmLayer::new("wild", &wat::parse_str(outside).unwrap()).unwrap()).get("/").send().status(), 500);
    assert!(errors_of("wild", "output"));

    for (name, output) in [("junk", "launch missiles\n"), ("crlf", "set x-a: b\rinjected: 1\n"), ("badstatus", "respond 999\n\n")] {
        let res = client(WasmLayer::new(name, &plugin(Some(output), None, "")).unwrap()).get("/").send();
        assert_eq!(res.status(), 500, "{name}");
        assert!(errors_of(name, "output"), "{name}");
    }
}

#[test]
fn modules_with_imports_or_the_wrong_shape_are_refused() {
    let imports = r#"(module
        (import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))
        (memory (export "memory") 1)
        (func (export "rws_alloc") (param i32) (result i32) (i32.const 0))
        (func (export "rws_on_request") (param i32 i32) (result i64) (i64.const 0)))"#;
    let e = WasmLayer::new("wasi", &wat::parse_str(imports).unwrap()).unwrap_err();
    assert!(e.contains("plugins get no imports"), "{e}");

    let no_hook = r#"(module (memory (export "memory") 1) (func (export "rws_alloc") (param i32) (result i32) (i32.const 0)))"#;
    assert!(WasmLayer::new("x", &wat::parse_str(no_hook).unwrap()).unwrap_err().contains("rws_on_request"));
    let wrong = r#"(module (memory (export "memory") 1)
        (func (export "rws_alloc") (param i32) (result i32) (i32.const 0))
        (func (export "rws_on_request") (param i32) (result i32) (i32.const 0)))"#;
    assert!(WasmLayer::new("x", &wat::parse_str(wrong).unwrap()).unwrap_err().contains("rws_on_request"));
    assert!(WasmLayer::new("x", b"not wasm").unwrap_err().contains("not a valid WebAssembly module"));
}

#[test]
fn output_parsing() {
    assert_eq!(parse_request_output(b"").unwrap(), RequestAction::Pass);
    assert_eq!(parse_request_output(b"\n\n").unwrap(), RequestAction::Pass);
    assert_eq!(
        parse_request_output(b"add x-a: 1\r\nset X-B:2\nremove x-c\n").unwrap(),
        RequestAction::Modify {
            edits: vec![Edit::Add("x-a".into(), "1".into()), Edit::Set("X-B".into(), "2".into()), Edit::Remove("x-c".into())],
            uri: None
        }
    );
    assert_eq!(
        parse_request_output(b"respond 401\nwww-authenticate: Bearer").unwrap(),
        RequestAction::Respond { status: 401, headers: vec![("www-authenticate".into(), "Bearer".into())], body: vec![] }
    );
    assert_eq!(
        parse_request_output(b"respond 200\n\nline one\n\nline two").unwrap(),
        RequestAction::Respond { status: 200, headers: vec![], body: b"line one\n\nline two".to_vec() }
    );
    for bad in [&b"uri http://elsewhere/"[..], b"uri /a b", b"set nocolon", b"set a b: c", b"\xff\xfe", b"remove", b"status 200"] {
        assert!(parse_request_output(bad).is_err(), "{:?}", String::from_utf8_lossy(bad));
    }
    assert_eq!(parse_response_output(b"status 204\nadd x: y\n").unwrap(), ResponseAction { edits: vec![Edit::Add("x".into(), "y".into())], status: Some(204) });
    assert!(parse_response_output(b"respond 200").is_err());
    assert!(parse_response_output(b"uri /x").is_err());
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
