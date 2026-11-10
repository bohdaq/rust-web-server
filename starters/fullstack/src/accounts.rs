//! Accounts: sign-up, login, password reset and email confirmation under
//! `/auth` (rws's `AuthFlows`), a welcome email sent by a background job,
//! and where mail goes.
//!
//! Settings (environment):
//!
//! | Variable | Without it |
//! |---|---|
//! | `AUTH_SECRET` (32+ bytes) | a random secret: sessions end when the app restarts |
//! | `APP_URL` (`https://notes.example`) | `http://127.0.0.1:7878`, the start of the links in emails |
//! | `RWS_SMTP_HOST`, `RWS_SMTP_FROM`, … | mail is kept in memory (`/_mail` previews with `RWS_ENV=dev`) |
//! | `ADMIN_EMAILS` (`ann@example.com,bo@example.com`) | nobody gets into the model admin at `/admin` (PostgreSQL only) |
//!
//! Accounts live in the `rws_users` table with PostgreSQL, in memory otherwise.
//! Sign-ins are rate limited per client address: the one the kube-dc
//! gateway forwards (its private address is trusted by default).

use std::sync::Arc;

use rust_web_server::auth_flows::{AuthFlows, AuthUser, DbUserStore, MemoryUserStore, UserStore};
use rust_web_server::jobs::JobQueue;
use rust_web_server::mailer::{Deliver, Email, Mail, Mailer, MailPreviews, MailerError, TestMailbox};

use crate::store::Store;

/// The auth flows and what they send mail with.
#[derive(Clone)]
pub struct Accounts {
    pub auth: AuthFlows,
    /// Where mail goes when no SMTP server is configured (and in tests).
    pub mailbox: Option<TestMailbox>,
    /// Background jobs: the welcome email.
    pub jobs: Arc<JobQueue>,
}

/// The welcome email for `email`.
pub fn welcome(email: &str) -> Result<Mail, MailerError> {
    Ok(Email::builder()
        .to(email)
        .subject("Welcome to {{project-name}}")
        .text("Thanks for signing up. Your notes are waiting.")
        .html("<p>Thanks for signing up. Your notes are waiting.</p>")
        .build()?
        .into())
}

impl Accounts {
    /// From the environment (see the module docs), accounts in `store`'s database.
    pub async fn from_env(store: &Store) -> Result<Accounts, String> {
        let users: Arc<dyn UserStore> = match store.pool() {
            Some(pool) => Arc::new(DbUserStore::new(pool.clone()).await.map_err(|e| format!("rws_users: {e}"))?),
            None => Arc::new(MemoryUserStore::new()),
        };
        let (transport, mailbox): (Arc<dyn Deliver>, Option<TestMailbox>) = match std::env::var("RWS_SMTP_HOST") {
            Ok(_) => (Arc::new(Mailer::from_env().map_err(|e| e.to_string())?), None),
            Err(_) => {
                println!("RWS_SMTP_HOST is not set: mail is kept in memory.");
                let mailbox = TestMailbox::new();
                (Arc::new(mailbox.clone()), Some(mailbox))
            }
        };
        let secret = match std::env::var("AUTH_SECRET") {
            Ok(secret) if secret.len() >= 32 => secret.into_bytes(),
            Ok(_) => return Err("AUTH_SECRET needs at least 32 bytes".into()),
            Err(_) => {
                println!("AUTH_SECRET is not set: a random one, so sessions end when the app restarts.");
                rust_web_server::crypto::generate_token(32).into_bytes()
            }
        };
        let base_url = std::env::var("APP_URL").unwrap_or_else(|_| "http://127.0.0.1:7878".into());
        Ok(Accounts::new(&secret, &base_url, users, transport, mailbox))
    }

    /// Accounts in memory, mail in a `TestMailbox` (tests).
    pub fn in_memory() -> Accounts {
        let mailbox = TestMailbox::new();
        Accounts::new(b"a test secret that is at least 32 bytes long", "http://127.0.0.1:7878", Arc::new(MemoryUserStore::new()), Arc::new(mailbox.clone()), Some(mailbox))
    }

    fn new(secret: &[u8], base_url: &str, users: Arc<dyn UserStore>, transport: Arc<dyn Deliver>, mailbox: Option<TestMailbox>) -> Accounts {
        let jobs = Arc::new(JobQueue::new(2));
        let (queue, sender) = (Arc::clone(&jobs), Arc::clone(&transport));
        let auth = AuthFlows::with_arcs(secret, base_url, users, transport)
            .app_name("{{project-name}}")
            .after_login("/")
            .on_signup(move |user: &AuthUser| match welcome(&user.email) {
                // Sent by a worker: the sign-up doesn't wait for SMTP.
                Ok(mail) => mail.deliver_later(&queue, Arc::clone(&sender)),
                Err(e) => eprintln!("welcome email: {e}"),
            });
        Accounts { auth, mailbox, jobs }
    }

    /// The mail previews (`/_mail`), for `RWS_ENV=dev`.
    pub fn previews() -> MailPreviews {
        MailPreviews::new("/_mail").add("welcome", || welcome("ann@example.com"))
    }
}
