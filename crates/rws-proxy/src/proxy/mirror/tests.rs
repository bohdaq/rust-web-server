use super::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;

/// A backend that answers 200 and reports each request's first line and
/// body over the channel.
fn recording_backend() -> (String, mpsc::Receiver<(String, String)>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let tx = tx.clone();
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                loop {
                    let mut first = String::new();
                    if reader.read_line(&mut first).unwrap_or(0) == 0 {
                        return;
                    }
                    let mut length = 0;
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                            break;
                        }
                        if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                            length = v.trim().parse().unwrap_or(0);
                        }
                    }
                    let mut body = vec![0; length];
                    let _ = reader.read_exact(&mut body);
                    let _ = tx.send((first.trim().to_string(), String::from_utf8_lossy(&body).to_string()));
                    let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
                }
            });
        }
    });
    (url, rx)
}

fn get(path: &str) -> Request {
    Request::parse(format!("GET {path} HTTP/1.1\r\nHost: app\r\n\r\n").as_bytes()).unwrap()
}

fn outcome(mirror: &str, outcome: &str) -> u64 {
    counts().lock().unwrap().get(&(mirror.to_string(), outcome)).copied().unwrap_or(0)
}

#[test]
fn percent_spreads_copies_evenly() {
    let m = Mirror::new("sample-test", vec![]).percent(25.0);
    let picks: Vec<bool> = (0..8).map(|_| m.sampled()).collect();
    assert_eq!(picks, [false, false, false, true, false, false, false, true]);
    let all = Mirror::new("all", vec![]);
    assert!((0..5).all(|_| all.sampled()));
    let none = Mirror::new("none", vec![]).percent(0.0);
    assert!((0..5).all(|_| !none.sampled()));
    assert!((0..5).all(|_| !Mirror::new("nan", vec![]).percent(f64::NAN).sampled()));
    let m = Mirror::new("third", vec![]).percent(33.3);
    assert_eq!((0..999).filter(|_| m.sampled()).count(), 332, "floor(999 × 0.333)");
}

#[test]
fn copies_reach_the_mirror_with_their_body() {
    let (url, seen) = recording_backend();
    let m = Mirror::new("deliver-test", vec![url]);
    let request = Request::parse(b"POST /orders?x=1 HTTP/1.1\r\nHost: app\r\nContent-Length: 5\r\n\r\nhello").unwrap();
    assert!(m.send(&request, &ConnectionInfo::default()));
    let (line, body) = seen.recv_timeout(Duration::from_secs(5)).expect("the mirror got a copy");
    assert_eq!((line.as_str(), body.as_str()), ("POST /orders?x=1 HTTP/1.1", "hello"));
    for _ in 0..50 {
        if outcome("deliver-test", "sent") == 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(outcome("deliver-test", "sent"), 1);
}

#[test]
fn large_bodies_and_empty_backend_lists_are_not_mirrored() {
    let m = Mirror::new("skip-test", vec!["http://127.0.0.1:9".into()]);
    let big = format!("POST / HTTP/1.1\r\nHost: a\r\nContent-Length: {}\r\n\r\n{}", MAX_MIRRORED_BODY + 1, "x".repeat(MAX_MIRRORED_BODY + 1));
    assert!(!m.send(&Request::parse(big.as_bytes()).unwrap(), &ConnectionInfo::default()));
    assert_eq!(outcome("skip-test", "skipped"), 1);
    let empty = Mirror::new("empty-test", vec![]);
    assert!(!empty.send(&get("/"), &ConnectionInfo::default()));
    assert_eq!(outcome("empty-test", "failed"), 1);
}

#[test]
fn a_dead_mirror_doesnt_touch_the_response() {
    let closed = TcpListener::bind("127.0.0.1:0").unwrap();
    let dead = format!("http://{}", closed.local_addr().unwrap());
    drop(closed);
    let app = crate::middleware::WithMiddleware::new(crate::app::App::with_state(()).get("/hi", |_, _, _, _| {
        Response::text(crate::response::STATUS_CODE_REASON_PHRASE.n200_ok, "hello")
    }))
    .wrap(MirrorLayer::new(Mirror::new("dead-test", vec![dead])));
    let client = crate::test_client::TestClient::new(app);
    let started = std::time::Instant::now();
    let res = client.get("/hi").send();
    assert_eq!((res.status(), res.body_text()), (200, "hello"));
    assert!(started.elapsed() < Duration::from_millis(500), "never waits for the mirror");
    for _ in 0..100 {
        if outcome("dead-test", "failed") == 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(outcome("dead-test", "failed"), 1);
}

#[test]
fn a_config_file_proxy_route_mirrors_and_keeps_the_async_forwarder() {
    let (primary, primary_seen) = recording_backend();
    let (shadow, shadow_seen) = recording_backend();
    let toml = format!(
        r#"
[[upstream]]
name = "app"
backends = ["{primary}"]

[[upstream]]
name = "shadow"
backends = ["{shadow}"]

[[route]]
name = "api"
[route.match]
path = "/api/*"
[route.action]
type = "proxy"
upstream = "app"
[route.middleware.mirror]
upstream = "shadow"
percent = 50
"#
    );
    let report = crate::config_check::check_str(&toml);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    let config = crate::proxy_config::ProxyConfig::from_str(&toml);
    assert!(config.routes[0].middleware.is_empty(), "a mirror alone keeps the route on the async path");
    let (app, _handles) = crate::proxy_config::builder::build(config);
    for i in 0..4 {
        let response = app.execute(&get(&format!("/api/{i}")), &ConnectionInfo::default()).unwrap();
        assert_eq!(response.status_code, 200);
    }
    for _ in 0..4 {
        primary_seen.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    let mut mirrored: Vec<String> = (0..2).map(|_| shadow_seen.recv_timeout(Duration::from_secs(5)).unwrap().0).collect();
    mirrored.sort();
    assert_eq!(mirrored, ["GET /api/1 HTTP/1.1", "GET /api/3 HTTP/1.1"]);
    assert!(shadow_seen.recv_timeout(Duration::from_millis(300)).is_err(), "50%: two of four");
}

#[test]
fn rws_check_reports_mirror_mistakes() {
    let base = "[[route]]\n[route.match]\npath = \"/\"\n[route.action]\ntype = \"respond\"\nbody = \"x\"\n[route.middleware.mirror]\n";
    let errors = |extra: &str| crate::config_check::check_str_offline(&format!("{base}{extra}")).errors.join("\n");
    assert!(errors("").contains("needs `upstream` or `backends`"));
    assert!(errors("upstream = \"u\"\nbackends = [\"http://a:1\"]\n").contains("use one"));
    assert!(errors("backends = [\"http://a:1\"]\npercent = 150\n").contains("from 0 to 100"));
    assert!(errors("backends = [\"http://a:1\"]\npercent = \"lots\"\n").contains("from 0 to 100"));
    assert!(errors("backends = [\"http://a:1\"]\npercnt = 5\n").contains("percnt: unknown key"));
    assert!(errors("backends = [\"http://a:1\"]\npercent = 5\n").is_empty());
}
