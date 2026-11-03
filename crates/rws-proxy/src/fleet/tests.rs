use super::*;
use crate::test_env::TempPath;
use std::io::{Read, Write};
use std::sync::Arc;

const GOOD: &str = "port = 7979\n[[route]]\n[route.match]\npath = \"/*\"\n[route.action]\ntype = \"respond\"\nstatus = 200\nbody = \"fleet\"\n";
const BAD: &str = "port = 7979\n[[route]]\n[route.match]\npath = \"/*\"\n[route.action]\ntype = \"proxy\"\nupstream = \"nowhere\"\n";

fn keypair() -> (aws_lc_rs::signature::Ed25519KeyPair, Vec<u8>) {
    use aws_lc_rs::signature::KeyPair;
    let pkcs8 = aws_lc_rs::signature::Ed25519KeyPair::generate_pkcs8(&aws_lc_rs::rand::SystemRandom::new()).unwrap();
    let pair = aws_lc_rs::signature::Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
    let public = pair.public_key().as_ref().to_vec();
    (pair, public)
}

fn sig(pair: &aws_lc_rs::signature::Ed25519KeyPair, body: &str) -> String {
    Base64::encode(pair.sign(body.as_bytes()).as_ref()).unwrap()
}

/// An HTTP source serving `files` (path → (body, etag)), answering
/// `If-None-Match` with 304, and recording POSTs (reports).
fn server(files: Arc<Mutex<HashMap<String, (String, String)>>>, reports: Arc<Mutex<Vec<String>>>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut buf = vec![0u8; 16384];
            let n = s.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
            if req.starts_with("POST") {
                reports.lock().unwrap().push(req.split("\r\n\r\n").nth(1).unwrap_or("").to_string());
                let _ = s.write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n");
                continue;
            }
            let files = files.lock().unwrap();
            let reply = match files.get(&path) {
                Some((_, etag)) if req.to_ascii_lowercase().contains(&format!("if-none-match: {etag}")) => "HTTP/1.1 304 Not Modified\r\nConnection: close\r\n\r\n".to_string(),
                Some((body, etag)) => format!("HTTP/1.1 200 OK\r\nETag: {etag}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()),
                None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
            };
            let _ = s.write_all(reply.as_bytes());
        }
    });
    format!("http://{addr}")
}
use std::collections::HashMap;

fn fleet(source: Source, public_key: Option<Vec<u8>>, report_url: String, path: &TempPath) -> Fleet {
    Fleet::new(FleetConfig {
        source,
        interval: Duration::from_secs(30),
        public_key,
        report_url,
        node: "web-1".into(),
        config_path: path.to_path_buf(),
        state_dir: path.with_extension("state"),
        s3_endpoint: String::new(),
    })
}

#[test]
fn sources_parse() {
    assert_eq!(Source::parse("https://cfg.example/rws/{node}.toml", "web-7").unwrap(), Source::Http("https://cfg.example/rws/web-7.toml".into()));
    assert_eq!(Source::parse("s3://ops-config/rws/prod.toml", "n").unwrap(), Source::S3 { bucket: "ops-config".into(), key: "rws/prod.toml".into() });
    assert_eq!(
        Source::parse("git+https://git.example/ops.git#main:rws/{node}.toml", "web-1").unwrap(),
        Source::Git { repo: "https://git.example/ops.git".into(), branch: "main".into(), path: "rws/web-1.toml".into() }
    );
    assert!(Source::parse("ftp://x/y", "n").is_err());
    assert!(Source::parse("git+https://x.git#-c:x", "n").is_err(), "a branch can't be an option");
    assert!(Source::parse("s3://bucket", "n").is_err());
}

#[test]
fn signatures_verify_only_with_the_right_key_and_body() {
    let (pair, public) = keypair();
    let s = sig(&pair, GOOD);
    assert!(verify(GOOD.as_bytes(), s.as_bytes(), &public).is_ok());
    assert!(verify(BAD.as_bytes(), s.as_bytes(), &public).is_err());
    let (_, other) = keypair();
    assert!(verify(GOOD.as_bytes(), s.as_bytes(), &other).is_err());
    assert!(verify(GOOD.as_bytes(), b"bm90IGEgc2lnbmF0dXJl", &public).is_err());
    assert_eq!(read_public_key(&Base64::encode(&public).unwrap()).unwrap(), public);
    assert!(read_public_key("c2hvcnQ=").unwrap_err().contains("32 bytes"));
}

#[test]
fn a_signed_config_is_applied_then_not_refetched_and_bad_ones_are_refused_and_reported() {
    let (pair, public) = keypair();
    let files = Arc::new(Mutex::new(HashMap::new()));
    let reports = Arc::new(Mutex::new(Vec::new()));
    files.lock().unwrap().insert("/web-1.toml".to_string(), (GOOD.to_string(), "\"v1\"".to_string()));
    files.lock().unwrap().insert("/web-1.toml.sig".to_string(), (sig(&pair, GOOD), "\"s1\"".to_string()));
    let base = server(Arc::clone(&files), Arc::clone(&reports));
    let path = TempPath::new("fleet-applied.toml");
    std::fs::write(&*path, "port = 7979\n").unwrap();
    let f = fleet(Source::Http(format!("{base}/web-1.toml")), Some(public), format!("{base}/report"), &path);
    let outcome = f.tick();
    assert!(matches!(outcome, Outcome::Applied(_)), "{outcome:?}");
    assert_eq!(std::fs::read_to_string(&*path).unwrap(), GOOD);
    assert_eq!(f.tick(), Outcome::Unchanged, "ETag: nothing refetched");
    assert!(f.status_json().contains(r#""error":null"#));

    // A tampered config (signature for the old one) is refused.
    files.lock().unwrap().insert("/web-1.toml".to_string(), (format!("{GOOD}# changed\n"), "\"v2\"".to_string()));
    let outcome = f.tick();
    assert!(matches!(&outcome, Outcome::Refused(e) if e.contains("signature")), "{outcome:?}");
    assert_eq!(std::fs::read_to_string(&*path).unwrap(), GOOD, "the running config stays");
    // A signed config that doesn't pass --check is refused too.
    files.lock().unwrap().insert("/web-1.toml".to_string(), (BAD.to_string(), "\"v3\"".to_string()));
    files.lock().unwrap().insert("/web-1.toml.sig".to_string(), (sig(&pair, BAD), "\"s3\"".to_string()));
    let outcome = f.tick();
    assert!(matches!(&outcome, Outcome::Refused(e) if e.contains("rws --check")), "{outcome:?}");
    assert!(f.status_json().contains("rws --check"));
    let reports = reports.lock().unwrap();
    assert!(reports.iter().any(|r| r.contains(r#""applied":true"#) && r.contains(r#""node":"web-1""#)), "{reports:?}");
    assert!(reports.iter().any(|r| r.contains(r#""applied":false"#) && r.contains("signature")), "{reports:?}");
}

#[test]
fn insecure_sources_need_no_signature() {
    let files = Arc::new(Mutex::new(HashMap::new()));
    files.lock().unwrap().insert("/c.toml".to_string(), (GOOD.to_string(), "\"a\"".to_string()));
    let base = server(files, Arc::new(Mutex::new(Vec::new())));
    let path = TempPath::new("fleet-insecure.toml");
    let f = fleet(Source::Http(format!("{base}/c.toml")), None, String::new(), &path);
    assert!(matches!(f.tick(), Outcome::Applied(_)));
}

#[test]
fn a_git_source_is_cloned_and_pulled() {
    if std::process::Command::new("git").arg("--version").output().is_err() {
        return;
    }
    let (pair, public) = keypair();
    let repo = TempPath::new("fleet-git-repo");
    std::fs::create_dir_all(repo.join("rws")).unwrap();
    let git = |args: &[&str]| assert!(std::process::Command::new("git").arg("-C").arg(repo.as_str()).args(args).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap().status.success());
    git(&["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("rws/web-1.toml"), GOOD).unwrap();
    std::fs::write(repo.join("rws/web-1.toml.sig"), sig(&pair, GOOD)).unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "one"]);
    let path = TempPath::new("fleet-git.toml");
    let f = fleet(Source::Git { repo: format!("file://{}", repo.as_str()), branch: "main".into(), path: "rws/web-1.toml".into() }, Some(public), String::new(), &path);
    assert!(matches!(f.tick(), Outcome::Applied(_)));
    let second = GOOD.replace("fleet", "fleet-2");
    std::fs::write(repo.join("rws/web-1.toml"), &second).unwrap();
    std::fs::write(repo.join("rws/web-1.toml.sig"), sig(&pair, &second)).unwrap();
    git(&["commit", "-q", "-am", "two"]);
    assert!(matches!(f.tick(), Outcome::Applied(_)));
    assert_eq!(std::fs::read_to_string(&*path).unwrap(), second);
    std::fs::remove_dir_all(&*repo).ok();
    std::fs::remove_dir_all(path.with_extension("state")).ok();
}

#[test]
fn keygen_and_sign_make_files_verify_accepts() {
    let dir = TempPath::new("fleet-keys");
    std::fs::create_dir_all(&*dir).unwrap();
    let prefix = dir.join("ops").to_string_lossy().to_string();
    let argv = |words: &[&str]| words.iter().map(|w| w.to_string()).collect::<Vec<_>>();
    assert_eq!(command(&argv(&["rws", "fleet", "keygen", &prefix])), Some(0));
    assert_eq!(command(&argv(&["rws", "fleet", "keygen", &prefix])), Some(1), "an existing key isn't overwritten");
    let config = dir.join("rws.toml");
    std::fs::write(&config, GOOD).unwrap();
    let key = format!("{prefix}.key");
    assert_eq!(command(&argv(&["rws", "fleet", "sign", config.to_str().unwrap(), "--key", &key])), Some(0));
    let public = read_public_key(&format!("{prefix}.pub")).unwrap();
    let signed = std::fs::read(format!("{}.sig", config.display())).unwrap();
    assert!(String::from_utf8_lossy(&signed).starts_with(SIG_PREFIX));
    let first = verify(GOOD.as_bytes(), &signed, &public).unwrap();
    assert!(first > 1_700_000_000, "serial defaults to now");
    assert_eq!(command(&argv(&["rws", "fleet", "sign", config.to_str().unwrap(), "--key", &key])), Some(0));
    assert!(verify(GOOD.as_bytes(), &std::fs::read(format!("{}.sig", config.display())).unwrap(), &public).unwrap() > first, "re-signing grows the serial");
    std::fs::write(&config, BAD).unwrap();
    assert_eq!(command(&argv(&["rws", "fleet", "sign", config.to_str().unwrap(), "--key", &key])), Some(1), "a config that fails --check isn't signed");
    assert_eq!(command(&argv(&["rws", "fleet"])), Some(2));
    assert_eq!(command(&argv(&["rws", "serve"])), None);
    std::fs::remove_dir_all(&*dir).ok();
}

fn sig_v2(pair: &aws_lc_rs::signature::Ed25519KeyPair, serial: u64, body: &str) -> String {
    format!("{SIG_PREFIX}{serial} {}", Base64::encode(pair.sign(&signed_message(serial, body.as_bytes())).as_ref()).unwrap())
}

#[test]
fn serials_refuse_rollback_and_survive_a_restart() {
    let (pair, public) = keypair();
    let v1 = GOOD.replace("fleet", "one");
    let v2 = GOOD.replace("fleet", "two");
    let files = Arc::new(Mutex::new(HashMap::new()));
    let put = |body: &str, sig: String, tag: &str| {
        files.lock().unwrap().insert("/c.toml".to_string(), (body.to_string(), format!("\"{tag}\"")));
        files.lock().unwrap().insert("/c.toml.sig".to_string(), (sig, format!("\"s{tag}\"")));
    };
    put(&v1, sig_v2(&pair, 10, &v1), "1");
    let base = server(Arc::clone(&files), Arc::new(Mutex::new(Vec::new())));
    let path = TempPath::new("fleet-serial.toml");
    let state = path.with_extension("state");
    let make = || fleet(Source::Http(format!("{base}/c.toml")), Some(public.clone()), String::new(), &path);
    let f = make();
    assert!(matches!(f.tick(), Outcome::Applied(_)));
    put(&v2, sig_v2(&pair, 11, &v2), "2");
    assert!(matches!(f.tick(), Outcome::Applied(_)));
    // The older, validly signed file again: refused, also after a restart.
    put(&v1, sig_v2(&pair, 10, &v1), "3");
    assert!(matches!(f.tick(), Outcome::Refused(e) if e.contains("serial 10")));
    let restarted = make();
    assert!(matches!(restarted.tick(), Outcome::Refused(e) if e.contains("isn't above the applied 11")));
    assert_eq!(std::fs::read_to_string(&*path).unwrap(), v2);
    // A serial changed after signing doesn't verify.
    let forged = sig_v2(&pair, 10, &v1).replacen(" 10 ", " 99 ", 1);
    assert!(verify(v1.as_bytes(), forged.as_bytes(), &public).is_err());
    std::fs::remove_dir_all(&state).ok();
}

#[test]
fn a_fetched_config_cant_change_the_fleet_settings() {
    let (pair, public) = keypair();
    let local = "port = 7979\n[fleet]\nsource = \"https://cfg.example/c.toml\"\npublic_key = \"x\"\n";
    let fetched = "port = 7979\n[fleet]\nsource = \"https://cfg.example/c.toml\"\npublic_key = \"x\"\ninsecure = true\n";
    let files = Arc::new(Mutex::new(HashMap::new()));
    files.lock().unwrap().insert("/c.toml".to_string(), (fetched.to_string(), "\"a\"".to_string()));
    files.lock().unwrap().insert("/c.toml.sig".to_string(), (sig_v2(&pair, 5, fetched), "\"b\"".to_string()));
    let base = server(files, Arc::new(Mutex::new(Vec::new())));
    let path = TempPath::new("fleet-settings.toml");
    std::fs::write(&*path, local).unwrap();
    let f = fleet(Source::Http(format!("{base}/c.toml")), Some(public), String::new(), &path);
    assert!(matches!(f.tick(), Outcome::Refused(e) if e.contains("[fleet] settings")));
    assert_eq!(std::fs::read_to_string(&*path).unwrap(), local);
}

#[test]
fn signature_files_parse() {
    assert_eq!(parse_signature(b"rws-fleet-sig v2 x AAAA").unwrap_err(), "the signature's serial \"x\" isn't a number");
    assert!(parse_signature(b"rws-fleet-sig v2 7").is_err());
    let (pair, _) = keypair();
    assert_eq!(parse_signature(sig_v2(&pair, 7, "a").as_bytes()).unwrap().0, 7);
    assert_eq!(parse_signature(sig(&pair, "a").as_bytes()).unwrap().0, 0, "18.23 format: serial 0");
}
