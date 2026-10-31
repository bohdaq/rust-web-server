use super::test_support::{body, WhoAmI};
use super::*;
use crate::application::Application;
use crate::request::Request;
use crate::server::ConnectionInfo;

fn get() -> Request {
    Request::parse(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n").unwrap()
}

#[test]
fn subjects_are_typed() {
    let bare = Identity::new("alice", "basic");
    assert_eq!((bare.subject(), bare.subject_type(), bare.subject_id(), bare.source()), ("user:alice", "user", "alice", "basic"));
    let typed = Identity::new("service:billing", "forward");
    assert_eq!((typed.subject_type(), typed.subject_id()), ("service", "billing"));
}

#[test]
fn identities_come_from_top_level_string_claims() {
    let claims = r#"{"sub":"alice","email":"a@example.com","n":3,"nested":{"sub":"mallory"}}"#;
    let identity = Identity::from_claims(claims, "sub", "jwt").unwrap();
    assert_eq!(identity.subject(), "user:alice");
    assert_eq!(identity.claim("email").as_deref(), Some("a@example.com"));
    assert_eq!(identity.claims(), Some(claims));
    assert_eq!(Identity::from_claims(claims, "email", "jwt").unwrap().subject(), "user:a@example.com");
    assert!(Identity::from_claims(claims, "n", "jwt").is_none(), "not a string");
    assert!(Identity::from_claims(claims, "missing", "jwt").is_none());
    assert!(Identity::from_claims(r#"{"sub":""}"#, "sub", "jwt").is_none(), "empty");
    assert!(Identity::from_claims("not json", "sub", "jwt").is_none());
    assert_eq!(Identity::from_claims(r#"{"sub":"a\"b"}"#, "sub", "jwt").unwrap().subject(), "user:a\"b");
}

#[test]
fn with_identity_sets_and_restores_even_on_panic() {
    assert!(Identity::current().is_none());
    with_identity(Identity::new("alice", "test"), || {
        assert_eq!(Identity::current().unwrap().subject(), "user:alice");
        let caught = std::panic::catch_unwind(|| with_identity(Identity::new("bob", "test"), || panic!("handler bug")));
        assert!(caught.is_err());
        assert_eq!(Identity::current().unwrap().subject(), "user:alice", "restored after the panic");
    });
    assert!(Identity::current().is_none());
}

#[cfg(feature = "http2")]
#[test]
fn async_code_runs_as_its_scoped_identity() {
    let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
    let seen = runtime.block_on(scope_identity(Identity::new("carol", "test"), async {
        tokio::task::yield_now().await;
        Identity::current().map(|i| i.subject().to_string())
    }));
    assert_eq!(seen.as_deref(), Some("user:carol"));
}

#[test]
fn a_client_cannot_send_an_identity() {
    let request = Request::parse(b"GET / HTTP/1.1\r\nHost: x\r\nX-Rws-Identity: user:root\r\nIdentity: user:root\r\n\r\n").unwrap();
    assert_eq!(body(&WhoAmI.execute(&request, &ConnectionInfo::default()).unwrap()), "anonymous");
    assert_eq!(body(&WhoAmI.execute(&get(), &ConnectionInfo::default()).unwrap()), "anonymous");
}

#[cfg(feature = "auth")]
#[test]
fn jwt_and_basic_auth_hand_over_their_caller() {
    use crate::middleware::WithMiddleware;
    let jwt = crate::test_client::TestClient::new(WithMiddleware::new(WhoAmI).wrap(crate::auth::JwtLayer::new(b"secret")));
    let token = crate::auth::build_jwt(r#"{"sub":"alice","email":"a@example.com"}"#, b"secret");
    assert_eq!(jwt.get("/").header("Authorization", &format!("Bearer {token}")).send().body_text(), "user:alice via jwt");
    let by_email = crate::test_client::TestClient::new(WithMiddleware::new(WhoAmI).wrap(crate::auth::JwtLayer::new(b"secret").identity_claim("email")));
    assert_eq!(by_email.get("/").header("Authorization", &format!("Bearer {token}")).send().body_text(), "user:a@example.com via jwt");
    let no_sub = crate::auth::build_jwt(r#"{"scope":"read"}"#, b"secret");
    assert_eq!(jwt.get("/").header("Authorization", &format!("Bearer {no_sub}")).send().body_text(), "anonymous", "valid, no identity");

    let basic = crate::test_client::TestClient::new(WithMiddleware::new(WhoAmI).wrap(crate::auth::BasicAuthLayer::new(|u, p| u == "admin" && p == "pw")));
    let credentials = crate::core::base64::Base64::encode(b"admin:pw").unwrap();
    assert_eq!(basic.get("/").header("Authorization", &format!("Basic {credentials}")).send().body_text(), "user:admin via basic");
}

#[test]
fn mcp_bearer_tokens_name_the_caller() {
    use crate::mcp::{McpContent, McpServer};
    let server = McpServer::new("srv", "1.0").require_bearer_tokens([("alice-token", "alice")]).tool("whoami", "Who am I", "{}", |_| {
        Ok(McpContent::text(Identity::current().map_or("anonymous".to_string(), |i| format!("{} via {}", i.subject(), i.source()))))
    });
    let client = crate::test_client::TestClient::new(server);
    let response = client
        .post("/mcp")
        .header("Authorization", "Bearer alice-token")
        .body_json(r#"{"jsonrpc":"2.0","method":"tools/call","id":1,"params":{"name":"whoami","arguments":{}}}"#)
        .send();
    assert!(response.body_text().contains("user:alice via mcp"), "{}", response.body_text());
}

// ── AuthzLayer and object templates ──────────────────────────────────────────

fn request(raw: &str) -> Request {
    Request::parse(raw.as_bytes()).unwrap()
}

#[test]
fn object_templates_read_path_query_and_headers() {
    let t = ObjectTemplate::parse("doc:{path:2}").unwrap();
    assert_eq!(t.object_type(), Some("doc"));
    assert_eq!(t.render(&request("GET /docs/42/edit?x=1 HTTP/1.1\r\nHost: x\r\n\r\n")).unwrap().to_string(), "doc:42");
    assert!(t.render(&request("GET /docs HTTP/1.1\r\nHost: x\r\n\r\n")).is_none(), "no second segment");
    let q = ObjectTemplate::parse("org:{header:X-Org}-{query:team}").unwrap();
    assert_eq!(q.render(&request("GET /?a=b&team=core HTTP/1.1\r\nHost: x\r\nX-Org: acme\r\n\r\n")).unwrap().to_string(), "org:acme-core");
    assert!(q.render(&request("GET /?team=core HTTP/1.1\r\nHost: x\r\n\r\n")).is_none(), "missing header");
    assert!(t.render(&request("GET /docs/a%23b HTTP/1.1\r\nHost: x\r\n\r\n")).is_some_and(|o| o.id == "a%23b"), "raw segment, still a valid id");
    for bad in ["doc:{path:0}", "doc:{path:x}", "doc:{cookie:a}", "doc:{path:1", "{path:1}", "doc:42}", ""] {
        assert!(ObjectTemplate::parse(bad).is_err(), "{bad:?}");
    }
}

fn docs_authz() -> Authorizer {
    let schema = Schema::new().relation("doc", "owner", "user").unwrap().relation("doc", "viewer", "user | owner").unwrap();
    let authz = Authorizer::new(schema, MemoryStore::new()).unwrap();
    authz.write_str("doc:42#owner@user:alice").unwrap();
    authz
}

#[test]
fn the_layer_answers_401_403_or_lets_through() {
    use crate::middleware::Middleware;
    let layer = AuthzLayer::new(docs_authz(), "viewer", "doc:{path:2}").unwrap();
    let get = |path: &str| request(&format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n"));
    let run = |who: Option<&str>, path: &str| {
        let call = || layer.handle(&get(path), &ConnectionInfo::default(), &WhoAmI).unwrap();
        let response = match who {
            Some(subject) => with_identity(Identity::new(subject, "test"), call),
            None => call(),
        };
        (response.status_code, body(&response))
    };
    assert_eq!(run(Some("alice"), "/docs/42"), (200, "user:alice via test".to_string()));
    assert_eq!(run(Some("bob"), "/docs/42").0, 403);
    assert_eq!(run(Some("alice"), "/docs/7").0, 403);
    assert_eq!(run(Some("alice"), "/docs").0, 403, "no object");
    assert_eq!(run(None, "/docs/42").0, 401);
    assert!(AuthzLayer::new(docs_authz(), "editor", "doc:{path:2}").unwrap_err().contains("doc#editor isn't defined"));
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
