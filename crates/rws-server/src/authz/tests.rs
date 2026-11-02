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
    // A claim can't name a set or another type: it's always a user id.
    assert!(Identity::from_claims(r#"{"sub":"group:eng#member"}"#, "sub", "jwt").is_none(), "'#' can't be in an id");
    let typed = Identity::from_claims(r#"{"sub":"service:billing"}"#, "sub", "jwt").unwrap();
    assert_eq!((typed.subject_type(), typed.subject_id()), ("user", "service:billing"));
    assert!(Identity::user("has space", "basic").is_none() && Identity::user("", "basic").is_none());
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
    assert_eq!(t.render(&request("GET /docs/a%20b%40c HTTP/1.1\r\nHost: x\r\n\r\n")), None, "decoded once: a space isn't an id");
    assert_eq!(t.render(&request("GET /docs/alice%40example.com HTTP/1.1\r\nHost: x\r\n\r\n")).unwrap().id, "alice@example.com");
    assert_eq!(t.render(&request("GET /docs/a%23b HTTP/1.1\r\nHost: x\r\n\r\n")), None, "'#' after decoding");
    // Dot segments anywhere (raw or encoded) and encoded slashes name nothing:
    // a backend could resolve them to another document than the one checked.
    for path in ["/docs/42/../7", "/docs/42/%2e%2e/7", "/docs/42/%2E%2E/7", "/docs/./42", "/docs/42%2F..%2F7", "/docs/42%5C..%5C7"] {
        assert_eq!(t.render(&request(&format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n"))), None, "{path}");
    }
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
fn a_token_cannot_claim_to_be_a_group() {
    use crate::middleware::WithMiddleware;
    let schema = Schema::new()
        .relation("group", "member", "user").unwrap()
        .relation("doc", "editor", "user | group#member").unwrap();
    let authz = Authorizer::new(schema, MemoryStore::new()).unwrap();
    authz.write_str("doc:42#editor@group:eng#member").unwrap();
    authz.write_str("doc:42#editor@user:group:eng").unwrap(); // a user literally named "group:eng"
    // The first layer pushed is the outermost: the authenticator first.
    let app = WithMiddleware::new(WhoAmI)
        .wrap(crate::auth::JwtLayer::new(b"secret"))
        .wrap(AuthzLayer::new(authz, "editor", "doc:{path:2}").unwrap());
    let client = crate::test_client::TestClient::new(app);
    let call = |sub: &str| {
        let token = crate::auth::build_jwt(&format!(r#"{{"sub":"{sub}"}}"#), b"secret");
        let response = client.get("/docs/42").header("Authorization", &format!("Bearer {token}")).send();
        (response.status(), response.body_text().to_string())
    };
    assert_eq!(call("group:eng#member").0, 401, "not a usable id: no caller");
    assert_eq!(call("group:eng"), (200, "user:group:eng via jwt".to_string()), "only the user of that name");
    assert_eq!(call("group:other").0, 403);
}

#[cfg(feature = "http2")]
#[test]
fn async_handlers_on_another_thread_see_the_caller() {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let seen = runtime.block_on(async {
        with_identity(Identity::new("alice", "test"), || {
            crate::async_bridge::block_on_isolated(|| async { Identity::current().map(|i| i.subject().to_string()) })
        })
    });
    assert_eq!(seen.as_deref(), Some("user:alice"));
}

