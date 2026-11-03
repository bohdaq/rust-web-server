//! Tests of `config_check` (a split crate) that need the facade or repository fixtures:
//! a crate's tests run in the crate's directory, without the crates above it.
#![allow(unused_imports, dead_code)]

use crate::config_check::*;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn has(findings: &[String], needle: &str) -> bool {
    findings.iter().any(|f| f.contains(needle))
}

const MACHINE_DEPENDENT: &str = r#"tls_cert_file = "/nonexistent/cert.pem"
tls_key_file  = "/nonexistent/key.pem"
spa_fallback  = "nonexistent-index.html"

[[upstream]]
name        = "app"
backends    = ["10.0.0.10:8080"]  # http1-only builds reject https:// backends
tls_ca_file = "/nonexistent/ca.pem"

[[route]]
  [route.match]
  path = "/api/*"
  [route.action]
  type     = "proxy"
  upstream = "app"
  [route.middleware]
  auth = { type = "bearer", token_env = "RWS_TEST_OFFLINE_TOKEN_UNSET" }

[[route]]
  [route.match]
  path = "/files/*"
  [route.action]
  type = "static"
  [route.action.static]
  root = "/nonexistent/www"
"#;

/// Every TOML block on the cookbook pages is a complete configuration that
/// `rws --check` accepts (offline: files, env vars and the network aren't
/// looked at). Recipes stay working as settings change (#74). Needs the
/// features the recipes use; CI's feature-set run has them.
#[cfg(all(feature = "acme", feature = "auth", feature = "sso"))]
#[test]
fn cookbook_configs_pass_check() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/src/content/docs/cookbook");
    let mut pages: Vec<_> = std::fs::read_dir(&dir).expect("cookbook pages").filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "md")).collect();
    pages.sort();
    let mut checked = 0;
    for page in pages {
        let text = std::fs::read_to_string(&page).unwrap();
        let mut rest = text.as_str();
        while let Some(start) = rest.find("```toml\n") {
            let body = &rest[start + 8..];
            let end = body.find("\n```").expect("closed block");
            let block = &body[..end];
            let report = crate::config_check::check_str_offline(block);
            assert!(report.is_ok() && report.warnings.iter().all(|w| !w.contains("unknown")), "{}:\n{block}\n{report:?}", page.display());
            checked += 1;
            rest = &body[end..];
        }
    }
    assert!(checked >= 8, "only {checked} cookbook configs");
}
