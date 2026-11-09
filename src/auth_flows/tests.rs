use super::*;
use crate::app::App;
use crate::mailer::TestMailbox;
use crate::test_client::{TestClient, TestResponse};

const SECRET: &[u8] = b"0123456789abcdef0123456789abcdef-test";

fn setup(configure: impl FnOnce(AuthFlows) -> AuthFlows) -> (TestClient<impl Application>, TestMailbox, MemoryUserStore) {
    let mailbox = TestMailbox::new();
    let store = MemoryUserStore::new();
    let auth = configure(AuthFlows::new(SECRET, "https://app.example", store.clone(), mailbox.clone()).app_name("Notes"));
    let app = App::with_state(())
        .get("/", |_, _, _, _| Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "home"))
        .get("/whoami", |request, _, _, _| match CurrentUser::of(request) {
            Some(user) => Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, &format!("{} {}", user.email, user.confirmed)),
            None => Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "nobody"),
        })
        .group("/private", |g| g.guard(signed_in).get("/", |_, _, _, _| Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "secret")))
        .group("/pages", |g| g.guard(signed_in_or_redirect("/auth/login")).get("/{name}", |_, _, _, _| Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "page")))
        .wrap(auth);
    (TestClient::new(app), mailbox, store)
}

/// `name=value` of the `rws_auth` cookie a response sets.
fn session(response: &TestResponse) -> Option<String> {
    response
        .headers()
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("set-cookie") && h.value.starts_with("rws_auth="))
        .map(|h| h.value.split(';').next().unwrap().to_string())
}

fn token_in(mailbox: &TestMailbox) -> String {
    let text = mailbox.last().unwrap().email.text.unwrap();
    text.split("token=").nth(1).unwrap().split_whitespace().next().unwrap().to_string()
}

fn json_post<A: Application>(client: &TestClient<A>, path: &str, body: &str) -> TestResponse {
    client.post(path).body_json(body).send()
}

#[test]
fn signup_signs_in_and_sends_a_confirmation_link() {
    let (client, mailbox, _) = setup(|a| a);
    let created = json_post(&client, "/auth/signup", r#"{"email":" Ann@Example.com ","password":"correct horse"}"#);
    assert_eq!(201, created.status(), "{}", created.body_text());
    let body: serde_json::Value = serde_json::from_slice(created.body_bytes()).unwrap();
    assert_eq!("ann@example.com", body["user"]["email"]);
    let cookie = session(&created).unwrap();
    let set_cookie = created.headers().iter().find(|h| h.value.starts_with("rws_auth=")).unwrap().value.to_string();
    assert!(set_cookie.contains("HttpOnly") && set_cookie.contains("Secure") && set_cookie.contains("SameSite=Lax"), "{set_cookie}");

    let mail = mailbox.last().unwrap();
    assert_eq!(vec!["ann@example.com".to_string()], mail.email.to);
    assert!(mail.email.subject.contains("Notes"));
    assert!(mail.email.text.unwrap().contains("https://app.example/auth/confirm?token="));

    let me = client.get("/auth/me").header("Cookie", &cookie).send();
    assert_eq!(200, me.status());
    assert_eq!("ann@example.com false", client.get("/whoami").header("Cookie", &cookie).send().body_text());
    assert_eq!("secret", client.get("/private").header("Cookie", &cookie).send().body_text());
    assert_eq!(401, client.get("/private").send().status());
    assert_eq!(401, client.get("/auth/me").send().status());

    // Confirming flips the flag.
    let confirmed = client.get(&format!("/auth/confirm?token={}", token_in(&mailbox))).header("Accept", "application/json").send();
    assert_eq!(200, confirmed.status(), "{}", confirmed.body_text());
    assert_eq!("ann@example.com true", client.get("/whoami").header("Cookie", &cookie).send().body_text());
}

#[test]
fn signup_rejects_bad_input_and_taken_emails() {
    let (client, _, _) = setup(|a| a);
    assert_eq!(422, json_post(&client, "/auth/signup", r#"{"email":"not-an-email","password":"long enough"}"#).status());
    assert_eq!(422, json_post(&client, "/auth/signup", r#"{"email":"a@b.co","password":"short"}"#).status());
    assert_eq!(422, json_post(&client, "/auth/signup", r#"{"email":"a@b.co\r\nBcc: x@y.z","password":"long enough"}"#).status());
    assert_eq!(201, json_post(&client, "/auth/signup", r#"{"email":"a@b.co","password":"long enough"}"#).status());
    assert_eq!(409, json_post(&client, "/auth/signup", r#"{"email":"A@B.CO","password":"long enough"}"#).status());
}

#[test]
fn login_and_logout() {
    let (client, _, _) = setup(|a| a);
    json_post(&client, "/auth/signup", r#"{"email":"bob@example.com","password":"hunter2hunter2"}"#);
    assert_eq!(401, json_post(&client, "/auth/login", r#"{"email":"bob@example.com","password":"wrong password"}"#).status());
    assert_eq!(401, json_post(&client, "/auth/login", r#"{"email":"nobody@example.com","password":"hunter2hunter2"}"#).status());
    let ok = json_post(&client, "/auth/login", r#"{"email":"BOB@example.com","password":"hunter2hunter2"}"#);
    assert_eq!(200, ok.status());
    let cookie = session(&ok).unwrap();
    let out = client.post("/auth/logout").header("Cookie", &cookie).header("Accept", "application/json").send();
    assert_eq!(204, out.status());
    assert!(out.headers().iter().any(|h| h.value.starts_with("rws_auth=;") && h.value.contains("Max-Age=0")));
}

#[test]
fn tampered_or_expired_sessions_are_ignored() {
    let (client, _, store) = setup(|a| a);
    let cookie = session(&json_post(&client, "/auth/signup", r#"{"email":"c@example.com","password":"long enough"}"#)).unwrap();
    let mut tampered = cookie.clone();
    tampered.replace_range(tampered.len() - 2.., "00");
    assert_eq!("nobody", client.get("/whoami").header("Cookie", &tampered).send().body_text());
    // A session for an id whose account is gone.
    let flows = AuthFlows::new(SECRET, "https://app.example", store.clone(), TestMailbox::new());
    let ghost = AuthUser { id: "999".into(), email: "x@y.z".into(), password_hash: "h".into(), confirmed: true };
    let ghost_cookie = flows.session_cookie(&ghost).split(';').next().unwrap().to_string();
    assert_eq!("nobody", client.get("/whoami").header("Cookie", &ghost_cookie).send().body_text());
    // Signed with another secret.
    let other = AuthFlows::new(b"another secret that is long enough!!", "https://app.example", store.clone(), TestMailbox::new());
    let user = store.users()[0].clone();
    let foreign = other.session_cookie(&user).split(';').next().unwrap().to_string();
    assert_eq!("nobody", client.get("/whoami").header("Cookie", &foreign).send().body_text());
}

#[test]
fn identity_headers_from_the_client_are_dropped() {
    let (client, _, _) = setup(|a| a);
    let spoofed = client.get("/whoami").header("X-Rws-Auth-User-Id", "1").header("X-Rws-Auth-User-Email", "admin@example.com").header("X-Rws-Auth-Mark", "guess").send();
    assert_eq!("nobody", spoofed.body_text());
    assert_eq!(401, client.get("/private").header("X-Rws-Auth-User-Id", "1").send().status());
}

#[test]
fn current_user_needs_the_middleware() {
    let request = Request {
        method: "GET".into(),
        request_uri: "/".into(),
        http_version: "HTTP/1.1".into(),
        headers: vec![Header::new(USER_ID_HEADER, "1"), Header::new(USER_EMAIL_HEADER, "a@b.c")],
        body: Vec::new().into(),
    };
    assert_eq!(None, CurrentUser::of(&request));
    assert_eq!(401, CurrentUser::from_request(&request).unwrap_err().status_code);
}

#[test]
fn confirmation_can_be_required() {
    let (client, mailbox, _) = setup(|a| a.require_confirmation(true));
    let created = json_post(&client, "/auth/signup", r#"{"email":"d@example.com","password":"long enough"}"#);
    assert_eq!(201, created.status());
    assert!(session(&created).is_none());
    let body: serde_json::Value = serde_json::from_slice(created.body_bytes()).unwrap();
    assert_eq!(true, body["confirmation_required"]);
    assert_eq!(403, json_post(&client, "/auth/login", r#"{"email":"d@example.com","password":"long enough"}"#).status());

    // Resend: a new link, only for unconfirmed accounts that exist.
    assert_eq!(202, json_post(&client, "/auth/confirm/resend", r#"{"email":"nobody@example.com"}"#).status());
    assert_eq!(1, mailbox.count());
    assert_eq!(202, json_post(&client, "/auth/confirm/resend", r#"{"email":"d@example.com"}"#).status());
    assert_eq!(2, mailbox.count());

    let token = token_in(&mailbox);
    assert_eq!(200, json_post(&client, "/auth/confirm", &format!(r#"{{"token":"{token}"}}"#)).status());
    assert_eq!(400, json_post(&client, "/auth/confirm", r#"{"token":"bogus.00"}"#).status());
    assert_eq!(200, json_post(&client, "/auth/login", r#"{"email":"d@example.com","password":"long enough"}"#).status());
}

#[test]
fn password_reset_works_once_and_signs_out_other_sessions() {
    let (client, mailbox, _) = setup(|a| a);
    let old_session = session(&json_post(&client, "/auth/signup", r#"{"email":"e@example.com","password":"first password"}"#)).unwrap();
    mailbox.clear();

    // The same answer whether or not the account exists; mail only for one that does.
    assert_eq!(202, json_post(&client, "/auth/password/forgot", r#"{"email":"ghost@example.com"}"#).status());
    assert_eq!(0, mailbox.count());
    assert_eq!(202, json_post(&client, "/auth/password/forgot", r#"{"email":"E@example.com"}"#).status());
    let mail = mailbox.last().unwrap();
    assert!(mail.email.text.as_ref().unwrap().contains("https://app.example/auth/password/reset?token="));
    let token = token_in(&mailbox);

    assert_eq!(422, json_post(&client, "/auth/password/reset", &format!(r#"{{"token":"{token}","password":"x"}}"#)).status());
    let reset = json_post(&client, "/auth/password/reset", &format!(r#"{{"token":"{token}","password":"second password"}}"#));
    assert_eq!(200, reset.status(), "{}", reset.body_text());
    let new_session = session(&reset).unwrap();
    // Used once: the hash it was tied to is gone.
    assert_eq!(400, json_post(&client, "/auth/password/reset", &format!(r#"{{"token":"{token}","password":"third password"}}"#)).status());
    // Sessions from before the change are over; the new one works.
    assert_eq!("nobody", client.get("/whoami").header("Cookie", &old_session).send().body_text());
    assert_eq!("e@example.com true", client.get("/whoami").header("Cookie", &new_session).send().body_text());
    assert_eq!(401, json_post(&client, "/auth/login", r#"{"email":"e@example.com","password":"first password"}"#).status());
    assert_eq!(200, json_post(&client, "/auth/login", r#"{"email":"e@example.com","password":"second password"}"#).status());
    // A confirmation token isn't a reset token.
    assert_eq!(400, json_post(&client, "/auth/password/reset", r#"{"token":"","password":"long enough"}"#).status());
}

#[test]
fn tokens_are_checked_for_purpose_and_expiry() {
    let store = MemoryUserStore::new();
    let flows = AuthFlows::new(SECRET, "https://x.example", store.clone(), TestMailbox::new());
    let user = store.create("f@example.com", "hash").unwrap().unwrap();
    let confirm = flows.token("confirm", &user, 60);
    assert_eq!(Some(user.clone()), flows.check_token("confirm", &confirm));
    assert_eq!(None, flows.check_token("reset", &confirm));
    let expired = {
        let value = format!("confirm.{}.{}.{}", hex(user.id.as_bytes()), now() - 1, fingerprint(&user.email));
        signed_cookie(&hex(value.as_bytes()), SECRET)
    };
    assert_eq!(None, flows.check_token("confirm", &expired));
    assert_eq!(None, flows.check_token("confirm", "zz.zz"));
}

fn form<A: Application>(client: &TestClient<A>, path: &str, body: &str) -> TestResponse {
    client.post(path).header("Content-Type", "application/x-www-form-urlencoded").body_text(body).send()
}

#[test]
fn form_flows_redirect_with_flash() {
    let (client, _, _) = setup(|a| a.after_login("/dashboard").after_logout("/bye"));
    let created = form(&client, "/auth/signup", "email=g%40example.com&password=long+enough+pw");
    assert_eq!((303, Some("/dashboard")), (created.status(), created.header("location")));
    assert!(session(&created).is_some());
    assert!(created.headers().iter().any(|h| h.value.starts_with("rws_flash=notice=Welcome")));

    let wrong = form(&client, "/auth/login", "email=g%40example.com&password=nope&return_to=%2Fpages%2Fx");
    assert_eq!(303, wrong.status());
    assert_eq!(Some("/auth/login?return_to=%2Fpages%2Fx"), wrong.header("location"));
    assert!(wrong.headers().iter().any(|h| h.value.starts_with("rws_flash=alert=wrong")));

    let back = form(&client, "/auth/login", "email=g%40example.com&password=long+enough+pw&return_to=%2Fpages%2Fx");
    assert_eq!(Some("/pages/x"), back.header("location"));
    // Never off-site.
    let offsite = form(&client, "/auth/login", "email=g%40example.com&password=long+enough+pw&return_to=%2F%2Fevil.example");
    assert_eq!(Some("/dashboard"), offsite.header("location"));

    let out = form(&client, "/auth/logout", "");
    assert_eq!(Some("/bye"), out.header("location"));
}

#[test]
fn pages_render_forms_and_guards_redirect_to_them() {
    let (client, _, _) = setup(|a| a);
    let login = client.get("/auth/login?return_to=/pages/a").send();
    assert_eq!(200, login.status());
    let html = login.body_text();
    assert!(html.contains(r#"<form method="post" action="/auth/login">"#) && html.contains(r#"name="return_to" value="/pages/a""#), "{html}");
    assert!(client.get("/auth/signup").send().body_text().contains("Create an account"));
    assert!(client.get("/auth/password/forgot").send().body_text().contains("Email me a link"));
    let reset = client.get("/auth/password/reset?token=%22%3E%3Cscript%3E").send().body_text().to_string();
    assert!(reset.contains(r#"value="&quot;&gt;&lt;script&gt;""#), "{reset}");
    let flash = client.get("/auth/login").header("Cookie", "rws_flash=alert=%3Cb%3Ebad").send();
    assert!(flash.body_text().contains(r#"<p class="alert">&lt;b&gt;bad</p>"#));

    let guarded = client.get("/pages/a?x=1").send();
    assert_eq!((303, Some("/auth/login?return_to=%2Fpages%2Fa%3Fx%3D1")), (guarded.status(), guarded.header("location")));
    assert_eq!(404, client.get("/auth/nowhere").send().status());
    assert_eq!("home", client.get("/").send().body_text());
}

#[test]
fn attempts_are_rate_limited_per_client() {
    let (client, _, _) = setup(|a| a.rate_limit(3, 60));
    for _ in 0..3 {
        assert_eq!(401, json_post(&client, "/auth/login", r#"{"email":"h@example.com","password":"whatever pw"}"#).status());
    }
    let limited = json_post(&client, "/auth/login", r#"{"email":"h@example.com","password":"whatever pw"}"#);
    assert_eq!((429, Some("60")), (limited.status(), limited.header("retry-after")));
    // Separate budgets per action.
    assert_eq!(202, json_post(&client, "/auth/password/forgot", r#"{"email":"h@example.com"}"#).status());
}

#[test]
fn passthrough_only_without_anything_to_check() {
    let flows = AuthFlows::new(SECRET, "http://localhost", MemoryUserStore::new(), TestMailbox::new());
    let conn = ConnectionInfo { client: crate::server::Address { ip: "127.0.0.1".into(), port: 1 }, server: crate::server::Address { ip: "127.0.0.1".into(), port: 2 }, request_size: 16000, sni_hostname: None, tls: false };
    let req = |uri: &str, headers: Vec<Header>| Request { method: "GET".into(), request_uri: uri.into(), http_version: "HTTP/1.1".into(), headers, body: Vec::new().into() };
    assert!(flows.is_passthrough(&req("/x", vec![Header::new("Cookie", "a=1")]), &conn));
    assert!(!flows.is_passthrough(&req("/auth/login", vec![]), &conn));
    assert!(!flows.is_passthrough(&req("/x", vec![Header::new("Cookie", "rws_auth=1")]), &conn));
    assert!(!flows.is_passthrough(&req("/x", vec![Header::new("x-rws-auth-user-id", "1")]), &conn));
    // http: no Secure flag.
    let user = AuthUser { id: "1".into(), email: "a@b.c".into(), password_hash: "h".into(), confirmed: false };
    assert!(!flows.session_cookie(&user).contains("Secure"));
}

#[test]
#[should_panic(expected = "at least 32 bytes")]
fn short_secrets_are_refused() {
    AuthFlows::new(b"short", "https://x", MemoryUserStore::new(), TestMailbox::new());
}

#[cfg(feature = "tera")]
#[test]
fn templates_replace_the_built_in_pages_and_emails() {
    let engine = Arc::new(
        crate::template::TeraEngine::from_raw(&[
            ("auth/login.html", "custom login at {{ prefix | safe }} {{ return_to | safe }}"),
            ("auth/mail/confirm.txt", "Hello {{ email }}, confirm for {{ app }}: {{ link }}"),
        ])
        .unwrap(),
    );
    let (client, mailbox, _) = setup(|a| a.templates(engine));
    assert_eq!("custom login at /auth /p", client.get("/auth/login?return_to=/p").send().body_text());
    assert!(client.get("/auth/signup").send().body_text().contains("<form"), "built in when there's no template");
    json_post(&client, "/auth/signup", r#"{"email":"i@example.com","password":"long enough"}"#);
    let text = mailbox.last().unwrap().email.text.unwrap();
    assert!(text.starts_with("Hello i@example.com, confirm for Notes: https://app.example/auth/confirm?token="), "{text}");
}

#[cfg(feature = "model-sqlite")]
#[test]
fn the_database_store() {
    let pool = rws_server::async_bridge::block_on_shared(async { crate::model::DbPool::memory().await.unwrap() });
    let store = rws_server::async_bridge::block_on_shared(DbUserStore::new(pool.clone())).unwrap();
    let user = store.create("j@example.com", "hash1").unwrap().unwrap();
    assert_eq!(32, user.id.len());
    assert_eq!(None, store.create("j@example.com", "hash2").unwrap());
    assert_eq!(Some(user.clone()), store.find_by_email("j@example.com").unwrap());
    store.set_password_hash(&user.id, "hash3").unwrap();
    store.confirm(&user.id).unwrap();
    assert_eq!(
        Some(AuthUser { password_hash: "hash3".into(), confirmed: true, ..user.clone() }),
        store.find_by_id(&user.id).unwrap()
    );
    assert_eq!(None, store.find_by_id("nope").unwrap());
    // The whole flow on it.
    let mailbox = TestMailbox::new();
    let auth = AuthFlows::new(SECRET, "https://app.example", store, mailbox);
    let client = TestClient::new(App::with_state(()).wrap(auth));
    assert_eq!(201, json_post(&client, "/auth/signup", r#"{"email":"k@example.com","password":"long enough"}"#).status());
    assert_eq!(200, json_post(&client, "/auth/login", r#"{"email":"k@example.com","password":"long enough"}"#).status());
}

#[test]
fn signup_hooks_see_each_new_account() {
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let s = Arc::clone(&seen);
    let (client, _, _) = setup(move |a| a.on_signup(move |user| s.lock().unwrap().push(user.email.clone())));
    json_post(&client, "/auth/signup", r#"{"email":"l@example.com","password":"long enough"}"#);
    json_post(&client, "/auth/signup", r#"{"email":"l@example.com","password":"long enough"}"#); // taken: no hook
    assert_eq!(vec!["l@example.com".to_string()], *seen.lock().unwrap());
}
