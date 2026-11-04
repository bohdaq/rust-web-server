use super::*;
use crate::server::Address as PeerAddress;
use crate::test_env::TempPath;
use std::net::TcpListener;

fn request(method: &str, uri: &str, headers: &[(&str, &str)], body: &[u8]) -> Request {
    Request {
        method: method.to_string(),
        request_uri: uri.to_string(),
        http_version: "HTTP/1.1".to_string(),
        headers: headers.iter().map(|(n, v)| Header::new(n.to_string(), v.to_string())).collect(),
        body: body.to_vec().into(),
    }
}

fn connection(tls: bool) -> ConnectionInfo {
    let mut c = ConnectionInfo::new(PeerAddress { ip: "203.0.113.5".to_string(), port: 40000 }, PeerAddress { ip: "127.0.0.1".to_string(), port: 8443 });
    c.tls = tls;
    c
}

/// A FastCGI responder on a free port: for each request it answers with
/// `reply(params, stdin)` (a CGI response) over kept-alive connections.
/// Returns its address and a counter of connections accepted.
fn responder(reply: fn(&[(String, String)], &[u8]) -> Vec<u8>) -> (String, Arc<std::sync::atomic::AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let accepted = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = Arc::clone(&accepted);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            std::thread::spawn(move || loop {
                let (mut params, mut stdin, mut keep) = (Vec::new(), Vec::new(), false);
                loop {
                    let Ok(Some(r)) = read_record(&mut stream) else { return };
                    match r.kind {
                        BEGIN_REQUEST => keep = r.content.get(2) == Some(&KEEP_CONN),
                        PARAMS => params.extend_from_slice(&r.content),
                        STDIN if r.content.is_empty() => break,
                        STDIN => stdin.extend_from_slice(&r.content),
                        _ => {}
                    }
                }
                let params = decode_params(&params).unwrap();
                let out = reply(&params, &stdin);
                let mut msg = Vec::new();
                encode_record(&mut msg, STDERR, 1, b"a warning from the script\n");
                for chunk in out.chunks(1000) {
                    encode_record(&mut msg, STDOUT, 1, chunk);
                }
                encode_record(&mut msg, STDOUT, 1, &[]);
                encode_record(&mut msg, END_REQUEST, 1, &[0, 0, 0, 0, 0, 0, 0, 0]);
                if stream.write_all(&msg).is_err() || !keep {
                    return;
                }
            });
        }
    });
    (addr, accepted)
}

/// A CGI response echoing the params and body as `key=value` lines.
fn echo(params: &[(String, String)], stdin: &[u8]) -> Vec<u8> {
    let mut body = String::new();
    for (k, v) in params {
        body.push_str(&format!("{k}={v}\n"));
    }
    body.push_str(&format!("BODY={}\n", String::from_utf8_lossy(stdin)));
    format!("Content-Type: text/plain\r\nX-Powered-By: test\r\nSet-Cookie: a=1\r\nSet-Cookie: b=2\r\n\r\n{body}").into_bytes()
}

/// A docroot: index.php, other.php, style.css, sub/index.php, uploads/x.jpg.
fn docroot(name: &str) -> TempPath {
    let root = TempPath::new(name);
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::create_dir_all(root.join("uploads")).unwrap();
    std::fs::create_dir_all(root.join(".git")).unwrap();
    for f in ["index.php", "other.php", "sub/index.php", ".git/config.php"] {
        std::fs::write(root.join(f), "<?php").unwrap();
    }
    std::fs::write(root.join("style.css"), "body{}").unwrap();
    std::fs::write(root.join("uploads/x.jpg"), "jpeg").unwrap();
    root
}

fn body_text(r: &Response) -> String {
    String::from_utf8_lossy(&Response::generate_body(r.content_range_list.clone())).into_owned()
}

#[test]
fn records_and_params_round_trip() {
    let params = vec![("SHORT".to_string(), "v".to_string()), ("LONG".to_string(), "x".repeat(300))];
    assert_eq!(decode_params(&encode_params(&params)).unwrap(), params);
    let mut buf = Vec::new();
    encode_stream(&mut buf, STDIN, 7, &vec![b'a'; 70_000]);
    let records = parse_records(&buf).unwrap();
    assert_eq!(records.len(), 3, "65535 + rest + terminator");
    assert!(records.iter().all(|r| r.request_id == 7 && r.kind == STDIN));
    assert_eq!(records[0].content.len() + records[1].content.len(), 70_000);
    assert!(records[2].content.is_empty());
    assert_eq!(buf.len() % 8, 0, "padded to 8 bytes");
    assert!(decode_params(&[5, 1, b'a']).is_none(), "truncated");
    assert!(parse_records(&[2, 6, 0, 1, 0, 0, 0, 0]).is_none(), "bad version");
    fuzz::records(&[1, 4, 0, 1, 0, 3, 0, 0, 0x80, 0xff, 0xff]);
}

#[test]
fn cgi_headers_set_status_and_redirects() {
    assert_eq!(parse_cgi_headers(b"Status: 404 Not Here\r\nContent-Type: text/html\r\n").unwrap(), (404, "Not Here".to_string(), vec![("Content-Type".to_string(), "text/html".to_string())]));
    assert_eq!(parse_cgi_headers(b"Location: /login\n").unwrap().0, 302);
    assert_eq!(parse_cgi_headers(b"Status: 301\nLocation: /x\n").unwrap().0, 301);
    assert_eq!(parse_cgi_headers(b"X: 1\n").unwrap().0, 200);
    assert!(parse_cgi_headers(b"no colon here\n").is_err());
    assert!(parse_cgi_headers(b"Status: abc\n").is_err());
    assert_eq!(header_end(b"A: 1\n\nbody"), Some(6));
    assert_eq!(header_end(b"A: 1\r\n\r\nbody"), Some(8));
}

#[test]
fn addresses_parse() {
    assert_eq!(Address::parse("127.0.0.1:9000").unwrap(), Address::Tcp("127.0.0.1:9000".to_string()));
    assert_eq!(Address::parse("tcp://php:9000").unwrap(), Address::Tcp("php:9000".to_string()));
    #[cfg(unix)]
    assert_eq!(Address::parse("unix:/run/php/fpm.sock").unwrap(), Address::Unix(PathBuf::from("/run/php/fpm.sock")));
    assert!(Address::parse("php").is_err());
}

#[test]
fn paths_map_to_scripts_static_files_and_the_front_controller() {
    let root = docroot("fcgi-resolve");
    let app = FastCgi::new(FastCgiConfig::new("127.0.0.1:1", &*root)).unwrap();
    let script = |t: Target| match t {
        Target::Script { script_name, path_info, .. } => format!("{script_name}|{path_info}"),
        other => format!("{other:?}"),
    };
    assert_eq!(script(app.resolve("/other.php")), "/other.php|");
    assert_eq!(script(app.resolve("/")), "/index.php|");
    assert_eq!(script(app.resolve("/sub/")), "/sub/index.php|");
    assert_eq!(script(app.resolve("/sub")), "Redirect(\"/sub/\")");
    assert_eq!(script(app.resolve("/other.php/users/7")), "/other.php|/users/7");
    assert_eq!(script(app.resolve("/users/7")), "/index.php|", "front controller");
    assert_eq!(app.resolve("/style.css"), Target::Static);
    // Not the cgi.fix_pathinfo hole: x.jpg is never run as PHP.
    assert_eq!(script(app.resolve("/uploads/x.jpg/a.php")), "/index.php|");
    assert_eq!(app.resolve("/../etc/passwd"), Target::NotFound);
    assert_eq!(app.resolve("/.git/config.php"), Target::NotFound);
    std::fs::create_dir_all(root.join(".well-known")).unwrap();
    std::fs::write(root.join(".well-known/security.txt"), "Contact: x").unwrap();
    assert_eq!(app.resolve("/.well-known/security.txt"), Target::Static, ".well-known is served");
    assert_eq!(script(app.resolve("/.env")), "NotFound");
    // Kept literal (no file of that name): the front controller, never outside the root.
    assert_eq!(script(app.resolve("/%2e%2e/x")), "/index.php|");

    let mut cfg = FastCgiConfig::new("127.0.0.1:1", &*root);
    cfg.front_controller = String::new();
    let app = FastCgi::new(cfg).unwrap();
    assert_eq!(app.resolve("/users/7"), Target::NotFound);
    std::fs::remove_dir_all(&*root).ok();
}

#[test]
fn a_script_runs_with_cgi_variables_and_its_response_comes_back() {
    let root = docroot("fcgi-run");
    let (addr, accepted) = responder(echo);
    let mut cfg = FastCgiConfig::new(addr, &*root);
    cfg.params = vec![("APP_ENV".to_string(), "prod".to_string())];
    let app = FastCgi::new(cfg).unwrap();
    let req = request(
        "POST",
        "/other.php/a/b?x=1&y=2",
        &[("Host", "shop.test:8443"), ("Content-Type", "application/x-www-form-urlencoded"), ("Cookie", "s=1"), ("Proxy", "http://evil:1"), ("X-Custom", "yes")],
        b"name=rws",
    );
    let res = app.execute(&req, &connection(true)).unwrap();
    assert_eq!(res.status_code, 200);
    let body = body_text(&res);
    for line in [
        "REQUEST_METHOD=POST",
        "REQUEST_URI=/other.php/a/b?x=1&y=2",
        "QUERY_STRING=x=1&y=2",
        "SCRIPT_NAME=/other.php",
        "PATH_INFO=/a/b",
        "CONTENT_TYPE=application/x-www-form-urlencoded",
        "CONTENT_LENGTH=8",
        "HTTPS=on",
        "SERVER_NAME=shop.test",
        "SERVER_PORT=8443",
        "REMOTE_ADDR=203.0.113.5",
        "HTTP_COOKIE=s=1",
        "HTTP_X_CUSTOM=yes",
        "APP_ENV=prod",
        "BODY=name=rws",
    ] {
        assert!(body.lines().any(|l| l == line), "missing {line:?} in\n{body}");
    }
    assert!(body.contains(&format!("SCRIPT_FILENAME={}/other.php", root.display())), "{body}");
    assert!(body.contains(&format!("DOCUMENT_ROOT={}\n", root.display())), "{body}");
    assert!(!body.contains("HTTP_PROXY"), "httpoxy: {body}");
    assert_eq!(res.headers.iter().filter(|h| h.name.eq_ignore_ascii_case("set-cookie")).count(), 2);
    assert!(res.content_range_list[0].content_type.starts_with("text/plain"));

    // Kept alive: the next requests reuse the connection.
    for _ in 0..3 {
        assert_eq!(app.execute(&request("GET", "/", &[], b""), &connection(false)).unwrap().status_code, 200);
    }
    assert_eq!(accepted.load(std::sync::atomic::Ordering::SeqCst), 1);
    std::fs::remove_dir_all(&*root).ok();
}

#[test]
fn status_and_location_from_the_script() {
    let root = docroot("fcgi-status");
    let (addr, _) = responder(|params, _| {
        let uri = params.iter().find(|(k, _)| k == "REQUEST_URI").map(|(_, v)| v.as_str()).unwrap_or("");
        match uri {
            "/missing" => b"Status: 404 Not Found\r\nContent-Type: text/html\r\n\r\n<h1>gone</h1>".to_vec(),
            "/login" => b"Location: /dashboard\r\n\r\n".to_vec(),
            _ => b"garbage without headers".to_vec(),
        }
    });
    let app = FastCgi::new(FastCgiConfig::new(addr, &*root)).unwrap();
    let res = app.execute(&request("GET", "/missing", &[], b""), &connection(false)).unwrap();
    assert_eq!((res.status_code, body_text(&res).as_str()), (404, "<h1>gone</h1>"));
    let res = app.execute(&request("GET", "/login", &[], b""), &connection(false)).unwrap();
    assert_eq!(res.status_code, 302);
    assert!(res.headers.iter().any(|h| h.name == "Location" && &*h.value == "/dashboard"));
    assert_eq!(app.execute(&request("GET", "/bad", &[], b""), &connection(false)).unwrap().status_code, 502);
    std::fs::remove_dir_all(&*root).ok();
}

#[test]
fn an_unreachable_server_is_502_and_static_files_go_to_the_static_handler() {
    let root = docroot("fcgi-down");
    struct Static;
    impl Application for Static {
        fn execute(&self, _: &Request, _: &ConnectionInfo) -> Result<Response, String> {
            Ok(simple(200, Vec::new(), "static"))
        }
    }
    let app = FastCgi::new(FastCgiConfig::new("127.0.0.1:1", &*root)).unwrap().with_static(Arc::new(Static));
    assert_eq!(app.execute(&request("GET", "/index.php", &[], b""), &connection(false)).unwrap().status_code, 502);
    let res = app.execute(&request("GET", "/style.css", &[], b""), &connection(false)).unwrap();
    assert_eq!(body_text(&res), "static");
    let res = app.execute(&request("GET", "/sub?a=1", &[], b""), &connection(false)).unwrap();
    assert_eq!(res.status_code, 301);
    assert!(res.headers.iter().any(|h| h.name == "Location" && &*h.value == "/sub/?a=1"));
    assert!(FastCgi::new(FastCgiConfig::new("127.0.0.1:1", "/nonexistent/rws-root")).is_err());
    std::fs::remove_dir_all(&*root).ok();
}

/// php-fpm in another container sees the same paths but not the same
/// symlink targets (a Kubernetes ConfigMap volume: `app.php` →
/// `..data/app.php` → `..2026_11_04_…/app.php`, a directory named per pod),
/// so it is told the path under the configured root, not the resolved one.
#[cfg(unix)]
#[test]
fn script_filename_keeps_symlinks_in_the_path() {
    let root = docroot("fcgi-symlinked");
    std::fs::create_dir_all(root.join("..2026_11_04_05_58_48.1")).unwrap();
    std::fs::write(root.join("..2026_11_04_05_58_48.1/app.php"), "<?php").unwrap();
    std::os::unix::fs::symlink("..2026_11_04_05_58_48.1", root.join("..data")).unwrap();
    std::os::unix::fs::symlink("..data/app.php", root.join("app.php")).unwrap();
    let app = FastCgi::new(FastCgiConfig::new("127.0.0.1:1", &*root)).unwrap();
    match app.resolve("/app.php") {
        Target::Script { filename, .. } => assert_eq!(filename, root.join("app.php")),
        other => panic!("{other:?}"),
    }
    // A link out of the root is still refused.
    let outside = TempPath::new("fcgi-outside");
    std::fs::create_dir_all(&*outside).unwrap();
    std::fs::write(outside.join("x.php"), "<?php").unwrap();
    std::os::unix::fs::symlink(outside.join("x.php"), root.join("out.php")).unwrap();
    assert!(!matches!(app.resolve("/out.php"), Target::Script { ref script_name, .. } if script_name == "/out.php"));
}
