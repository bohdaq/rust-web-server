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
