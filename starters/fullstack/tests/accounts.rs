//! Accounts through `site()`: sign up, the welcome email (sent by a
//! background job), `/api/account` with and without the session.

use std::time::{Duration, Instant};

use {{crate_name}}::{site, Accounts, Store};
use rust_web_server::test_client::TestClient;
use serde_json::Value;

#[test]
fn sign_up_then_see_the_account() {
    let accounts = Accounts::in_memory();
    let mailbox = accounts.mailbox.clone().unwrap();
    let client = TestClient::new(site(Store::memory(), &accounts));

    assert_eq!(401, client.get("/api/account").send().status());

    let created = client.post("/auth/signup").body_json(r#"{"email":"ann@example.com","password":"correct horse"}"#).send();
    assert_eq!(201, created.status(), "{}", created.body_text());
    let cookie = created
        .headers()
        .iter()
        .find(|h| h.value.starts_with("rws_auth="))
        .map(|h| h.value.split(';').next().unwrap().to_string())
        .expect("a session cookie");

    let me = client.get("/api/account").header("Cookie", &cookie).send();
    assert_eq!(200, me.status());
    let me: Value = serde_json::from_slice(me.body_bytes()).unwrap();
    assert_eq!("ann@example.com", me["email"]);

    // The confirmation email at once, the welcome email from the job queue.
    let deadline = Instant::now() + Duration::from_secs(5);
    while mailbox.count() < 2 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let subjects: Vec<String> = mailbox.sent().into_iter().map(|m| m.email.subject).collect();
    assert!(subjects.iter().any(|s| s.starts_with("Confirm your email")), "{subjects:?}");
    assert!(subjects.iter().any(|s| s.starts_with("Welcome to")), "{subjects:?}");

    // The notes API still works without an account.
    assert_eq!(200, client.get("/api/notes").send().status());
}

#[test]
fn the_login_form_is_served() {
    let client = TestClient::new(site(Store::memory(), &Accounts::in_memory()));
    let page = client.get("/auth/login").send();
    assert_eq!(200, page.status());
    assert!(page.body_text().contains("<form"));
}

#[test]
fn the_welcome_email_previews() {
    let welcome = {{crate_name}}::accounts::welcome("ann@example.com").unwrap();
    assert_eq!(vec!["ann@example.com".to_string()], welcome.email.to);
}
