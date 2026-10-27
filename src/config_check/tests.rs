use super::*;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn has(findings: &[String], needle: &str) -> bool {
    findings.iter().any(|f| f.contains(needle))
}

#[test]
fn check_target_from_args_forms() {
    // default_config_path() reads RWS_CONFIG_FILE, which other tests set.
    let _g = crate::test_env::lock();
    assert_eq!(check_target_from_args(&args(&["rws"])), None);
    assert_eq!(check_target_from_args(&args(&["rws", "--port=80"])), None);
    assert_eq!(check_target_from_args(&args(&["rws", "--check=/etc/a.toml"])), Some("/etc/a.toml".to_string()));
    assert_eq!(check_target_from_args(&args(&["rws", "--check", "b.toml"])), Some("b.toml".to_string()));
    // A following flag is not a path: check the default file.
    assert_eq!(check_target_from_args(&args(&["rws", "--check", "--port=80"])), Some(default_config_path()));
}

#[test]
fn a_valid_proxy_config_is_ok() {
    let report = check_str(r#"
port       = 8080
log_format = "json"

[cors]
allow_all = true

[[upstream]]
name     = "app"
backends = ["localhost:3000", "http://10.0.0.2:8080"]
strategy = "least_connections"
  [upstream.health_check]
  path = "/healthz"

[[route]]
  [route.match]
  path = "/api/*"
  [route.action]
  type     = "proxy"
  upstream = "app"
  [route.middleware]
  rate_limit = { max_requests = 500, window_secs = 60 }
  ip_filter  = { allow = ["10.0.0.0/8", "2001:db8::/32"] }

[[route]]
  [route.match]
  path = "/old"
  [route.action]
  type     = "redirect"
  location = "/new"
  status   = 308
"#);
    assert!(report.is_ok(), "{:?}", report);
    assert!(report.warnings.is_empty(), "{:?}", report);
}

#[test]
fn setting_mistakes_are_errors() {
    let report = check_str(r#"
port = 70000
ip = "localhost"
thread_cuont = 4
log_format = "pretty"
max_body_size_in_bytes = "10MB"

[cors]
allow_all = "yes"
"#);
    assert!(!report.is_ok());
    for needle in ["thread_cuont", "port = \"70000\"", "ip = \"localhost\"", "log_format", "max_body_size_in_bytes", "cors_allow_all"] {
        assert!(has(&report.errors, needle), "missing {needle}: {:?}", report.errors);
    }
}

#[test]
fn runtime_is_shared_or_per_core() {
    assert!(check_str("runtime = \"per-core\"\n").errors.is_empty());
    assert!(check_str("runtime = \"shared\"\n").errors.is_empty());
    let report = check_str("runtime = \"percore\"\n");
    assert!(has(&report.errors, "runtime"), "{:?}", report.errors);
}

#[test]
fn tls_files_must_come_in_pairs_and_exist() {
    let report = check_str("tls_cert_file = \"cert.pem\"\n");
    assert!(has(&report.errors, "tls_key_file is not"), "{:?}", report.errors);

    let report = check_str("tls_cert_file = \"/nonexistent/cert.pem\"\ntls_key_file = \"/nonexistent/key.pem\"\n");
    assert!(has(&report.errors, "certificate file '/nonexistent/cert.pem' not found"), "{:?}", report.errors);
}

#[cfg(feature = "http2")]
#[test]
fn tls_files_that_are_not_pem_are_errors() {
    let cert = crate::test_env::TempPath::new("check_cert.pem");
    let key = crate::test_env::TempPath::new("check_key.pem");
    std::fs::write(cert.as_str(), "not a certificate").unwrap();
    std::fs::write(key.as_str(), "not a key").unwrap();
    let report = check_str(&format!("tls_cert_file = \"{}\"\ntls_key_file = \"{}\"\n", cert.as_str(), key.as_str()));
    assert!(!report.is_ok(), "{:?}", report);
}

#[test]
fn acme_checks() {
    let report = check_str(r#"
port = 443
http_redirect_port = 80

[acme]
domains = ["example.com", "*.example.com", "localhost"]
"#);
    assert!(has(&report.errors, "the wildcard '*.example.com' needs challenge = \"dns-01\""), "{:?}", report.errors);
    assert!(has(&report.errors, "'localhost' is not a domain name"), "{:?}", report.errors);
    assert!(has(&report.warnings, "no email"), "{:?}", report.warnings);
    assert!(!has(&report.warnings, "http_redirect_port"), "{:?}", report.warnings);

    let report = check_str("[acme]\ndomains = [\"example.com\"]\nemail = \"a@example.com\"\n");
    assert!(has(&report.warnings, "set http_redirect_port = 80"), "{:?}", report.warnings);
}

#[test]
fn proxy_mistakes_are_reported() {
    let report = check_str(r#"
[[upstream]]
name     = "app"
backends = ["localhost"]
strategy = "fastest"

[[upstream]]
name     = "app"
backends = []

[[route]]
  [route.action]
  type    = "proxy"
  upsteam = "app"

[[route]]
  [route.action]
  type     = "proxy"
  upstream = "missing"

[[route]]
  [route.action]
  type = "redirct"

[[route]]
  [route.match]
  path = "/r"
  [route.action]
  type   = "redirect"
  status = 200

[[route]]
  [route.match]
  path = "/x"
  [route.action]
  type = "respond"
  [route.middleware]
  ip_filter  = { deny = ["intranet"] }
  rate_limit = { max_requests = 0 }
  [route.middlewares]
  timeout_ms = 5
"#);
    for needle in [
        "backend 'localhost' is not host:port",
        "unknown strategy 'fastest'",
        "declared more than once",
        "`backends` is empty",
        "[route.action] upsteam: unknown key",
        "proxy action has no `upstream`",
        "upstream 'missing' is not a declared [[upstream]]",
        "unknown action type 'redirct'",
        "redirect status 200 is not a 3xx code",
        "ip_filter entry 'intranet'",
        "rate_limit needs max_requests",
        "[route.middlewares]: unknown table",
    ] {
        assert!(has(&report.errors, needle), "missing {needle:?}: {:#?}", report.errors);
    }
    assert!(has(&report.warnings, "matches every request"), "{:?}", report.warnings);
}

#[test]
fn undeclared_upstream_that_is_an_address_is_a_warning() {
    let report = check_str(r#"
[[route]]
  [route.action]
  type     = "proxy"
  upstream = "localhost:3000"
"#);
    assert!(report.is_ok(), "{:?}", report);
    assert!(has(&report.warnings, "not a declared [[upstream]]"), "{:?}", report.warnings);
}

#[test]
fn auth_secret_that_is_not_set_is_an_error() {
    let report = check_str(r#"
[[route]]
  [route.action]
  type = "respond"
  [route.middleware]
  auth = { type = "bearer", token_env = "RWSTEST_CHECK_NEVER_SET_TOKEN" }
"#);
    assert!(has(&report.errors, "'RWSTEST_CHECK_NEVER_SET_TOKEN' is unset"), "{:?}", report.errors);
}

#[test]
fn unreadable_file_is_an_error() {
    let report = check_file("/nonexistent/rws.config.toml");
    assert!(!report.is_ok());
    assert!(report.render("/nonexistent/rws.config.toml").contains("1 error"));
}

#[test]
fn render_summarizes() {
    let mut report = ConfigReport::default();
    assert_eq!(report.render("f.toml"), "f.toml: configuration OK\n");
    report.warn("w");
    assert_eq!(report.render("f.toml"), "warning: w\nf.toml: configuration OK (1 warning)\n");
    report.error("e");
    assert_eq!(report.render("f.toml"), "error: e\nwarning: w\nf.toml: 1 error, 1 warning\n");
}

// issue #26: three more checks catch real mistakes.

#[test]
fn a_missing_spa_fallback_file_is_a_warning() {
    let report = check_str(r#"
spa_fallback = "does-not-exist.html"
"#);
    assert!(has(&report.warnings, "spa_fallback"), "{report:?}");

    let existing = crate::test_env::TempPath::new("check-fallback-ok.html");
    std::fs::write(existing.as_str(), "<html></html>").unwrap();
    let report = check_str(&format!(r#"spa_fallback = "{}""#, existing.as_str()));
    assert!(!has(&report.warnings, "spa_fallback"), "{report:?}");
}

#[test]
fn a_cors_origin_with_a_path_or_no_scheme_is_a_warning() {
    let report = check_str(r#"
[cors]
allow_origins = "https://ok.example.com, example.com, https://bad.example.com/, https://x.example.com/path"
"#);
    assert!(!has(&report.warnings, "ok.example.com"), "{report:?}");
    assert!(has(&report.warnings, "does not look like"), "{report:?}");
    assert!(has(&report.warnings, "trailing slash"), "{report:?}");
    assert!(has(&report.warnings, "path"), "{report:?}");
}

#[test]
fn two_virtual_hosts_with_the_same_domain_are_an_error() {
    let report = check_str(r#"
[[virtual_host]]
domain = "same.example.com"
cert_file = "/tmp/a.pem"
key_file = "/tmp/a.key"

[[virtual_host]]
domain = "same.example.com"
cert_file = "/tmp/b.pem"
key_file = "/tmp/b.key"
"#);
    assert!(has(&report.errors, "same domain"), "{report:?}");
}

// issue #25: --format=json gives CI and editors one machine-readable object.

#[test]
fn wants_json_output_reads_the_format_flag() {
    assert!(wants_json_output(&args(&["rws", "--check", "--format=json"])));
    assert!(!wants_json_output(&args(&["rws", "--check"])));
}

#[test]
fn render_json_carries_path_ok_and_both_lists() {
    let mut report = ConfigReport::default();
    report.error("route 'api': upstream 'x' is not a declared [[upstream]]");
    report.warn("spa_fallback = \"missing.html\": the file does not exist");

    let json = report.render_json("rws.config.toml");
    assert!(json.starts_with('{') && json.ends_with('}'), "{json}");
    assert!(json.contains(r#""path":"rws.config.toml""#), "{json}");
    assert!(json.contains(r#""ok":false"#), "{json}");
    assert!(json.contains(r#""errors":["route 'api': upstream 'x' is not a declared [[upstream]]"]"#), "{json}");
    assert!(json.contains(r#""warnings":["#), "{json}");
    // the report itself is still renderable as text
    assert!(report.render("rws.config.toml").starts_with("error: "));
}

#[test]
fn render_json_escapes_quotes_and_stays_valid_looking() {
    let mut report = ConfigReport::default();
    report.error(r#"port = "nope": not a port number"#);
    let json = report.render_json("p.toml");
    assert!(json.contains(r#"\"nope\""#), "{json}");
    let ok = ConfigReport::default().render_json("p.toml");
    assert!(ok.contains(r#""ok":true"#) && ok.contains(r#""errors":[]"#), "{ok}");
}

#[cfg(feature = "acme")]
#[test]
fn acme_on_demand_checks() {
    let base = "port = 443\nhttp_redirect_port = 80\n[acme]\ndomains = [\"saas.example\"]\nemail = \"a@example.com\"\n";
    let report = check_str(&format!("{base}on_demand = true\non_demand_ask = \"http://127.0.0.1:9000/allowed\"\n"));
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    let report = check_str(&format!("{base}on_demand = true\n"));
    assert!(report.errors.iter().any(|e| e.contains("on_demand_ask")), "{:?}", report.errors);
    let report = check_str(&format!("{base}on_demand = true\non_demand_ask = \"127.0.0.1:9000\"\n"));
    assert!(report.errors.iter().any(|e| e.contains("http://")), "{:?}", report.errors);
    let report = check_str(&format!("{base}on_demand = yes\n"));
    assert!(report.errors.iter().any(|e| e.contains("true or false")), "{:?}", report.errors);
}

#[cfg(feature = "acme")]
#[test]
fn acme_challenge_checks() {
    let report = check_str("port = 443\n[acme]\ndomains = [\"example.com\"]\nemail = \"a@example.com\"\nchallenge = \"tls-alpn-01\"\n");
    assert!(report.is_ok(), "{:?}", report);
    assert!(!has(&report.warnings, "http_redirect_port"), "TLS-ALPN-01 doesn't need port 80: {:?}", report.warnings);

    let report = check_str("[acme]\ndomains = [\"example.com\"]\nchallenge = \"dns-02\"\n");
    assert!(has(&report.errors, "expected http-01, tls-alpn-01, dns-01 or dns-persist-01"), "{:?}", report.errors);

    let dns = |extra: &str| check_str(&format!("port = 443\n[acme]\ndomains = [\"example.com\", \"*.example.com\"]\nemail = \"a@example.com\"\nchallenge = \"dns-01\"\n{extra}"));
    assert!(has(&dns("").errors, "needs dns_provider"), "{:?}", dns("").errors);
    assert!(has(&dns("dns_provider = \"rfc2136\"\n").errors, "needs dns_server"));
    let report = dns("dns_provider = \"rfc2136\"\ndns_server = \"ns1.example.com\"\ndns_tsig_key = \"k\"\n");
    assert!(has(&report.errors, "go together"), "{:?}", report.errors);
    let report = dns("dns_provider = \"exec\"\ndns_exec = \"/nonexistent/hook\"\n");
    assert!(has(&report.errors, "file not found"), "{:?}", report.errors);
    let report = dns("dns_provider = \"rfc2136\"\ndns_server = \"ns1.example.com:5353\"\ndns_tsig_key = \"k\"\ndns_tsig_secret = \"c2VjcmV0\"\n");
    assert!(report.errors.is_empty(), "a wildcard with DNS-01 is fine: {:?}", report.errors);

    assert!(has(&dns("dns_provider = \"cloudflare\"\n").errors, "needs dns_api_token"));
    let report = dns("dns_provider = \"cloudflare\"\ndns_api_token = \"t\"\ndns_api_url = \"api.example\"\n");
    assert!(has(&report.errors, "dns_api_url"), "{:?}", report.errors);
    let report = dns("dns_provider = \"cloudflare\"\ndns_api_token = \"t\"\ndns_zone_id = \"abc\"\n");
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert!(dns("dns_provider = \"route53\"\n").errors.is_empty());
    assert!(has(&dns("dns_provider = \"gandi\"\n").errors, "expected rfc2136, cloudflare, route53 or exec"));
}

#[cfg(feature = "acme")]
#[test]
fn acme_dns_persist_checks() {
    let persist = |extra: &str| check_str(&format!("port = 443\n[acme]\ndomains = [\"example.com\", \"*.example.com\"]\nemail = \"a@example.com\"\nchallenge = \"dns-persist-01\"\n{extra}"));
    assert!(persist("").is_ok(), "no provider needed, wildcards fine: {:?}", persist(""));
    assert!(has(&persist("dns_provider = \"rfc2136\"\n").warnings, "ignored"));
    let report = persist("on_demand = true\non_demand_ask = \"http://127.0.0.1:9000/\"\n");
    assert!(has(&report.errors, "not dns-persist-01"), "{:?}", report.errors);
}
