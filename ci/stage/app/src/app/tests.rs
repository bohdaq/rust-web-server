use super::*;
use hmac::Mac;
use rust_web_server::test_client::{TestClient, TestResponse};

fn client() -> TestClient<Root> {
    TestClient::new(build(AppConfig::for_tests()))
}

fn client_with(f: impl FnOnce(&mut AppConfig)) -> TestClient<Root> {
    let mut config = AppConfig::for_tests();
    f(&mut config);
    TestClient::new(build(config))
}

/// `name=value` of the first `Set-Cookie` named `name`.
fn cookie(r: &TestResponse, name: &str) -> String {
    r.headers()
        .iter()
        .filter(|h| h.name.eq_ignore_ascii_case("Set-Cookie"))
        .map(|h| h.value.split(';').next().unwrap().to_string())
        .find(|c| c.starts_with(&format!("{name}=")))
        .unwrap_or_else(|| panic!("no {name} cookie"))
}

fn b64url(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..=chunk.len() {
            out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

fn pem_wrap(label: &str, der: &[u8]) -> String {
    let b64 = rust_web_server::auth::base64_encode(der);
    let lines: Vec<&str> = b64.as_bytes().chunks(64).map(|c| std::str::from_utf8(c).unwrap()).collect();
    format!("-----BEGIN {label}-----\n{}\n-----END {label}-----\n", lines.join("\n"))
}

fn jwt(alg: &str, claims: &str, sign: impl Fn(&[u8]) -> Vec<u8>) -> String {
    let message = format!("{}.{}", b64url(format!(r#"{{"alg":"{alg}","typ":"JWT"}}"#).as_bytes()), b64url(claims.as_bytes()));
    let signature = sign(message.as_bytes());
    format!("{message}.{}", b64url(&signature))
}

#[test]
fn healthz_is_ok() {
    let r = client().get("/healthz").send();
    assert_eq!(200, r.status());
    assert_eq!("ok", r.body_text());
}

#[test]
fn sessions_in_memory() {
    let c = client();
    assert_eq!("memory", c.get("/app/session/store").send().body_text());
    assert_eq!(401, c.get("/app/session/me").send().status());
    let r = c.post("/app/session/login?user=alice").send();
    assert_eq!(200, r.status());
    let sid = cookie(&r, SESSION_COOKIE);
    let me = c.get("/app/session/me").header("Cookie", &sid).send();
    assert_eq!(200, me.status());
    assert_eq!("alice", me.body_text());
    assert_eq!(401, c.get("/app/session/me").header("Cookie", "rws_e2e_sid=forged").send().status());
    assert_eq!(400, c.post("/app/session/login").send().status());
}

#[test]
fn csrf_form_and_submit() {
    let c = client();
    let form = c.get("/app/csrf/form").send();
    assert_eq!(200, form.status());
    let csrf_cookie = cookie(&form, "_csrf");
    let token = csrf_cookie.trim_start_matches("_csrf=").to_string();
    assert_eq!(64, token.len());
    assert!(form.body_text().contains(&format!(r#"name="_csrf" value="{token}""#)), "{}", form.body_text());

    let submit = |body: &str, cookie: Option<&str>| {
        let mut req = c.post("/app/csrf/submit").header("Content-Type", "application/x-www-form-urlencoded");
        if let Some(cookie) = cookie {
            req = req.header("Cookie", cookie);
        }
        req.body_bytes(body.as_bytes().to_vec()).send()
    };
    let ok = submit(&format!("_csrf={token}&x=1"), Some(&csrf_cookie));
    assert_eq!(200, ok.status());
    assert_eq!("csrf-ok", ok.body_text());
    assert_eq!(403, submit("x=1", Some(&csrf_cookie)).status());
    assert_eq!(403, submit("_csrf=wrong&x=1", Some(&csrf_cookie)).status());
    assert_eq!(403, submit(&format!("_csrf={token}"), None).status());
    // The X-CSRF-Token header works too.
    let r = c.post("/app/csrf/submit").header("Cookie", &csrf_cookie).header("X-CSRF-Token", &token).send();
    assert_eq!(200, r.status());
}

#[test]
fn csrf_is_scoped_to_its_routes() {
    // Other POST routes are not refused for a missing token.
    let r = client().post("/app/session/login?user=bob").send();
    assert_eq!(200, r.status());
}

#[test]
fn jwt_rs256() {
    use aws_lc_rs::signature::KeyPair;
    let key = aws_lc_rs::rsa::KeyPair::generate(aws_lc_rs::rsa::KeySize::Rsa2048).unwrap();
    let pem = pem_wrap("RSA PUBLIC KEY", key.public_key().as_ref());
    let sign = |m: &[u8]| {
        let mut sig = vec![0u8; key.public_modulus_len()];
        key.sign(&aws_lc_rs::signature::RSA_PKCS1_SHA256, &aws_lc_rs::rand::SystemRandom::new(), m, &mut sig).unwrap();
        sig
    };
    let c = client_with(|cfg| cfg.jwt_rs256 = rs256_key(&pem));
    let token = jwt("RS256", r#"{"sub":"rs-user","exp":9999999999}"#, sign);
    let r = c.get("/app/jwt/rs256").header("Authorization", &format!("Bearer {token}")).send();
    assert_eq!(200, r.status(), "{}", r.body_text());
    assert_eq!("rs-user", r.body_text());
    let expired = jwt("RS256", r#"{"sub":"rs-user","exp":1}"#, sign);
    assert_eq!(401, c.get("/app/jwt/rs256").header("Authorization", &format!("Bearer {expired}")).send().status());
    assert_eq!(401, c.get("/app/jwt/rs256").send().status());
    let mut tampered = token.clone();
    tampered.replace_range(tampered.len() - 4.., "AAAA");
    assert_eq!(401, c.get("/app/jwt/rs256").header("Authorization", &format!("Bearer {tampered}")).send().status());
}

#[test]
fn jwt_es256() {
    use p256::ecdsa::signature::Signer;
    use p256::pkcs8::{EncodePublicKey, LineEnding};
    let signing = p256::ecdsa::SigningKey::from_slice(&[7u8; 32]).unwrap();
    let pem = signing.verifying_key().to_public_key_pem(LineEnding::LF).unwrap();
    let sign = |m: &[u8]| {
        let sig: p256::ecdsa::Signature = signing.sign(m);
        sig.to_bytes().to_vec()
    };
    let c = client_with(|cfg| cfg.jwt_es256 = es256_key(&pem));
    let token = jwt("ES256", r#"{"sub":"es-user"}"#, sign);
    let r = c.get("/app/jwt/es256").header("Authorization", &format!("Bearer {token}")).send();
    assert_eq!(200, r.status(), "{}", r.body_text());
    assert_eq!("es-user", r.body_text());
    // An ES256 key never accepts an HS256 token.
    let hs = rust_web_server::auth::build_jwt(r#"{"sub":"x"}"#, b"k");
    assert_eq!(401, c.get("/app/jwt/es256").header("Authorization", &format!("Bearer {hs}")).send().status());
}

#[test]
fn webhook_signature() {
    let c = client_with(|cfg| cfg.webhook_secret = Some("hook-secret".into()));
    let body = br#"{"action":"opened"}"#;
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(b"hook-secret").unwrap();
    mac.update(body);
    let sig: String = mac.finalize().into_bytes().iter().map(|b| format!("{b:02x}")).collect();
    let r = c.post("/app/webhook/github").header("X-Hub-Signature-256", &format!("sha256={sig}")).body_bytes(body.to_vec()).send();
    assert_eq!(200, r.status());
    assert_eq!("webhook-ok", r.body_text());
    let r = c.post("/app/webhook/github").header("X-Hub-Signature-256", "sha256=00").body_bytes(body.to_vec()).send();
    assert_eq!(401, r.status());
    assert_eq!(401, c.post("/app/webhook/github").body_bytes(body.to_vec()).send().status());
}

#[test]
fn sse_events_encode() {
    assert_eq!(b"id: 3\ndata: {\"n\":3}\n\n".to_vec(), sse_event(3).encode());
    let r = client().get("/app/sse?count=2&every_ms=1").send();
    assert_eq!(200, r.status());
    assert!(r.header("Content-Type").unwrap_or("").starts_with("text/event-stream"));
}

#[test]
fn template_escapes() {
    let c = client();
    let r = c.get("/app/hello?name=x").send();
    assert_eq!(200, r.status());
    assert_eq!("<p>Hello, x</p>", r.body_text());
    let r = c.get("/app/hello?name=%3Cb%3E").send();
    assert_eq!("<p>Hello, &lt;b&gt;</p>", r.body_text());
}

#[test]
fn openapi_describes_todos() {
    let c = client();
    let r = c.get("/openapi.json").send();
    assert_eq!(200, r.status());
    let spec: serde_json::Value = serde_json::from_str(r.body_text()).unwrap();
    assert_eq!("3.1.0", spec["openapi"]);
    assert!(spec["paths"]["/app/todos"]["post"].is_object(), "{spec}");
    assert!(spec["paths"]["/app/todos/{id}"]["get"].is_object());
    assert!(spec["paths"]["/app/todos/{id}"]["delete"].is_object());
    assert_eq!(200, c.get("/docs").send().status());
}

fn mcp_call(c: &TestClient<Root>, token: &str, session: Option<&str>, body: &str) -> TestResponse {
    let mut req = c
        .post("/mcp")
        .header("Authorization", &format!("Bearer {token}"))
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2025-06-18");
    if let Some(s) = session {
        req = req.header("Mcp-Session-Id", s);
    }
    req.body_json(body).send()
}

#[test]
fn mcp_tool_resource_prompt() {
    let c = client_with(|cfg| cfg.mcp_token = Some("mcp-secret".into()));
    let init = c
        .post("/mcp")
        .header("Authorization", "Bearer mcp-secret")
        .header("Accept", "application/json, text/event-stream")
        .body_json(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}"#)
        .send();
    assert_eq!(200, init.status(), "{}", init.body_text());
    let session = init.header("Mcp-Session-Id").expect("session id").to_string();

    let r = mcp_call(&c, "mcp-secret", Some(&session), r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"add","arguments":{"a":2,"b":40}}}"#);
    assert_eq!(200, r.status(), "{}", r.body_text());
    assert!(r.body_text().contains(r#""text":"42""#), "{}", r.body_text());

    let r = mcp_call(&c, "mcp-secret", Some(&session), r#"{"jsonrpc":"2.0","id":3,"method":"resources/read","params":{"uri":"e2e://hello"}}"#);
    assert!(r.body_text().contains("hello from rws-e2e-app"), "{}", r.body_text());

    let r = mcp_call(&c, "mcp-secret", Some(&session), r#"{"jsonrpc":"2.0","id":4,"method":"prompts/get","params":{"name":"greet","arguments":{"name":"Ann"}}}"#);
    assert!(r.body_text().contains("Say hello to Ann."), "{}", r.body_text());

    let r = mcp_call(&c, "wrong", Some(&session), r#"{"jsonrpc":"2.0","id":5,"method":"tools/list"}"#);
    assert_eq!(401, r.status());
}

#[test]
fn mcp_without_token_is_503() {
    let r = client().post("/mcp").body_json(r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#).send();
    assert_eq!(503, r.status());
    assert!(r.body_text().contains("MCP_TOKEN"));
}

#[test]
fn a2a_echoes() {
    let c = client();
    let card = c.get("/.well-known/agent-card.json").send();
    assert_eq!(200, card.status());
    assert!(card.body_text().contains("rws-e2e echo agent"));
    let r = c
        .post("/a2a")
        .header("A2A-Version", "1.0")
        .body_json(r#"{"jsonrpc":"2.0","id":1,"method":"SendMessage","params":{"message":{"messageId":"m1","role":"ROLE_USER","parts":[{"text":"ping 123"}]}}}"#)
        .send();
    assert_eq!(200, r.status(), "{}", r.body_text());
    let v: serde_json::Value = serde_json::from_str(r.body_text()).unwrap();
    let text = v.pointer("/result/message/parts/0/text").and_then(|t| t.as_str());
    assert_eq!(Some("ping 123"), text, "{v}");
}

#[test]
fn llm_mock() {
    let c = client();
    let models = c.get("/v1/models").send();
    assert!(models.body_text().contains(r#""id":"mock-1""#));
    let body = r#"{"model":"mock-1","messages":[{"role":"system","content":"s"},{"role":"user","content":"first"},{"role":"assistant","content":"a"},{"role":"user","content":"héllo"}]}"#;
    let r = c.post("/v1/chat/completions").body_json(body).send();
    assert_eq!(200, r.status());
    let v: serde_json::Value = serde_json::from_str(r.body_text()).unwrap();
    assert_eq!("chat.completion", v["object"]);
    assert_eq!("echo: héllo", v["choices"][0]["message"]["content"]);
    assert_eq!(15, v["usage"]["total_tokens"]);
    assert_eq!(10, v["usage"]["prompt_tokens"]);
    assert_eq!(5, v["usage"]["completion_tokens"]);

    let stream = body.replace(r#""model":"mock-1","#, r#""model":"mock-1","stream":true,"#);
    let r = c.post("/v1/chat/completions").body_json(&stream).send();
    assert_eq!(200, r.status());
    assert!(r.header("Content-Type").unwrap_or("").starts_with("text/event-stream"));
    let events: Vec<&str> = r.body_text().lines().filter_map(|l| l.strip_prefix("data: ")).collect();
    assert_eq!(Some(&"[DONE]"), events.last());
    let chunks: Vec<serde_json::Value> = events[..events.len() - 1].iter().map(|e| serde_json::from_str(e).unwrap()).collect();
    let content: String = chunks.iter().filter_map(|c| c["choices"][0]["delta"]["content"].as_str()).collect();
    assert_eq!("echo: héllo", content);
    assert_eq!(15, chunks.last().unwrap()["usage"]["total_tokens"]);
    assert_eq!(400, c.post("/v1/chat/completions").body_json("{}").send().status());
}

#[test]
fn missing_env_answers_503_naming_it() {
    let c = client();
    for (method, path, needle) in [
        ("POST", "/app/todos", "RWS_DB_NAME"),
        ("GET", "/app/todos/1", "RWS_DB_NAME"),
        ("POST", "/app/jobs?key=k", "RWS_DB_NAME"),
        ("GET", "/app/jobs/k", "RWS_DB_NAME"),
        ("POST", "/app/mail?to=a@e2e.test&subject=s", "SMTP_HOST"),
        ("PUT", "/app/files/a.txt", "S3_ENDPOINT"),
        ("GET", "/app/files/a.txt/presign", "S3_ENDPOINT"),
        ("GET", "/app/jwt/rs256", "JWT_RS256_PUBLIC_KEY"),
        ("GET", "/app/jwt/es256", "JWT_ES256_PUBLIC_KEY"),
        ("POST", "/app/webhook/github", "WEBHOOK_SECRET"),
    ] {
        let r = match method {
            "GET" => c.get(path).send(),
            "PUT" => c.put(path).body_text("x").send(),
            _ => c.post(path).body_json(r#"{"title":"t"}"#).send(),
        };
        assert_eq!(503, r.status(), "{method} {path}: {}", r.body_text());
        assert!(r.body_text().contains(needle), "{method} {path}: {}", r.body_text());
    }
}

#[test]
fn presign_with_s3_configured() {
    let c = client_with(|cfg| {
        cfg.s3 = Ok(S3 {
            storage: rust_web_server::storage::S3Storage::new(s3_config()),
            config: s3_config(),
        })
    });
    let r = c.get("/app/files/report.txt/presign").send();
    assert_eq!(200, r.status());
    let url = r.body_text();
    assert!(url.starts_with("https://s3.e2e.test/bucket/report.txt?X-Amz-Algorithm=AWS4-HMAC-SHA256&"), "{url}");
    assert!(url.contains("X-Amz-Expires=300") && url.contains("X-Amz-Signature="));
}

fn s3_config() -> rust_web_server::storage::S3Config {
    rust_web_server::storage::S3Config {
        bucket: "bucket".into(),
        region: "us-east-1".into(),
        access_key: "AK".into(),
        secret_key: "SK".into(),
        endpoint: "https://s3.e2e.test".into(),
    }
}

#[test]
fn pod_name() {
    assert_eq!("test-pod", client().get("/app/pod").send().body_text());
}
