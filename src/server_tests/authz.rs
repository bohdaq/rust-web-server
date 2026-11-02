//! Tests of rws-server's `authz` that need the facade (proxy routes,
//! `rws --check`, the admin API, ACME, the `routes!` macro).
#![allow(unused_imports, dead_code)]

use crate::authz::test_support::{body, WhoAmI};
use crate::authz::*;
use crate::application::Application;
use crate::request::Request;
use crate::server::ConnectionInfo;

fn get() -> Request {
    Request::parse(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n").unwrap()
}

fn request(raw: &str) -> Request {
    Request::parse(raw.as_bytes()).unwrap()
}

fn docs_authz() -> Authorizer {
    let schema = Schema::new().relation("doc", "owner", "user").unwrap().relation("doc", "viewer", "user | owner").unwrap();
    let authz = Authorizer::new(schema, MemoryStore::new()).unwrap();
    authz.write_str("doc:42#owner@user:alice").unwrap();
    authz
}

const AUTHZ_TOML: &str = r#"
[authz]
tuples = ["doc:42#owner@user:alice"]
[authz.type.doc]
owner = "user"
viewer = "user | owner"

[[route]]
name = "docs"
[route.match]
path = "/docs/*"
[route.action]
type = "respond"
body = "the doc"
[route.middleware.auth]
type = "jwt"
secret_env = "RWS_TEST_AUTHZ_JWT"
[route.middleware.authz]
relation = "viewer"
object = "doc:{path:2}"
"#;

#[cfg(feature = "auth")]
#[test]
fn config_routes_check_relations_and_runtime_tuples_survive_a_reload() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_TEST_AUTHZ_JWT", "secret");
    let report = crate::config_check::check_str(AUTHZ_TOML);
    assert!(report.errors.is_empty() && report.warnings.is_empty(), "{:?} {:?}", report.errors, report.warnings);
    let (app, _handles) = crate::proxy_config::builder::build(crate::proxy_config::ProxyConfig::from_str(AUTHZ_TOML));
    let client = crate::test_client::TestClient::new(app.clone());
    let token = |sub: &str| format!("Bearer {}", crate::auth::build_jwt(&format!(r#"{{"sub":"{sub}"}}"#), b"secret"));
    assert_eq!(client.get("/docs/42").header("Authorization", &token("alice")).send().status(), 200);
    assert_eq!(client.get("/docs/42").header("Authorization", &token("bob")).send().status(), 403);
    assert_eq!(client.get("/docs/42").send().status(), 401, "jwt layer: no token");
    // Path confusion: the check must not see doc 42 while a backend serves 7.
    assert_eq!(client.get("/docs/42/../7").header("Authorization", &token("alice")).send().status(), 403);
    assert_eq!(client.get("/docs/42/%2e%2e/7").header("Authorization", &token("alice")).send().status(), 403);
    // A tuple written at runtime (as the admin API will) survives a reload.
    let authorizer = crate::authz::config::current().unwrap().unwrap();
    authorizer.write_str("doc:42#viewer@user:bob").unwrap();
    app.reload_from_str(&AUTHZ_TOML.replace("the doc", "the doc, v2")).unwrap();
    let response = client.get("/docs/42").header("Authorization", &token("bob")).send();
    assert_eq!((response.status(), response.body_text()), (200, "the doc, v2"));
    std::env::remove_var("RWS_TEST_AUTHZ_JWT");
}

#[test]
fn rws_check_reports_authz_mistakes() {
    let errors = |toml: &str| crate::config_check::check_str_offline(toml).errors.join("\n");
    let warnings = |toml: &str| crate::config_check::check_str_offline(toml).warnings.join("\n");
    assert!(errors(&AUTHZ_TOML.replace("[authz]\n", "[nothing]\n").replace("[authz.type.doc]", "[nothing.type.doc]")).contains("needs an [authz] section"));
    assert!(errors(&AUTHZ_TOML.replace(r#"relation = "viewer""#, r#"relation = "editor""#)).contains("doc#editor isn't defined"));
    assert!(errors(&AUTHZ_TOML.replace("doc:{path:2}", "doc:{path:x}")).contains("{path:N} needs N"));
    assert!(errors(&AUTHZ_TOML.replace(r#"viewer = "user | owner""#, r#"viewer = "user | team#member""#)).contains("[authz]: "));
    assert!(errors(&AUTHZ_TOML.replace("doc:42#owner@user:alice", "doc:42#owner@group:x")).contains("tuple"));
    assert!(errors(&AUTHZ_TOML.replace(r#"object = "doc:{path:2}""#, "object = \"doc:{path:2}\"\nrole = \"x\"")).contains("role: unknown key"));
    assert!(errors(&AUTHZ_TOML.replace("[authz]\n", "[authz]\nstore = \"redis\"\n")).contains("isn't supported"));
    let no_auth = AUTHZ_TOML.replace("[route.middleware.auth]\ntype = \"jwt\"\nsecret_env = \"RWS_TEST_AUTHZ_JWT\"\n", "");
    assert!(warnings(&no_auth).contains("every request gets 401"));
    let global = AUTHZ_TOML.replace("[[route]]", "[middleware.authz]\nrelation = \"viewer\"\nobject = \"doc:{path:2}\"\n\n[[route]]");
    assert!(warnings(&global).contains("/healthz, /readyz and /metrics included"), "{}", warnings(&global));
}

#[test]
fn the_admin_api_lists_writes_deletes_and_checks_tuples() {
    let _g = crate::test_env::lock(); // the admin's authorizer is process-wide
    let schema = Schema::new()
        .relation("group", "member", "user").unwrap()
        .relation("doc", "viewer", "user | group#member").unwrap();
    let authz = Authorizer::new(schema, MemoryStore::new()).unwrap();
    crate::admin::ops::register_authorizer(authz.clone());
    let api = crate::admin::AdminApi::new("t");
    let call = |method: &str, path: &str, body: &str| {
        let raw = format!("{method} {path} HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer t\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len());
        let response = api.execute(&Request::parse(raw.as_bytes()).unwrap(), &ConnectionInfo::default()).unwrap();
        (response.status_code, body_of(&response))
    };
    fn body_of(r: &crate::response::Response) -> String {
        String::from_utf8_lossy(&r.content_range_list.first().map(|c| c.body.clone()).unwrap_or_default()).to_string()
    }
    assert_eq!(call("GET", "/admin/api/v1/authz", ""), (200, r#"{"types":{"doc":["viewer"],"group":["member"],"user":[]}}"#.to_string()));
    assert_eq!(call("POST", "/admin/api/v1/authz/tuples", r#"{"tuple":"group:eng#member@user:gina"}"#), (200, r#"{"tuple":"group:eng#member@user:gina","written":true}"#.to_string()));
    call("POST", "/admin/api/v1/authz/tuples", r#"{"tuple":"doc:42#viewer@group:eng#member"}"#);
    assert_eq!(call("POST", "/admin/api/v1/authz/tuples", r#"{"tuple":"doc:42#owner@user:x"}"#).0, 400, "not in the schema");
    assert_eq!(call("POST", "/admin/api/v1/authz/tuples", r#"{"nope":1}"#).0, 400);
    let check = r#"{"subject":"user:gina","relation":"viewer","object":"doc:42"}"#;
    assert_eq!(call("POST", "/admin/api/v1/authz/check", check), (200, r#"{"allowed":true,"path":["doc:42#viewer","group:eng#member","group:eng#member@user:gina"]}"#.to_string()));
    assert_eq!(call("GET", "/admin/api/v1/authz/tuples?object=doc", ""), (200, r#"{"tuples":["doc:42#viewer@group:eng#member"]}"#.to_string()));
    assert_eq!(call("GET", "/admin/api/v1/authz/tuples?subject=user%3Agina", "").1, r#"{"tuples":["group:eng#member@user:gina"]}"#);
    assert_eq!(call("DELETE", "/admin/api/v1/authz/tuples?tuple=group%3Aeng%23member%40user%3Agina", ""), (200, r#"{"tuple":"group:eng#member@user:gina","deleted":true}"#.to_string()));
    assert_eq!(call("POST", "/admin/api/v1/authz/check", check).1, r#"{"allowed":false,"path":[]}"#);
    assert_eq!(call("PUT", "/admin/api/v1/authz/tuples", "").0, 405);
    assert_eq!(call("GET", "/admin/api/v1/authz/nothing", "").0, 404);
    assert!(authz.check("user:gina", "viewer", "doc:42").is_ok());
}
