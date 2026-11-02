use file_ext::FileExt;
use crate::app::controller::static_resource::StaticResourceController;
use crate::controller::Controller;
use crate::core::New;
use crate::header::Header;
use crate::http::VERSION;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::{Address, ConnectionInfo};

#[test]
fn not_found() {
    // is_matching's result now depends on RWS_CONFIG_SPA_FALLBACK (unset here,
    // but the SPA-fallback tests below toggle it) — take the lock so this
    // never observes a state left mid-flight by one of those.
    let _g = crate::test_env::lock();
    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK");
    let path = "/not_found";

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: path.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: Default::default(),
    };

    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    sni_hostname: None,
    tls: false,
    };

    let is_matching = StaticResourceController::is_matching(&request, &connection_info);
    assert!(!is_matching);

    let mut response = Response::new();
    response = StaticResourceController::process(&request, response, &connection_info);

    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n501_not_implemented.status_code);
    assert_eq!(response.reason_phrase, STATUS_CODE_REASON_PHRASE.n501_not_implemented.reason_phrase);
}

#[test]
fn malformed() {
    let _g = crate::test_env::lock();
    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK");
    let path = "//randomtext";

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: path.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: Default::default(),
    };

    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    sni_hostname: None,
    tls: false,
    };

    let is_matching = StaticResourceController::is_matching(&request, &connection_info);
    assert!(!is_matching);

    let mut response = Response::new();
    response = StaticResourceController::process(&request, response, &connection_info);

    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n501_not_implemented.status_code);
    assert_eq!(response.reason_phrase, STATUS_CODE_REASON_PHRASE.n501_not_implemented.reason_phrase);
}

#[test]
fn directory_listing_on_empty_or_missing_directory_still_renders_a_valid_page() {
    let html = StaticResourceController::render_directory_listing(
        "/nonexistent-directory-for-html-escaping-test",
        "/no_index",
    );

    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("Index of /no_index/"));
    assert!(html.contains("0 items"));
}

#[test]
fn directory_listing_has_parent_link_except_at_static_root() {
    let nested_html = StaticResourceController::render_directory_listing(
        "/nonexistent-directory-for-parent-link-test",
        "/no_index/nested",
    );
    assert!(nested_html.contains(".. (parent directory)"));
    assert!(nested_html.contains("href=\"/no_index/\""));
}

// ── SPA fallback (RWS_CONFIG_SPA_FALLBACK) ────────────────────────────────────

#[test]
fn path_has_extension_checks_only_the_last_segment() {
    assert!(!super::path_has_extension("/dashboard/settings"));
    assert!(!super::path_has_extension("/user.name/settings"), "dot in a non-last segment shouldn't count");
    assert!(super::path_has_extension("/assets/logo.png"));
    assert!(super::path_has_extension("/app.js"));
    assert!(!super::path_has_extension("/"));
}

fn spa_request(path: &str) -> (Request, ConnectionInfo) {
    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: path.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: Default::default(),
    };
    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
        sni_hostname: None,
        tls: false,
    };
    (request, connection_info)
}

#[test]
fn spa_fallback_disabled_by_default() {
    let _g = crate::test_env::lock();
    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK");

    let (request, connection_info) = spa_request("/dashboard/settings");
    assert!(!StaticResourceController::is_matching(&request, &connection_info));
}

#[test]
fn spa_fallback_skips_paths_that_look_like_a_missed_asset() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_CONFIG_SPA_FALLBACK", "static/index.html");
    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES");

    let (request, connection_info) = spa_request("/assets/missing-logo.png");
    assert!(
        !StaticResourceController::is_matching(&request, &connection_info),
        "a path with a file extension should 404, not fall back to index.html"
    );

    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK");
}

#[test]
fn spa_fallback_is_a_noop_when_configured_file_does_not_exist() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_CONFIG_SPA_FALLBACK", "static/this-file-does-not-exist.html");
    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES");

    let (request, connection_info) = spa_request("/dashboard/settings");
    assert!(!StaticResourceController::is_matching(&request, &connection_info));

    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK");
}

#[test]
fn spa_fallback_does_not_apply_to_the_root_path() {
    // request_uri == "/" is excluded from StaticResourceController entirely
    // (IndexController's territory) regardless of SPA fallback config.
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_CONFIG_SPA_FALLBACK", "static/index.html");
    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES");

    let (request, connection_info) = spa_request("/");
    assert!(!StaticResourceController::is_matching(&request, &connection_info));

    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK");
}
// ── large files: decide to stream before reading ──────────────────────────────

/// A file under `target/` (inside the working directory, so the static
/// controller serves it), deleted on drop.
struct ServedFile(String);
impl ServedFile {
    fn new(name: &str, len: usize) -> Self {
        let url = format!("/target/rws_static_test_{}_{}", std::process::id(), name);
        let fs = format!("{}{}", crate::app::controller::static_resource::working_directory(), url);
        std::fs::write(&fs, vec![b'x'; len]).unwrap();
        ServedFile(url)
    }
}
impl Drop for ServedFile {
    fn drop(&mut self) {
        let fs = format!("{}{}", crate::app::controller::static_resource::working_directory(), self.0);
        let _ = std::fs::remove_file(fs);
    }
}

fn static_request(method: &str, uri: &str, headers: Vec<Header>) -> Request {
    Request { method: method.to_string(), request_uri: uri.to_string(), http_version: VERSION.http_1_1.to_string(), headers, body: Default::default() }
}

// ── regular files: one open, metadata from the handle ─────────────────────────

fn served_with_mtime(name: &str, mtime_secs: u64) -> ServedFile {
    let file = ServedFile::new(name, 100);
    let fs = format!("{}{}", crate::app::controller::static_resource::working_directory(), file.0);
    std::fs::File::options().write(true).open(&fs).unwrap()
        .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(mtime_secs)).unwrap();
    file
}

fn via_app(request: &Request, config: crate::server_config::ServerConfig) -> Response {
    use crate::application::Application;
    let conn = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
        sni_hostname: None,
        tls: false,
    };
    crate::app::App::with_config(config).execute(request, &conn).unwrap()
}

// ── #43: static files stay inside the web root ───────────────────────────────

fn static_status(uri: &str) -> i16 {
    use crate::application::Application;
    let request = Request { method: "GET".into(), request_uri: uri.to_string(), http_version: "HTTP/1.1".into(), headers: vec![], body: Default::default() };
    let conn = ConnectionInfo { client: Address { ip: "127.0.0.1".into(), port: 1 }, server: Address { ip: "127.0.0.1".into(), port: 2 }, request_size: 0, sni_hostname: None, tls: false };
    crate::app::App::new().execute(&request, &conn).unwrap().status_code
}

#[test]
fn dot_dot_segments_cannot_leave_the_web_root() {
    // `/../<root dir name>/Cargo.toml` leaves the root and comes back in:
    // it was served (200); any `..` is refused now.
    let name = std::env::current_dir().unwrap().file_name().unwrap().to_string_lossy().to_string();
    assert_eq!(404, static_status(&format!("/../{}/Cargo.toml", name)));
    assert_eq!(404, static_status("/src/../Cargo.toml"));
    assert_eq!(404, static_status("/../../../../../../etc/passwd"));
    assert_eq!(200, static_status("/Cargo.toml"));
}

#[test]
fn hidden_files_are_not_served_but_well_known_is() {
    assert_eq!(404, static_status("/.gitignore"));
    assert!(super::check_request_path("/.git/config").is_err());
    assert!(super::check_request_path("/a/.env").is_err());
    assert!(super::check_request_path("/.well-known/security.txt").is_ok());
    assert!(super::check_request_path("/./index.html").is_ok());
    assert!(super::check_request_path("/a\\..\\b").is_err());
}

#[test]
fn key_and_config_files_are_not_served() {
    // rws.config.toml sits in the working directory (the web root) here.
    assert_eq!(404, static_status("/rws.config.toml"));
    for path in ["/key.pem", "/cert.PEM", "/a/acme_account.key", "/rws.config.toml"] {
        assert!(super::check_request_path(path).is_err(), "{path}");
    }
    assert!(super::check_request_path("/keys.html").is_ok());
}

#[cfg(unix)]
#[test]
fn a_symlink_out_of_the_root_is_not_followed() {
    let link = crate::test_env::TempPath::new_in_cwd("rws_test_escape_link");
    std::os::unix::fs::symlink("/etc", &*link).unwrap();
    let name = link.file_name().unwrap().to_string_lossy().to_string();
    assert_eq!(404, static_status(&format!("/{}/hostname", name)));
    assert_eq!(404, static_status(&format!("/{}/passwd", name)));
    assert!(!super::inside_root(&format!("{}/{}/passwd", super::working_directory(), name)));
    // A link that stays inside the root is still served.
    let inside = crate::test_env::TempPath::new_in_cwd("rws_test_inside_link.toml");
    std::os::unix::fs::symlink("Cargo.toml", &*inside).unwrap();
    let inside_name = inside.file_name().unwrap().to_string_lossy().to_string();
    assert_eq!(200, static_status(&format!("/{}", inside_name)));
}

// ── #55: the request path is percent-decoded once ───────────────────────────

#[test]
fn percent_encoded_names_are_found() {
    let file = crate::test_env::TempPath::new_in_cwd("spaced name.txt");
    std::fs::write(&*file, "spaced").unwrap();
    let name = file.file_name().unwrap().to_string_lossy().to_string();
    assert!(name.contains(' '));
    assert_eq!(200, static_status(&format!("/{}", name.replace(' ', "%20"))));
    // The general path (a Range request) finds it too, with its ETag.
    {
        use crate::application::Application;
        let request = Request { method: "GET".into(), request_uri: format!("/{}", name.replace(' ', "%20")), http_version: "HTTP/1.1".into(),
            headers: vec![Header { name: "Range".into(), value: "bytes=0-2".into() }], body: Default::default() };
        let conn = ConnectionInfo { client: Address { ip: "127.0.0.1".into(), port: 1 }, server: Address { ip: "127.0.0.1".into(), port: 2 }, request_size: 0, sni_hostname: None, tls: false };
        let response = crate::app::App::new().execute(&request, &conn).unwrap();
        assert_eq!(206, response.status_code, "{}", String::from_utf8_lossy(&response.content_range_list.first().map(|c| c.body.clone()).unwrap_or_default()));
        assert_eq!(b"spa".to_vec(), response.content_range_list[0].body);
        assert!(response.headers.iter().any(|h| h.name == "ETag"), "no ETag for an escaped name");
    }
    // Decoded once: `%2520` is a literal `%20`, which isn't this file.
    assert_eq!(404, static_status(&format!("/{}", name.replace(' ', "%2520"))));
    assert_eq!(200, static_status("/Cargo%2Etoml"));
}

#[test]
fn encoded_traversal_and_hidden_names_are_refused() {
    let name = std::env::current_dir().unwrap().file_name().unwrap().to_string_lossy().to_string();
    assert_eq!(404, static_status(&format!("/%2e%2e/{}/Cargo.toml", name)));
    assert_eq!(404, static_status(&format!("/%2E%2E%2F{}%2FCargo.toml", name)));
    assert_eq!(404, static_status("/%2egitignore"));
    assert_eq!(404, static_status("/rws.config%2Etoml"));
    assert_eq!(404, static_status("/Cargo.toml%00.html"));
}

#[test]
fn percent_decode_path_rules() {
    use super::percent_decode_path;
    assert!(matches!(percent_decode_path("/plain/path"), Ok(std::borrow::Cow::Borrowed("/plain/path"))));
    assert_eq!("/a b", percent_decode_path("/a%20b").unwrap());
    assert_eq!("/a+b", percent_decode_path("/a+b").unwrap());
    assert_eq!("/100%", percent_decode_path("/100%").unwrap());
    assert_eq!("/%zz%4", percent_decode_path("/%zz%4").unwrap());
    assert_eq!("/é", percent_decode_path("/%C3%A9").unwrap());
    assert!(percent_decode_path("/%FF").is_err());
}

#[test]
fn directory_listing_links_resolve() {
    use crate::application::Application;
    let dir = crate::test_env::TempPath::new_in_cwd("listing_dir");
    std::fs::create_dir(&*dir).unwrap();
    std::fs::write(dir.join("a b&c.txt"), "x").unwrap();
    let dir_name = dir.file_name().unwrap().to_string_lossy().to_string();
    let request = Request { method: "GET".into(), request_uri: format!("/{}/", dir_name), http_version: "HTTP/1.1".into(), headers: vec![], body: Default::default() };
    let conn = ConnectionInfo { client: Address { ip: "127.0.0.1".into(), port: 1 }, server: Address { ip: "127.0.0.1".into(), port: 2 }, request_size: 0, sni_hostname: None, tls: false };
    let listing = crate::app::App::new().execute(&request, &conn).unwrap();
    let html = String::from_utf8_lossy(&listing.content_range_list[0].body).to_string();
    let href = html.split("href=\"").skip(1)
        .map(|rest| rest.split('"').next().unwrap())
        .find(|h| h.contains("a%20b"))
        .unwrap_or_else(|| panic!("no link to the file in {html}"))
        .replace("&amp;", "&");
    let target = if href.starts_with('/') { href } else { format!("/{}/{}", dir_name, href) };
    assert_eq!(200, static_status(&target), "{target}");
}

// ── #56: conditional requests ───────────────────────────────────────────────

#[test]
fn if_none_match_is_a_list_compared_weakly() {
    use super::if_none_match_matches as m;
    let etag = "\"1700-42\"";
    assert!(m("\"1700-42\"", etag));
    assert!(m("W/\"1700-42\"", etag));
    assert!(m("\"other\", \"1700-42\"", etag));
    assert!(m("\"a\",W/\"1700-42\" , \"b\"", etag));
    assert!(m("*", etag));
    assert!(!m("\"1700-43\"", etag));
    assert!(!m("\"a\", \"b\"", etag));
    assert!(!m("", etag));
    assert!(!m("1700-42", "\"1700-42-x\""));
}

fn get_with(uri: &str, headers: Vec<(&str, &str)>) -> Response {
    use crate::application::Application;
    let request = Request { method: "GET".into(), request_uri: uri.to_string(), http_version: "HTTP/1.1".into(),
        headers: headers.into_iter().map(|(n, v)| Header { name: n.to_string().into(), value: v.to_string().into() }).collect(), body: Default::default() };
    let conn = ConnectionInfo { client: Address { ip: "127.0.0.1".into(), port: 1 }, server: Address { ip: "127.0.0.1".into(), port: 2 }, request_size: 0, sni_hostname: None, tls: false };
    crate::app::App::new().execute(&request, &conn).unwrap()
}

fn etag_of(response: &Response) -> String {
    response.headers.iter().find(|h| h.name == Header::_ETAG).map(|h| h.value.to_string()).expect("no ETag")
}

#[test]
fn a_listed_or_weak_etag_gets_304() {
    let etag = etag_of(&get_with("/Cargo.toml", vec![]));
    assert_eq!(304, get_with("/Cargo.toml", vec![("If-None-Match", &format!("\"x\", {etag}"))]).status_code);
    assert_eq!(304, get_with("/Cargo.toml", vec![("If-None-Match", &format!("W/{etag}"))]).status_code);
    assert_eq!(200, get_with("/Cargo.toml", vec![("If-None-Match", "\"x\", \"y\"")]).status_code);
}

#[test]
fn if_range_with_a_stale_validator_sends_the_whole_file() {
    let whole = get_with("/Cargo.toml", vec![]);
    let etag = etag_of(&whole);
    let len = whole.content_range_list[0].body.len();
    // Current strong tag: the range is honored.
    let partial = get_with("/Cargo.toml", vec![("Range", "bytes=0-9"), ("If-Range", &etag)]);
    assert_eq!(206, partial.status_code);
    assert_eq!(10, partial.content_range_list[0].body.len());
    // Another tag, the weak form, or a date: the whole file, 200.
    for stale in ["\"0-0\"".to_string(), format!("W/{etag}"), "Sat, 01 Jan 2000 00:00:00 GMT".to_string()] {
        let response = get_with("/Cargo.toml", vec![("Range", "bytes=0-9"), ("If-Range", &stale)]);
        assert_eq!(200, response.status_code, "If-Range: {stale}");
        assert_eq!(len, response.content_range_list[0].body.len());
    }
    // If-Range without Range changes nothing.
    assert_eq!(200, get_with("/Cargo.toml", vec![("If-Range", "\"0-0\"")]).status_code);
}

#[test]
fn if_range_with_the_current_last_modified_keeps_the_range() {
    let whole = get_with("/Cargo.toml", vec![]);
    let last_modified = whole.headers.iter().find(|h| h.name == Header::_LAST_MODIFIED).map(|h| h.value.to_string()).expect("no Last-Modified");
    let partial = get_with("/Cargo.toml", vec![("Range", "bytes=0-9"), ("If-Range", &last_modified)]);
    assert_eq!(206, partial.status_code);
    assert_eq!(10, partial.content_range_list[0].body.len());
}

#[test]
fn one_satisfiable_range_is_enough_and_416_names_the_length() {
    let len = std::fs::metadata("Cargo.toml").unwrap().len();
    let partial = get_with("/Cargo.toml", vec![("Range", "bytes=0-9, 99999999-")]);
    assert_eq!(206, partial.status_code);
    assert_eq!(1, partial.content_range_list.len());
    assert_eq!(10, partial.content_range_list[0].body.len());

    let none = get_with("/Cargo.toml", vec![("Range", "bytes=99999999-, 99999999-100000000")]);
    assert_eq!(416, none.status_code);
    let content_range = none.headers.iter().find(|h| h.name == Header::_CONTENT_RANGE).map(|h| h.value.to_string());
    assert_eq!(Some(format!("bytes */{len}")), content_range);
}

#[test]
fn an_unsupported_method_on_a_file_is_405_with_allow() {
    use crate::application::Application;
    let conn = ConnectionInfo { client: Address { ip: "127.0.0.1".into(), port: 1 }, server: Address { ip: "127.0.0.1".into(), port: 2 }, request_size: 0, sni_hostname: None, tls: false };
    let send = |method: &str, uri: &str| {
        let request = Request { method: method.into(), request_uri: uri.into(), http_version: "HTTP/1.1".into(), headers: vec![], body: Default::default() };
        crate::app::App::new().execute(&request, &conn).unwrap()
    };
    for method in ["POST", "PUT", "DELETE", "PATCH"] {
        let response = send(method, "/Cargo.toml");
        assert_eq!(405, response.status_code, "{method}");
        let allow = response.headers.iter().find(|h| h.name == Header::_ALLOW).map(|h| h.value.to_string());
        assert_eq!(Some("GET, HEAD, OPTIONS".to_string()), allow);
    }
    // A path that doesn't exist, or one outside the root, stays 404.
    assert_eq!(404, send("POST", "/no-such-file.txt").status_code);
    assert_eq!(404, send("DELETE", "/../etc/passwd").status_code);
    assert_eq!(404, send("POST", "/rws.config.toml").status_code);
}

#[test]
fn an_escaped_extensionless_name_finds_its_html_file() {
    let file = crate::test_env::TempPath::new_in_cwd("spaced page.html");
    std::fs::write(&*file, "<p>page</p>").unwrap();
    let name = file.file_name().unwrap().to_string_lossy().to_string();
    let stem = name.trim_end_matches(".html").replace(' ', "%20");
    let response = get_with(&format!("/{stem}"), vec![]);
    assert_eq!(200, response.status_code);
    assert_eq!(b"<p>page</p>".to_vec(), response.content_range_list[0].body);
}

// ── #38: static files as compression dictionaries ─────────────────────────────

#[test]
fn dictionary_patterns_glob_like_url_patterns() {
    use super::dictionaries::{glob, matching_pattern, use_as_dictionary};
    assert!(glob("/js/app.*.js", "/js/app.1a2b.js"));
    assert!(glob("/js/*", "/js/vendor/x.js"), "* crosses /");
    assert!(glob("/a*b*c", "/aXbYc") && glob("/exact.css", "/exact.css"));
    assert!(!glob("/js/app.*.js", "/js/app.1a2b.css"));
    assert!(!glob("/js/app.*.js", "/css/app.1.js"));
    assert!(!glob("/a*bb", "/ab"), "parts can't overlap");
    let patterns = vec!["/css/*.css".to_string(), "/js/*.js".to_string()];
    assert_eq!(matching_pattern(&patterns, "/js/x.js"), Some("/js/*.js"));
    assert_eq!(matching_pattern(&patterns, "/img/x.png"), None);
    assert_eq!(use_as_dictionary("/js/*.js"), r#"match="/js/*.js""#);
    assert_eq!(use_as_dictionary(r#"/a"b\c"#), r#"match="/a\"b\\c""#);
}

#[test]
fn later_versions_of_a_dictionary_file_go_out_as_deltas() {
    use crate::compression::dictionary::{decode, hash, DictionaryEncoding};
    let file = crate::test_env::TempPath::new_in_cwd("rws_dict_test_bundle.js");
    let name = file.file_name().unwrap().to_string_lossy().to_string();
    let path = format!("/{name}");
    let version = |v: &str, n: usize| -> Vec<u8> {
        (0..n).map(|i| format!("export function widget{i}(el) {{ el.dataset.v = '{v}'; return el.querySelector('.w{i}'); }}\n")).collect::<String>().into_bytes()
    };
    let (v1, v2) = (version("1.0.0", 300), version("1.0.1", 301));
    let config = crate::server_config::ServerConfig {
        compression_dictionaries: vec!["/rws_test_*_rws_dict_test_*.js".to_string()],
        ..crate::server_config::ServerConfig::default()
    };
    let client = crate::test_client::TestClient::new(crate::app::App::with_config(config));
    // Builds without brotli (http1-only) send these uncompressed.
    let br = crate::compression::Encoding::Brotli.available().then_some("br");

    std::fs::write(&*file, &v1).unwrap();
    let first = client.get(&path).header("Accept-Encoding", "br, dcz, dcb").send();
    assert_eq!(first.status(), 200);
    assert_eq!(first.header("Use-As-Dictionary"), Some(r#"match="/rws_test_*_rws_dict_test_*.js""#));
    assert_eq!(first.header("Content-Encoding"), br, "no dictionary yet: plain br");
    assert!(first.header("Vary").unwrap().contains("Available-Dictionary"));

    std::fs::write(&*file, &v2).unwrap();
    let held = format!(":{}:", crate::core::base64::Base64::encode(&hash(&v1)).unwrap());
    for (accept, coding) in [("br, dcb, dcz", DictionaryEncoding::Dcz), ("br, dcb", DictionaryEncoding::Dcb)] {
        if !coding.available() {
            continue;
        }
        let delta = client.get(&path).header("Accept-Encoding", accept).header("Available-Dictionary", &held).send();
        assert_eq!(delta.status(), 200);
        assert_eq!(delta.header("Content-Encoding"), Some(coding.token()), "{accept}");
        assert!(delta.header("ETag").unwrap().contains(&format!("-{}-", coding.token())), "{:?}", delta.header("ETag"));
        assert_eq!(delta.header("Use-As-Dictionary"), Some(r#"match="/rws_test_*_rws_dict_test_*.js""#), "the new version is a dictionary too");
        assert!(delta.body_bytes().len() < 600, "{} bytes", delta.body_bytes().len());
        assert_eq!(decode(coding, delta.body_bytes(), &v1, 1 << 20).unwrap(), v2);
        // Its ETag revalidates to a 304.
        let etag = delta.header("ETag").unwrap().to_string();
        let again = client.get(&path).header("Accept-Encoding", accept).header("Available-Dictionary", &held).header("If-None-Match", &etag).send();
        assert_eq!(again.status(), 304);
    }

    let unknown = format!(":{}:", crate::core::base64::Base64::encode(&hash(b"never served")).unwrap());
    let fallback = client.get(&path).header("Accept-Encoding", "br, dcz").header("Available-Dictionary", &unknown).send();
    assert_eq!(fallback.header("Content-Encoding"), br, "a dictionary we don't have");
    assert!(super::dictionaries::kept() >= 2, "both versions kept");

    // Without the setting nothing changes.
    let plain = crate::test_client::TestClient::new(crate::app::App::with_config(crate::server_config::ServerConfig::default()));
    let response = plain.get(&path).header("Accept-Encoding", "br, dcz").header("Available-Dictionary", &held).send();
    assert_eq!((response.header("Use-As-Dictionary"), response.header("Content-Encoding")), (None, br));
}

// ── links with prose stuck to them (`/docs/x/):`) ─────────────────────────────

#[test]
fn trailing_punctuation_redirects_to_what_exists() {
    let file = crate::test_env::TempPath::new_in_cwd("trail page.txt");
    std::fs::write(&*file, "here").unwrap();
    let name = file.file_name().unwrap().to_string_lossy().replace(' ', "%20");
    let client = crate::test_client::TestClient::new(crate::app::App::with_config(crate::server_config::ServerConfig::default()));
    for suffix in ["):", ")", ".", ",", "%29%3A", "\");", ">"] {
        let response = client.get(&format!("/{name}{suffix}?q=1")).send();
        assert_eq!(response.status(), 301, "{suffix}");
        assert_eq!(response.header("Location"), Some(format!("/{name}?q=1").as_str()), "{suffix}");
    }
    assert_eq!(client.get(&format!("/{name}")).send().status(), 200, "the file itself");
    // Nothing behind it: no redirect (the 404 or a configured fallback answers).
    let missing = client.get("/rws-test-no-such-page-7f3a/):").send();
    assert!(missing.status() != 301 && missing.header("Location").is_none(), "{}", missing.status());
    let root = client.get("/):").send();
    assert_eq!((root.status(), root.header("Location")), (301, Some("/")), "the home page");
    assert_ne!(client.post(&format!("/{name}):")).send().status(), 301, "GET and HEAD only");
}
