use super::*;
use crate::admin::AdminConfig;
use crate::test_env::TempPath;
use std::sync::OnceLock;

const TOKEN: &str = "ctl-test-token";

/// One admin API on a free port for every test: (base URL, token file).
fn server() -> &'static (String, String, TempPath) {
    static SERVER: OnceLock<(String, String, TempPath)> = OnceLock::new();
    SERVER.get_or_init(|| {
        std::env::set_var("RWS_TEST_CTL_TOKEN", TOKEN);
        let config = AdminConfig { listen: "127.0.0.1:0".to_string(), token_env: "RWS_TEST_CTL_TOKEN".to_string(), ..AdminConfig::default() };
        let addr = crate::admin::start(&config).expect("admin API").expect("listening");
        let file = TempPath::new("rws-ctl-test-token");
        std::fs::write(&*file, format!("{TOKEN}\n")).unwrap();
        (format!("http://{addr}"), file.as_str().to_string(), file)
    })
}

/// `rws ctl --admin … --token-file … <words>` → (exit code, stdout, stderr).
fn ctl(words: &[&str]) -> (i32, String, String) {
    let (admin, token_file, _) = server();
    let mut args = vec!["--admin".to_string(), admin.clone(), format!("--token-file={token_file}")];
    args.extend(words.iter().map(|w| w.to_string()));
    run_args(&args)
}

fn run_args(args: &[String]) -> (i32, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(args, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn strings(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

#[test]
fn ctl_is_requested_by_subcommand_or_program_name() {
    assert_eq!(requested(&strings(&["rws", "ctl", "status"])), Some(&strings(&["status"])[..]));
    assert_eq!(requested(&strings(&["/usr/bin/rwsctl", "reload"])), Some(&strings(&["reload"])[..]));
    assert_eq!(requested(&strings(&["C:\\rws\\rwsctl.exe"])), Some(&[][..]));
    assert_eq!(requested(&strings(&["rws", "--port=80"])), None);
    assert_eq!(requested(&strings(&["rws"])), None);
    assert!(!invoked_as_rwsctl("/usr/bin/rws"));
}

#[test]
fn usage_errors_exit_2() {
    let (code, out, _) = run_args(&[]);
    assert_eq!(code, EXIT_UNUSABLE);
    assert!(out.contains("Commands:"));
    let (code, out, _) = run_args(&strings(&["help"]));
    assert_eq!((code, out.contains("Exit codes")), (EXIT_OK, true));
    let (code, _, err) = run_args(&strings(&["--token", "secret", "status"]));
    assert_eq!(code, EXIT_UNUSABLE);
    assert!(err.contains("never the command line"), "{err}");
    let (code, _, err) = run_args(&strings(&["--bogus", "status"]));
    assert_eq!(code, EXIT_UNUSABLE);
    assert!(err.contains("unknown option --bogus"), "{err}");
    let (code, _, err) = ctl(&["frobnicate"]);
    assert_eq!(code, EXIT_UNUSABLE);
    assert!(err.contains("unknown command: frobnicate"), "{err}");
}

#[test]
fn unreachable_and_unauthorized_exit_2() {
    let (code, _, err) = run_args(&strings(&["--admin", "127.0.0.1:1", "--token-file", server().1.as_str(), "status"]));
    assert_eq!(code, EXIT_UNUSABLE);
    assert!(err.contains("can't reach the admin API"), "{err}");

    let wrong = TempPath::new("rws-ctl-wrong-token");
    std::fs::write(&*wrong, "nope").unwrap();
    let (code, _, err) = run_args(&strings(&["--admin", server().0.as_str(), "--token-file", wrong.as_str(), "status"]));
    assert_eq!(code, EXIT_UNUSABLE);
    assert!(err.contains("refused the token (401)"), "{err}");

    let (code, _, err) = run_args(&strings(&["--admin", server().0.as_str(), "--token-file", "/nonexistent/rws-token", "status"]));
    assert_eq!(code, EXIT_UNUSABLE);
    assert!(err.contains("can't read the token file"), "{err}");

    let (code, _, err) = run_args(&strings(&["--admin", "https://127.0.0.1:9901", "--token-file", server().1.as_str(), "status"]));
    assert_eq!(code, EXIT_UNUSABLE);
    assert!(err.contains("plain HTTP"), "{err}");
}

#[test]
fn status_for_people_and_scripts() {
    let (code, out, err) = ctl(&["status"]);
    assert_eq!(code, EXIT_OK, "{err}");
    assert!(out.starts_with(&format!("rws {}  pid {}", env!("CARGO_PKG_VERSION"), std::process::id())), "{out}");
    assert!(out.contains("\nadmin     127.0.0.1:"), "{out}");
    assert!(out.contains("\nfeatures  "), "{out}");

    let (code, out, _) = ctl(&["--json", "status"]);
    assert_eq!(code, EXIT_OK);
    assert!(out.trim().starts_with('{') && out.contains(r#""version":""#), "{out}");
}

#[test]
fn maintenance_and_blocklist_change_the_server() {
    let _g = crate::test_env::lock();
    assert_eq!(ctl(&["maintenance", "on"]), (EXIT_OK, "maintenance on\n".to_string(), String::new()));
    assert!(crate::maintenance::MAINTENANCE_MODE.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(ctl(&["maintenance"]).1, "maintenance on\n");
    assert_eq!(ctl(&["maintenance", "off"]).1, "maintenance off\n");
    assert_eq!(ctl(&["maintenance", "maybe"]).0, EXIT_UNUSABLE);

    assert_eq!(ctl(&["block", "203.0.113.77"]).1, "blocked 203.0.113.77\n");
    assert!(ctl(&["blocklist"]).1.lines().any(|l| l == "203.0.113.77"));
    assert_eq!(ctl(&["unblock", "203.0.113.77"]).1, "unblocked 203.0.113.77\n");
    assert!(!ctl(&["blocklist"]).1.contains("203.0.113.77"));

    let (code, _, err) = ctl(&["block", "not-an-ip"]);
    assert_eq!(code, EXIT_FAILED);
    assert!(err.contains(r#"expected {"ip":"#), "{err}");
}

#[test]
fn refusals_exit_1_with_the_servers_reason() {
    let (code, _, err) = ctl(&["purge", "no-such-cache", "--prefix", "/a/"]);
    assert_eq!(code, EXIT_FAILED);
    assert!(err.contains(r#"no cache named "no-such-cache""#), "{err}");
    let (code, _, err) = ctl(&["drain", "no-such-upstream", "http://10.0.0.5:8080"]);
    assert_eq!(code, EXIT_FAILED);
    assert!(err.contains(r#"no upstream "no-such-upstream""#), "{err}");
}

#[test]
fn listings_and_certificates() {
    let (code, out, err) = ctl(&["certs"]);
    assert_eq!(code, EXIT_OK, "{err}");
    assert!(out.contains("ACME renewal: "), "{out}");
    assert_eq!(ctl(&["logs", "-n", "5"]).0, EXIT_OK);
    assert_eq!(ctl(&["--json", "logs"]).1.trim().chars().next(), Some('['));
    assert_eq!(ctl(&["upstreams"]).0, EXIT_OK);
    assert_eq!(ctl(&["caches"]).0, EXIT_OK);
}

#[test]
fn the_token_is_never_in_the_arguments_and_wildcards_mean_loopback() {
    let cmd = Command { admin: Some("0.0.0.0:9901".to_string()), ..Command::default() };
    assert_eq!(admin_base(&cmd), "http://127.0.0.1:9901");
    let cmd = Command { admin: Some("[::]:9901".to_string()), ..Command::default() };
    assert_eq!(admin_base(&cmd), "http://[::1]:9901");
    let cmd = Command { admin: Some("http://10.0.0.2:9901/".to_string()), ..Command::default() };
    assert_eq!(admin_base(&cmd), "http://10.0.0.2:9901");
}

#[test]
fn flags_parse_inline_and_separate() {
    let cmd = parse(&strings(&["traffic", "--week", "--host=example.com"])).unwrap();
    assert_eq!((cmd.hours, cmd.host.as_deref()), (Some(168), Some("example.com")));
    let cmd = parse(&strings(&["logs", "-f", "-n", "20"])).unwrap();
    assert!(cmd.follow);
    assert_eq!(cmd.lines, Some(20));
    assert!(parse(&strings(&["logs", "-n", "many"])).is_err());
    assert!(parse(&strings(&["purge", "app", "--path"])).is_err());
}

#[test]
fn chunked_bodies_are_decoded() {
    let raw = b"5\r\nhello\r\n7;ext=1\r\n, world\r\n0\r\n\r\n";
    let mut text = String::new();
    Dechunk::new(&raw[..]).read_to_string(&mut text).unwrap();
    assert_eq!(text, "hello, world");
    assert!(Dechunk::new(&b"zz\r\n"[..]).read_to_string(&mut String::new()).is_err());
}

#[test]
fn helpers_format_for_people() {
    assert_eq!(duration(59), "59s");
    assert_eq!(duration(3_725), "1h 2m");
    assert_eq!(duration(90_000), "1d 1h");
    assert_eq!(bytes(512), "512 B");
    assert_eq!(bytes(1536), "1.5 KiB");
    assert_eq!(percent_encode("route:web/a b"), "route%3Aweb%2Fa%20b");
    let mut out = Vec::new();
    table(&mut out, &["A", "LONG"], &[vec!["xyz".to_string(), "1".to_string()]]).unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), "A    LONG\nxyz  1\n");
    assert_eq!(text(r#""a\"b""#), "a\"b");
    assert_eq!(text("true"), "true");
}

#[test]
fn traffic_prints_totals_and_top_pages() {
    let body = r#"{"hours":24,"host":null,"totals":{"people":{"requests":10,"pages":4,"visitors":3},"ai":{"requests":2,"pages":2,"visitors":1},"bots":{"requests":5,"pages":0,"visitors":2}},"people_pages":[["/",3],["/docs",1]]}"#;
    let mut out = Vec::new();
    print_traffic(&mut out, body).unwrap();
    let out = String::from_utf8(out).unwrap();
    assert!(out.starts_with("last 24 h\n"), "{out}");
    assert!(out.contains("people     10        4      3"), "{out}");
    assert!(out.contains("top pages\n         3  /\n"), "{out}");
}

#[test]
fn whoami_and_audit() {
    let _g = crate::test_env::lock();
    assert_eq!(ctl(&["whoami"]).1, "admin (admin, via token)\n");
    ctl(&["maintenance", "off"]);
    let (code, out, err) = ctl(&["audit", "-n", "5"]);
    assert_eq!(code, EXIT_OK, "{err}");
    assert!(out.starts_with("TIME") && out.contains("admin (admin)") && out.contains("PUT /maintenance"), "{out}");
}
