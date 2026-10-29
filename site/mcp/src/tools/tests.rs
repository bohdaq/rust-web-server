use super::*;

fn text(c: Result<McpContent, String>) -> String {
    format!("{:?}", c.expect("tool result"))
}

#[test]
fn search_finds_the_section_about_path_parameters() {
    let docs = fixture();
    let out = text(search(&docs, r#"{"query":"route path parameter"}"#));
    assert!(out.contains("Routing › Path parameters"), "{out}");
    assert!(out.contains("building-apps/routing/#path-parameters"), "{out}");
}

#[test]
fn search_and_pages_follow_the_requested_version() {
    let docs = fixture();
    assert!(text(get_page(&docs, r#"{"path":"/building-apps/routing/","version":"18.6.0"}"#)).contains("Old routing page"));
    assert!(text(get_page(&docs, r#"{"path":"https://rws8.tech/docs/18.8/building-apps/routing/"}"#)).contains("PathParams"));
    let err = get_page(&docs, r#"{"path":"routing","version":"17.1"}"#).unwrap_err();
    assert!(err.contains("no docs for 17.1") && err.contains("18.8"), "{err}");
}

#[test]
fn a_missing_page_suggests_close_paths() {
    let err = get_page(&fixture(), r#"{"path":"routing"}"#).unwrap_err();
    assert!(err.contains("building-apps/routing"), "{err}");
}

#[test]
fn settings_resolve_from_env_names_keys_and_flags() {
    let docs = fixture();
    for name in ["RWS_CONFIG_PORT", "port", "--port"] {
        let out = text(setting(&docs, &format!(r#"{{"name":"{name}"}}"#)));
        assert!(out.contains("RWS_CONFIG_PORT") && out.contains("7878"), "{name}: {out}");
    }
    assert!(text(setting(&docs, r#"{"name":"[acme] domains"}"#)).contains("RWS_CONFIG_ACME_DOMAINS"));
    assert!(setting(&docs, r#"{"name":"nope"}"#).is_err());
}

#[test]
fn api_items_and_examples() {
    let docs = fixture();
    let out = text(api_item(&docs, r#"{"name":"rust_web_server::state::AppWithState"}"#));
    assert!(out.contains("module: `state`") && out.contains("docs.rs/rust-web-server/18.8.3"), "{out}");
    assert!(text(get_example(&docs, r#"{"name":"examples/hello.rs"}"#)).contains("cargo run --example hello"));
    assert!(get_example(&docs, r#"{"name":"nope"}"#).unwrap_err().contains("hello"));
}

#[test]
fn changes_list_breaking_changes_first_and_respect_the_range() {
    let docs = fixture();
    let out = text(changes(&docs, r#"{"since":"18.6.0"}"#));
    let breaking = out.find("Breaking changes").expect("breaking block");
    assert!(breaking < out.find("## 18.8.3").unwrap(), "{out}");
    assert!(out.contains("`Header` values are `Cow`") && !out.contains("Native histograms"), "{out}");
    let only = text(changes(&docs, r#"{"since":"18.7.0","until":"18.8.3"}"#));
    assert!(only.contains("18.8.3") && !only.contains("Starters"), "{only}");
    assert!(changes(&docs, r#"{"since":"soon"}"#).is_err());
}

#[test]
fn check_config_reports_mistakes_without_looking_at_this_machine() {
    let bad = text(check_config(r#"{"toml":"[[upstream]]\nname = \"app\"\nbackends = [\"10.0.0.1:80\"]\nstrategy = \"fastest\"\n"}"#));
    assert!(bad.contains("Not OK") && bad.contains("unknown strategy"), "{bad}");
    let files = text(check_config(r#"{"toml":"tls_cert_file = \"/etc/shadow\"\ntls_key_file = \"/root/key.pem\"\n"}"#));
    assert!(files.contains("OK: 0 error(s)"), "{files}");
    let big = format!(r#"{{"toml":"{}"}}"#, "#".repeat(MAX_CONFIG_BYTES + 1));
    assert!(check_config(&big).unwrap_err().contains("up to"));
}

#[test]
fn versions_lists_releases_and_page_paths() {
    let out = versions(&fixture());
    assert!(out.contains("18.8, 18.6") && out.contains("`features/acme`"), "{out}");
}

#[test]
fn plurals_and_common_words_do_not_change_the_answer() {
    let docs = fixture();
    let a = text(search(&docs, r#"{"query":"how do I add a route with path parameters","limit":1}"#));
    assert!(a.contains("Path parameters"), "{a}");
}
