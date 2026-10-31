use super::*;
use std::sync::Mutex;

fn collecting() -> (Arc<Sink>, Arc<Mutex<Vec<Vec<String>>>>) {
    let seen: Arc<Mutex<Vec<Vec<String>>>> = Arc::default();
    let into = Arc::clone(&seen);
    (Sink::new(move |links| into.lock().unwrap().push(links)), seen)
}

#[test]
fn without_a_sink_send_does_nothing() {
    assert!(!available());
    assert!(!send(&["</a.css>; rel=preload; as=style"]));
}

#[test]
fn send_reaches_the_sink_up_to_the_limit() {
    let (sink, seen) = collecting();
    with_sink(Some(sink), || {
        assert!(available());
        assert!(send(&["</a.css>; rel=preload; as=style", "</b.js>; rel=preload; as=script"]));
        for _ in 1..MAX_PER_REQUEST {
            assert!(send(&["</c.css>; rel=preload"]));
        }
        assert!(!send(&["</d.css>; rel=preload"]), "past the limit");
    });
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), MAX_PER_REQUEST);
    assert_eq!(seen[0], ["</a.css>; rel=preload; as=style", "</b.js>; rel=preload; as=script"]);
    assert!(!available(), "the sink is gone after with_sink");
}

#[test]
fn bad_values_are_refused() {
    let (sink, seen) = collecting();
    with_sink(Some(sink), || {
        assert!(!send(&["</a>; rel=preload\r\nSet-Cookie: x=1"]));
        assert!(!send(&["</a>\0"]));
        assert!(!send(&[" "]));
        assert!(!send::<&str>(&[]));
    });
    assert!(seen.lock().unwrap().is_empty());
}

#[test]
fn with_sink_restores_the_previous_one_even_on_panic() {
    let (outer, seen) = collecting();
    with_sink(Some(outer), || {
        let caught = std::panic::catch_unwind(|| with_sink(None, || panic!("handler bug")));
        assert!(caught.is_err());
        assert!(send(&["</a>; rel=preload"]), "outer sink back");
    });
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[test]
fn reset_starts_the_count_again() {
    let (sink, seen) = collecting();
    for _ in 0..2 {
        sink.reset();
        with_sink(Some(Arc::clone(&sink)), || (0..MAX_PER_REQUEST + 1).for_each(|_| { send(&["</a>; rel=preload"]); }));
    }
    assert_eq!(seen.lock().unwrap().len(), 2 * MAX_PER_REQUEST);
}

#[cfg(feature = "http2")]
#[test]
fn async_code_sends_through_the_task_scope() {
    let (sink, seen) = collecting();
    let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
    runtime.block_on(scope(Some(sink), async {
        tokio::task::yield_now().await;
        assert!(send(&["</a>; rel=preload"]));
    }));
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[test]
fn the_layer_sends_its_links_and_hands_the_request_on() {
    let app = crate::app::App::with_state(()).get("/", |_, _, _, _| Response::text(crate::response::STATUS_CODE_REASON_PHRASE.n200_ok, "page"));
    let layer = EarlyHintsLayer::new(["</a.css>; rel=preload; as=style", "bad\r\nvalue"]);
    assert_eq!(layer.links(), ["</a.css>; rel=preload; as=style"], "invalid values dropped");
    let app = crate::middleware::WithMiddleware::new(app).wrap(layer);
    let (sink, seen) = collecting();
    let request = Request::parse(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
    let response = with_sink(Some(sink), || app.execute(&request, &ConnectionInfo::default())).unwrap();
    assert_eq!(response.status_code, 200);
    assert_eq!(*seen.lock().unwrap(), [vec!["</a.css>; rel=preload; as=style".to_string()]]);
    // Without a sink (TestClient) the page is served as before.
    let client = crate::test_client::TestClient::new(app);
    assert_eq!(client.get("/").send().body_text(), "page");
}

#[test]
fn the_http1_head_lists_each_link() {
    let head = http1_head(&["</a.css>; rel=preload".to_string(), "</b.js>; rel=preload".to_string()]);
    assert_eq!(head, b"HTTP/1.1 103 Early Hints\r\nLink: </a.css>; rel=preload\r\nLink: </b.js>; rel=preload\r\n\r\n");
}

#[test]
fn upstream_103_links_are_read_from_the_head() {
    let head = b"HTTP/1.1 103 Early Hints\r\nLink: </a.css>; rel=preload\r\nlink:  </b.js>; rel=preload \r\nX-Other: y\r\n\r\n";
    assert_eq!(upstream_links(head), ["</a.css>; rel=preload", "</b.js>; rel=preload"]);
    assert!(upstream_links(b"HTTP/1.1 100 Continue\r\nLink: </a>\r\n\r\n").is_empty());
    assert!(upstream_links(b"HTTP/1.1 103 Early Hints\r\n\r\n").is_empty());
    assert!(upstream_links(b"garbage").is_empty());
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
