---
title: Sign-up and login
description: Account flows as one middleware — sign-up, login, logout, password reset and email confirmation, as JSON or HTML forms.
---

`auth_flows::AuthFlows` is the account part of an app — sign-up, login, logout, password reset and email confirmation — as one middleware, for JSON clients and HTML forms alike (18.35.0). It needs the `crypto`, `mailer` and `serde` features (all in the `web` preset).

```rust,no_run
use std::sync::Arc;
use rust_web_server::app::App;
use rust_web_server::auth_flows::{signed_in, AuthFlows, CurrentUser, DbUserStore};
use rust_web_server::flash::FlashLayer;
use rust_web_server::mailer::Mailer;

let store = DbUserStore::new(pool.clone()).await?;                 // or your own UserStore
let auth = AuthFlows::new(secret.as_bytes(), "https://notes.example", store, Mailer::from_env()?)
    .app_name("Notes");

let app = App::with_async_state(pool)
    .group("/api", |api| api.guard(signed_in).route(list_notes))  // 401 without a session
    .wrap(FlashLayer::new())   // first = outermost: it clears the flash the /auth pages show
    .wrap(auth);

#[get("/notes")]
async fn list_notes(user: CurrentUser, State(pool): State<DbPool>) -> Result<Json<Vec<Note>>, AppError> {
    // user.id, user.email, user.confirmed
}
```

`secret` signs sessions and links: at least 32 random bytes, the same on every replica, from configuration — never in the code. `base_url` starts the links in emails; it's never taken from the request's `Host` header, which a client chooses.

## Endpoints

A request with `Content-Type: application/json` gets JSON (errors as problem details); a form post gets a redirect with a [flash message](/building-apps/forms-uploads/#flash-messages-and-redirects).

| Request | JSON | Form |
|---|---|---|
| `POST /auth/signup` `email`, `password` | `201 {user}`, signed in — or `{user, confirmation_required}`; `409` taken, `422` invalid | redirect to the after-login path |
| `POST /auth/login` `email`, `password` | `200 {user}`; `401` wrong; `403` unconfirmed (with `require_confirmation`) | redirect to `return_to` (a local path) or the after-login path |
| `POST /auth/logout` | `204` | redirect to the after-logout path |
| `GET /auth/me` | `200 {user}` or `401` | — |
| `POST /auth/password/forgot` `email` | `202` whether or not the account exists | redirect to login |
| `POST /auth/password/reset` `token`, `password` | `200 {user}`, signed in; `400` bad or used link | redirect |
| `GET /auth/confirm?token=` (the email's link), `POST /auth/confirm` `token` | `200 {user}`; `400` | redirect |
| `POST /auth/confirm/resend` `email` | `202` | redirect to login |
| `GET /auth/login`, `/auth/signup`, `/auth/password/forgot`, `/auth/password/reset?token=` | — | the forms |

`{user}` is `{"id", "email", "confirmed"}`. Sign-ups, logins and the email requests are rate limited per client IP: 10 a minute each by default. The address is the one a trusted proxy forwarded ([`trusted_proxies`](/configuration/env-vars/): loopback and private ranges by default), so behind a load balancer on a private network each client has its own budget; a balancer with a public address must be added to the list, or every request counts as its own.

## Where it goes in the stack

The first layer wrapped is the outermost. `AuthFlows` answers the `/auth` requests itself, so layers that must see them go before it: `FlashLayer` (to clear the messages the forms show), `CsrfLayer` (to give the forms their token and check the posts), `ErrorPages`. Layers that need to know who's signed in — a [`ModelAdmin`](/database/model-admin/), your own — go after it.

```rust,no_run
let app = app.wrap(ErrorPages::from_env()).wrap(FlashLayer::new()).wrap(CsrfLayer::new()).wrap(auth).wrap(admin);
```

## In the app

For every other request the middleware reads the session and hands the account on:

- `CurrentUser` — a handler argument (`401` without a session), or `CurrentUser::of(request)` for an `Option`.
- `signed_in` — a [route guard](/building-apps/routing/#groups-and-guards) answering `401`.
- `signed_in_or_redirect("/auth/login")` — a guard for pages: a redirect to the login form with `return_to` set.

Headers a client sends to pose as someone are removed, and `CurrentUser` only believes values the middleware itself set. Requests with no session cookie, no such headers and outside `/auth` pass straight through, so async handlers stay on the async path.

## Sessions and links

A session is a signed cookie (`rws_auth`: `HttpOnly`, `SameSite=Lax`, `Secure` when the base URL is `https`, 30 days) holding the user id, its expiry and a fingerprint of the password hash. Nothing is stored server side, so any replica can check it, and **changing the password ends every session** — logout clears the cookie on that browser.

The links in emails carry tokens signed the same way. A reset link is tied to the current password hash, so it works once, and for an hour; a confirmation link works for three days. A finished password reset also confirms the email (the link reached the inbox). A failed login takes as long for an unknown email as for a wrong password, and "forgot password" answers the same either way, so neither tells who has an account.

To sign someone in from your own handler (after an OAuth login, say), add `auth.session_cookie(&user)` as a `Set-Cookie` header.

## Accounts

`UserStore` is five methods — `find_by_email`, `find_by_id`, `create`, `set_password_hash`, `confirm` — over an `AuthUser { id, email, password_hash, confirmed }`. Implement it over your own users table, or use:

| Store | Where accounts live |
|---|---|
| `DbUserStore::new(pool)` | The `rws_users` table (`model-*` features), created if absent: `id`, `email` (unique), `password_hash`, `confirmed`, `created_at` |
| `MemoryUserStore::new()` | Memory — tests and trying things out |

Emails are lower-cased and trimmed; passwords are hashed with Argon2id (`crypto::hash_password`).

## Settings

| Builder | Default |
|---|---|
| `.prefix("/account")` | `/auth` |
| `.app_name("Notes")` | "the app" (in email subjects and text) |
| `.require_confirmation(true)` | off: sign in at once; the confirmation email goes out either way |
| `.min_password_length(n)` | 8 |
| `.session_ttl(duration)` | 30 days |
| `.cookie_name(name)`, `.secure_cookie(bool)` | `rws_auth`; `Secure` when the base URL is `https` |
| `.after_login(path)`, `.after_logout(path)` | `/` |
| `.rate_limit(max, window_secs)` | 10 per 60 s per IP and action; `0` turns it off |
| `.templates(Arc<TeraEngine>)` (`tera`) | the built-in pages and plain-text emails |
| `.on_signup(\|user\| …)` | none; called with each new account (a welcome email, a default workspace); enqueue anything slow |

## Your own pages and emails

With `.templates(engine)`, each of these that the engine has replaces the built-in one:

| Template | Context |
|---|---|
| `auth/login.html`, `auth/signup.html`, `auth/forgot.html`, `auth/reset.html` | [`page_context`](/building-apps/templates/#view-helpers) (`flash`, `notice`, `alert`, `csrf_field`, `path`) plus `prefix`, `token`, `return_to` |
| `auth/mail/confirm.txt` / `.html`, `auth/mail/reset.txt` / `.html` | `link`, `app`, `email` |

Forms post the field names in the table above. With [`CsrfLayer`](/features/csrf/) outside `AuthFlows` — wrapped before it, as the first layer wrapped is the outermost: `.wrap(FlashLayer::new()).wrap(CsrfLayer::new()).wrap(auth)` — the built-in forms carry the token; your templates add `{{ csrf_field | safe }}`, and JSON clients send `X-CSRF-Token`.

## Testing

`TestMailbox` is the mailer that keeps what it's sent, so a test can follow a link:

```rust,no_run
let mailbox = TestMailbox::new();
let auth = AuthFlows::new(SECRET, "https://app.example", MemoryUserStore::new(), mailbox.clone());
let client = TestClient::new(app().wrap(auth));

client.post("/auth/password/forgot").body_json(r#"{"email":"ann@example.com"}"#).send();
let text = mailbox.last_to("ann@example.com").unwrap().email.text.unwrap();
let token = text.split("token=").nth(1).unwrap().split_whitespace().next().unwrap();
```
