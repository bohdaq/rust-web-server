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
