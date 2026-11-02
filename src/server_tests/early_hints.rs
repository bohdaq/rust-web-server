//! Tests of rws-server's `early_hints` that need the facade (proxy routes,
//! `rws --check`, the admin API, ACME, the `routes!` macro).
#![allow(unused_imports, dead_code)]

use std::sync::Arc;

use crate::early_hints::*;
use std::sync::Mutex;
use crate::application::Application;
use crate::request::Request;
use crate::server::ConnectionInfo;

fn collecting() -> (Arc<Sink>, Arc<Mutex<Vec<Vec<String>>>>) {
    let seen: Arc<Mutex<Vec<Vec<String>>>> = Arc::default();
    let into = Arc::clone(&seen);
    (Sink::new(move |links| into.lock().unwrap().push(links)), seen)
}

/// A backend answering every request with a `103` (one Link) and then
/// `200 backend`.
#[cfg(not(target_arch = "wasm32"))]
fn hinting_backend() -> String {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let mut seen = Vec::new();
                while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut buf) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => seen.extend_from_slice(&buf[..n]),
                    }
                }
                let _ = stream.write_all(b"HTTP/1.1 103 Early Hints\r\nLink: </from-backend.css>; rel=preload; as=style\r\n\r\n");
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\nbackend");
            });
        }
    });
    format!("{addr}")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn config_routes_send_their_hints_and_pass_a_backends_103_on() {
    let backend = hinting_backend();
    let toml = format!(
        "[[upstream]]\nname = \"app\"\nbackends = [\"http://{backend}\"]\n\n\
         [[route]]\nname = \"page\"\n[route.match]\npath = \"/page\"\n[route.action]\ntype = \"respond\"\nbody = \"page\"\n\
         [route.middleware]\nearly_hints = [\"</page.css>; rel=preload; as=style\"]\n\n\
         [[route]]\nname = \"api\"\n[route.match]\npath = \"/api/*\"\n[route.action]\ntype = \"proxy\"\nupstream = \"app\"\n\
         [route.middleware]\nearly_hints = [\"</api.css>; rel=preload; as=style\"]\n"
    );
    let report = crate::config_check::check_str(&toml);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    let config = crate::proxy_config::ProxyConfig::from_str(&toml);
    assert!(config.routes[1].middleware.is_empty(), "early hints keep the async forwarder");
    let (app, _handles) = crate::proxy_config::builder::build(config);
    let get = |path: &str| Request::parse(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes()).unwrap();
    let conn = ConnectionInfo::default();

    let (sink, seen) = collecting();
    let page = with_sink(Some(sink), || app.execute(&get("/page"), &conn)).unwrap();
    assert_eq!(page.status_code, 200);
    assert_eq!(*seen.lock().unwrap(), [vec!["</page.css>; rel=preload; as=style".to_string()]]);

    let both = vec![vec!["</api.css>; rel=preload; as=style".to_string()], vec!["</from-backend.css>; rel=preload; as=style".to_string()]];
    let (sink, seen) = collecting();
    let api = with_sink(Some(sink), || app.execute(&get("/api/1"), &conn)).unwrap();
    assert_eq!(String::from_utf8_lossy(&api.content_range_list[0].body), "backend");
    assert_eq!(*seen.lock().unwrap(), both, "sync forwarder");

    #[cfg(feature = "http2")]
    {
        let (sink, seen) = collecting();
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let request = get("/api/2");
        let response = runtime
            .block_on(scope(Some(sink), async { app.execute_async(&request, &conn).expect("async route").await }))
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&response.content_range_list[0].body), "backend");
        assert_eq!(*seen.lock().unwrap(), both, "async forwarder");
    }
    let metrics = prometheus_text();
    assert!(metrics.contains("rws_early_hints_total{source=\"configured\"}") && metrics.contains("rws_early_hints_total{source=\"upstream\"}"), "{metrics}");
}

#[test]
fn rws_check_reports_bad_hint_values() {
    let base = "[[route]]\n[route.match]\npath = \"/\"\n[route.action]\ntype = \"respond\"\nbody = \"x\"\n[route.middleware]\n";
    let report = |hints: &str| crate::config_check::check_str_offline(&format!("{base}early_hints = {hints}\n"));
    assert!(report(r#"["</a.css>; rel=preload"]"#).errors.is_empty());
    assert!(report(r#"[" "]"#).errors.join("\n").contains("early_hints value"));
    assert!(report(r#"["a.css; rel=preload"]"#).warnings.join("\n").contains("doesn't start with <URL>"));
}

#[test]
fn a_global_middleware_table_sends_its_hints_for_every_request() {
    let toml = "[middleware]\nearly_hints = [\"</site.css>; rel=preload; as=style\"]\n\n\
                [[route]]\n[route.match]\npath = \"/r\"\n[route.action]\ntype = \"respond\"\nbody = \"r\"\n";
    assert!(crate::config_check::check_str_offline(toml).errors.is_empty());
    let (app, _handles) = crate::proxy_config::builder::build(crate::proxy_config::ProxyConfig::from_str(toml));
    for path in ["/r", "/healthz"] {
        let (sink, seen) = collecting();
        let request = Request::parse(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes()).unwrap();
        with_sink(Some(sink), || app.execute(&request, &ConnectionInfo::default())).unwrap();
        assert_eq!(*seen.lock().unwrap(), [vec!["</site.css>; rel=preload; as=style".to_string()]], "{path}");
    }
}
