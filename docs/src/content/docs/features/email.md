---
title: Email / SMTP
description: Send transactional email from handlers using the built-in SMTP mailer — plain, STARTTLS, and SMTPS; multipart text+HTML; AUTH PLAIN.
---

`rust-web-server` ships a hand-rolled SMTP client in `src/mailer/mod.rs` — no third-party mail library required. Enable the `mailer` feature, set `RWS_SMTP_*` environment variables, and call `Mailer::send()` from any handler.

STARTTLS and SMTPS (TLS) additionally require the `http-client` or `http2` feature at compile time. Plain SMTP on port 25 works with only `mailer`.

```toml
# Cargo.toml
[dependencies]
# STARTTLS or SMTPS (most SMTP providers)
rust-web-server = { version = "18", features = ["mailer", "http-client"] }

# Plain SMTP / local relay only
rust-web-server = { version = "18", features = ["mailer"] }
```

## Quick start

```rust,no_run
use rust_web_server::mailer::{Email, Mailer};

let mailer = Mailer::from_env().expect("SMTP not configured");

let email = Email::builder()
    .to("user@example.com")
    .subject("Welcome!")
    .text("Thanks for signing up.")
    .build()
    .unwrap();

mailer.send(&email).expect("send failed");
```

## Configuration

`Mailer::from_env()` reads the following environment variables:

| Variable | Default | Required | Description |
|---|---|---|---|
| `RWS_SMTP_HOST` | — | **Yes** | SMTP server hostname |
| `RWS_SMTP_PORT` | `587` | No | Port (25 = relay, 587 = STARTTLS, 465 = SMTPS) |
| `RWS_SMTP_USER` | — | No | SMTP username; omit to skip AUTH |
| `RWS_SMTP_PASSWORD` | — | No | SMTP password |
| `RWS_SMTP_FROM` | — | **Yes** | Envelope and `From:` address |
| `RWS_SMTP_TLS` | `starttls` | No | `starttls`, `smtps`, or `none` |
| `RWS_SMTP_TIMEOUT_MS` | `10000` | No | Connect / read / write timeout in milliseconds |

### Provider examples

**Gmail (STARTTLS, port 587):**
```bash
RWS_SMTP_HOST=smtp.gmail.com
RWS_SMTP_PORT=587
RWS_SMTP_TLS=starttls
RWS_SMTP_USER=you@gmail.com
RWS_SMTP_PASSWORD=app-password   # create an App Password in Google Account settings
RWS_SMTP_FROM=you@gmail.com
```

**SendGrid (STARTTLS, port 587):**
```bash
RWS_SMTP_HOST=smtp.sendgrid.net
RWS_SMTP_PORT=587
RWS_SMTP_TLS=starttls
RWS_SMTP_USER=apikey
RWS_SMTP_PASSWORD=SG.xxxx
RWS_SMTP_FROM=noreply@yourdomain.com
```

**Amazon SES (STARTTLS, port 587):**
```bash
RWS_SMTP_HOST=email-smtp.us-east-1.amazonaws.com
RWS_SMTP_PORT=587
RWS_SMTP_TLS=starttls
RWS_SMTP_USER=AKIAIOSFODNN7EXAMPLE
RWS_SMTP_PASSWORD=<SES SMTP password>
RWS_SMTP_FROM=noreply@yourdomain.com
```

**Local relay (no TLS, port 25):**
```bash
RWS_SMTP_HOST=127.0.0.1
RWS_SMTP_PORT=25
RWS_SMTP_TLS=none
RWS_SMTP_FROM=app@internal
```

## Direct construction

Bypass env vars and build a `Mailer` directly:

```rust,no_run
use rust_web_server::mailer::{Mailer, SmtpTls};

let mailer = Mailer {
    host: "smtp.gmail.com".into(),
    port: 587,
    user: Some("you@gmail.com".into()),
    password: Some("app-password".into()),
    from: "you@gmail.com".into(),
    tls: SmtpTls::Starttls,
    timeout_ms: 15_000,
};
```

## Building emails

`Email::builder()` returns an `EmailBuilder` with a fluent API:

```rust,no_run
use rust_web_server::mailer::Email;

let email = Email::builder()
    .to("user@example.com")          // one or more To: recipients
    .to("other@example.com")
    .cc("manager@example.com")       // optional Cc:
    .bcc("archive@example.com")      // envelope only; not in headers
    .reply_to("support@example.com") // optional Reply-To:
    .subject("Your order shipped")
    .text("Your package is on the way.")  // plain-text body
    .html("<p>Your package is <b>on the way</b>.</p>")  // HTML body
    .build()
    .unwrap();
```

`build()` validates:

- At least one `to` address
- Non-empty `subject`
- At least one of `text` or `html`

When both `text` and `html` are provided, the message is sent as `multipart/alternative` so email clients can display whichever format they support.

## Sharing the mailer in handlers

For production use, put the mailer in your application state so every handler can access it:

```rust,no_run
use std::sync::Arc;
use rust_web_server::app::App;
use rust_web_server::mailer::{Email, Mailer};
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;

struct State {
    mailer: Arc<Mailer>,
}

let state = State {
    mailer: Arc::new(Mailer::from_env().unwrap()),
};

let app = App::with_state(state)
    .post("/register", |req, _params, _conn, state| {
        // … validate the request body …

        let email = Email::builder()
            .to("new_user@example.com")
            .subject("Confirm your email")
            .text("Please confirm: https://example.com/confirm?token=xxxx")
            .html("<p>Click <a href=\"https://example.com/confirm?token=xxxx\">here</a> to confirm.</p>")
            .build()
            .unwrap();

        // In production, send in a background task (see scheduler docs)
        if let Err(e) = state.mailer.send(&email) {
            eprintln!("email send error: {e}");
        }

        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        r.content_range_list = vec![Range::get_content_range(
            b"registered".to_vec(), MimeType::TEXT_PLAIN.to_string(),
        )];
        r
    });
```

## Templates

With the `tera` feature, `render(name, &ctx)` on the builder sets the bodies from the templates `<name>.txt` and `<name>.html` of the global engine ([`template::init`](/building-apps/templates/)), whichever exist (18.35.0):

```rust,no_run
use rust_web_server::mailer::Email;
use rust_web_server::template::Context;

let mut ctx = Context::new();
ctx.insert("name", "Ann");
let email = Email::builder()
    .to("ann@example.com")
    .subject("Welcome")
    .render("mail/welcome", &ctx)?     // templates/mail/welcome.txt and .html
    .build()?;
```

`render_with(&engine, name, &ctx)` uses a given `TeraEngine`. HTML templates are escaped as usual; `.txt` ones aren't.

## Attachments

A `Mail` is an `Email` plus attachments; `Mailer::send_mail` sends it as `multipart/mixed` with each file in base64:

```rust,no_run
use rust_web_server::mailer::{Mail, MailAttachment};

let mail = Mail::new(email).attach(MailAttachment::new("invoice.pdf", "application/pdf", pdf_bytes));
mailer.send_mail(&mail)?;
```

## Transports and tests

`Deliver` is what sends a `Mail`: `Mailer` (SMTP), `TestMailbox`, or your own (an email API over HTTP). Code that takes an `Arc<dyn Deliver>` can be tested without SMTP:

```rust
use std::sync::Arc;
use rust_web_server::mailer::{Deliver, Email, Mail, TestMailbox};

let mailbox = TestMailbox::new();
let transport: Arc<dyn Deliver> = Arc::new(mailbox.clone());
transport.deliver(&Mail::new(Email::builder().to("ann@example.com").subject("Hi").text("Hello").build().unwrap())).unwrap();
assert_eq!("Hi", mailbox.last_to("ann@example.com").unwrap().email.subject);
```

`TestMailbox` keeps everything it's given (`sent`, `last`, `last_to`, `count`, `clear`; clones share it) — a transport for local development too.

## Sending later

Sending takes a network round trip; a request shouldn't wait for it:

```rust,no_run
// In memory (`jobs`): sent on the queue's workers, with its retries.
mail.deliver_later(&job_queue, Arc::clone(&transport));

// Persistent (`jobs` + a `model-*` feature): survives a restart, retried like any job.
rust_web_server::mailer::register_delivery(&persistent_queue, transport);   // once, in the worker process
mail.enqueue(&persistent_queue, EnqueueOptions::new().queue("mailers")).await?;
```

A persistent mail is a [job](/features/jobs/#persistentjobqueue) of type `rws.mail` whose payload is `Mail::to_payload()`.

## Previews

`MailPreviews` is a middleware that shows what each email looks like, built from sample data, without sending anything — mount it **in development only**:

```rust,no_run
use rust_web_server::mailer::{Email, MailPreviews};

let previews = MailPreviews::new("/_mail")
    .add("welcome", || Ok(Email::builder().to("ann@example.com").subject("Welcome").render("mail/welcome", &sample_context())?.build()?.into()));
let app = app.wrap(previews);
```

`/_mail` lists the previews; `/_mail/welcome` shows the headers, attachments, the HTML body in a sandboxed frame and the text body; `/_mail/welcome.html` and `.txt` are the bodies alone.

## Use cases

| Use case | Setup |
|---|---|
| Password reset, email confirmation | Built in: [sign-up and login](/features/auth-flows/) |
| Transactional notifications | Order confirmation, invoice, shipping notification |
| Alerts | Background scheduler job sends alert emails on threshold events |

## TLS modes

| Mode | Port | When to use |
|---|---|---|
| `SmtpTls::Starttls` (default) | 587 | Supported by virtually all providers; upgrades from plain to TLS |
| `SmtpTls::Smtps` | 465 | Implicit TLS from the first byte; required by some providers |
| `SmtpTls::None` | 25 | Local relay (`localhost`), internal networks, or providers that handle TLS at the network edge |

:::note[TLS requirements]
STARTTLS and SMTPS require `rustls`, which is included when you compile with `http-client` or `http2` features. If you compile with only `mailer` (no TLS features), `send()` returns `Err(MailerError::Smtp(...))` when `SmtpTls::Starttls` or `SmtpTls::Smtps` is configured.
:::

## Error handling

`Mailer::send()` returns `Result<(), MailerError>`:

```rust,no_run
use rust_web_server::mailer::{Mailer, MailerError};

match mailer.send(&email) {
    Ok(()) => println!("sent"),
    Err(MailerError::MissingConfig(var)) => eprintln!("missing env var: {var}"),
    Err(MailerError::Io(e))             => eprintln!("network error: {e}"),
    Err(MailerError::Smtp(msg))         => eprintln!("SMTP rejected: {msg}"),
    Err(MailerError::Build(msg))        => eprintln!("bad email: {msg}"),
}
```

## Implementation notes

- **No third-party SMTP library.** The client hand-rolls RFC 5321 SMTP: TCP connect → EHLO → optional STARTTLS or SMTPS → optional AUTH PLAIN → MAIL FROM → RCPT TO (one per recipient) → DATA → message → QUIT.
- **RFC 5322 message builder.** `build_message()` assembles headers (`From`, `To`, `Cc`, `Subject`, `Reply-To`, `MIME-Version`, `Content-Type`) and the body. When both text and HTML are provided, a `multipart/alternative` structure is generated.
- **SMTP dot-stuffing.** Lines in the message body that begin with `.` are prefixed with an extra `.` (RFC 5321 §4.5.2) so the DATA terminator (`\r\n.\r\n`) is not triggered prematurely.
- **AUTH PLAIN.** Credentials are encoded as base64(`\0username\0password`) in a single AUTH PLAIN command using the built-in `crate::core::base64::Base64::encode`.
- **Sync only.** `Mailer::send()` blocks the calling thread. In async handlers (`AsyncAppWithState`), wrap the call in `tokio::task::spawn_blocking`.
