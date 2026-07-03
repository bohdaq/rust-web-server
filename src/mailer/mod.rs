//! Transactional email via SMTP.
//!
//! [`Mailer`] opens a plain or TLS-encrypted TCP connection to an SMTP server,
//! optionally authenticates with `AUTH PLAIN`, and sends RFC 5321 / RFC 5322
//! email. No third-party mail library is required.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use rust_web_server::mailer::{Email, Mailer};
//!
//! let mailer = Mailer::from_env().expect("SMTP config missing");
//! let email = Email::builder()
//!     .to("user@example.com")
//!     .subject("Welcome!")
//!     .text("Thanks for signing up.")
//!     .build()
//!     .unwrap();
//! mailer.send(&email).expect("send failed");
//! ```
//!
//! # Environment variables
//!
//! | Variable | Default | Description |
//! |---|---|---|
//! | `RWS_SMTP_HOST` | — (required) | SMTP server hostname |
//! | `RWS_SMTP_PORT` | `587` | SMTP port |
//! | `RWS_SMTP_USER` | — | SMTP username; omit to skip AUTH |
//! | `RWS_SMTP_PASSWORD` | — | SMTP password |
//! | `RWS_SMTP_FROM` | — (required) | Envelope / `From:` address |
//! | `RWS_SMTP_TLS` | `starttls` | TLS mode: `starttls`, `smtps`, `none` |
//! | `RWS_SMTP_TIMEOUT_MS` | `10000` | Connect / read / write timeout in ms |
//!
//! # Provider examples
//!
//! **Gmail** (STARTTLS, port 587):
//! ```bash
//! RWS_SMTP_HOST=smtp.gmail.com RWS_SMTP_PORT=587 RWS_SMTP_TLS=starttls
//! RWS_SMTP_USER=you@gmail.com RWS_SMTP_PASSWORD=app-password
//! RWS_SMTP_FROM=you@gmail.com
//! ```
//!
//! **SendGrid** (STARTTLS, port 587):
//! ```bash
//! RWS_SMTP_HOST=smtp.sendgrid.net RWS_SMTP_PORT=587 RWS_SMTP_TLS=starttls
//! RWS_SMTP_USER=apikey RWS_SMTP_PASSWORD=SG.xxxx RWS_SMTP_FROM=you@domain.com
//! ```
//!
//! **Localhost relay** (no TLS, port 25):
//! ```bash
//! RWS_SMTP_HOST=127.0.0.1 RWS_SMTP_PORT=25 RWS_SMTP_TLS=none
//! RWS_SMTP_FROM=app@internal
//! ```

use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

#[cfg(test)]
mod tests;

// ── Error ──────────────────────────────────────────────────────────────────────

/// Error returned by [`Mailer::send`] and [`EmailBuilder::build`].
#[derive(Debug)]
pub enum MailerError {
    /// A required environment variable is missing.
    MissingConfig(String),
    /// An I/O error on the TCP or TLS stream.
    Io(std::io::Error),
    /// The SMTP server returned an unexpected reply code.
    Smtp(String),
    /// The [`EmailBuilder`] was given invalid or incomplete input.
    Build(String),
}

impl fmt::Display for MailerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MailerError::MissingConfig(s) => write!(f, "missing SMTP config: {s}"),
            MailerError::Io(e)            => write!(f, "SMTP I/O error: {e}"),
            MailerError::Smtp(s)          => write!(f, "SMTP error: {s}"),
            MailerError::Build(s)         => write!(f, "email build error: {s}"),
        }
    }
}

impl std::error::Error for MailerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let MailerError::Io(e) = self { Some(e) } else { None }
    }
}

impl From<std::io::Error> for MailerError {
    fn from(e: std::io::Error) -> Self { MailerError::Io(e) }
}

// ── TLS mode ───────────────────────────────────────────────────────────────────

/// TLS mode for the SMTP connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmtpTls {
    /// Plain TCP — no encryption. Suitable for localhost relays and trusted
    /// internal networks only.
    None,
    /// STARTTLS upgrade on the initial plain connection (default, port 587).
    /// Requires the `http-client` or `http2` feature at compile time.
    Starttls,
    /// Implicit TLS from the first byte (port 465 / SMTPS).
    /// Requires the `http-client` or `http2` feature at compile time.
    Smtps,
}

// ── Email ─────────────────────────────────────────────────────────────────────

/// An outgoing email message.
///
/// Construct with [`Email::builder`] then call [`Mailer::send`].
#[derive(Debug, Clone)]
pub struct Email {
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub text: Option<String>,
    pub html: Option<String>,
    pub reply_to: Option<String>,
}

impl Email {
    /// Start building an email.
    pub fn builder() -> EmailBuilder {
        EmailBuilder {
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: String::new(),
            text: None,
            html: None,
            reply_to: None,
        }
    }
}

/// Builder for [`Email`].
pub struct EmailBuilder {
    to: Vec<String>,
    cc: Vec<String>,
    bcc: Vec<String>,
    subject: String,
    text: Option<String>,
    html: Option<String>,
    reply_to: Option<String>,
}

impl EmailBuilder {
    /// Add a `To:` recipient.
    pub fn to(mut self, addr: &str) -> Self {
        self.to.push(addr.to_string());
        self
    }

    /// Add a `Cc:` recipient.
    pub fn cc(mut self, addr: &str) -> Self {
        self.cc.push(addr.to_string());
        self
    }

    /// Add a `Bcc:` recipient (envelope only; not in headers).
    pub fn bcc(mut self, addr: &str) -> Self {
        self.bcc.push(addr.to_string());
        self
    }

    /// Set the `Subject:` header.
    pub fn subject(mut self, s: &str) -> Self {
        self.subject = s.to_string();
        self
    }

    /// Set a plain-text body.
    pub fn text(mut self, body: &str) -> Self {
        self.text = Some(body.to_string());
        self
    }

    /// Set an HTML body.
    pub fn html(mut self, body: &str) -> Self {
        self.html = Some(body.to_string());
        self
    }

    /// Set the `Reply-To:` header.
    pub fn reply_to(mut self, addr: &str) -> Self {
        self.reply_to = Some(addr.to_string());
        self
    }

    /// Validate and build the [`Email`].
    pub fn build(self) -> Result<Email, MailerError> {
        if self.to.is_empty() {
            return Err(MailerError::Build(
                "at least one To: recipient is required".to_string(),
            ));
        }
        if self.subject.is_empty() {
            return Err(MailerError::Build("subject is required".to_string()));
        }
        if self.text.is_none() && self.html.is_none() {
            return Err(MailerError::Build(
                "at least one of text or html body is required".to_string(),
            ));
        }
        Ok(Email {
            to: self.to,
            cc: self.cc,
            bcc: self.bcc,
            subject: self.subject,
            text: self.text,
            html: self.html,
            reply_to: self.reply_to,
        })
    }
}

// ── Mailer ────────────────────────────────────────────────────────────────────

/// SMTP mailer.
///
/// Create via [`Mailer::from_env`] or construct directly. Call [`Mailer::send`]
/// to deliver an [`Email`].
///
/// # Example — Gmail with STARTTLS
///
/// ```rust,no_run
/// use rust_web_server::mailer::{Email, Mailer, SmtpTls};
///
/// let mailer = Mailer {
///     host: "smtp.gmail.com".into(),
///     port: 587,
///     user: Some("you@gmail.com".into()),
///     password: Some("app-password".into()),
///     from: "you@gmail.com".into(),
///     tls: SmtpTls::Starttls,
///     timeout_ms: 10_000,
/// };
/// ```
pub struct Mailer {
    pub host: String,
    pub port: u16,
    pub user: Option<String>,
    pub password: Option<String>,
    pub from: String,
    pub tls: SmtpTls,
    pub timeout_ms: u64,
}

impl Mailer {
    /// Construct a `Mailer` from `RWS_SMTP_*` environment variables.
    ///
    /// Required: `RWS_SMTP_HOST` and `RWS_SMTP_FROM`.
    pub fn from_env() -> Result<Self, MailerError> {
        let host = std::env::var("RWS_SMTP_HOST")
            .map_err(|_| MailerError::MissingConfig("RWS_SMTP_HOST".into()))?;
        let from = std::env::var("RWS_SMTP_FROM")
            .map_err(|_| MailerError::MissingConfig("RWS_SMTP_FROM".into()))?;
        let port: u16 = std::env::var("RWS_SMTP_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(587);
        let user = std::env::var("RWS_SMTP_USER").ok();
        let password = std::env::var("RWS_SMTP_PASSWORD").ok();
        let tls = match std::env::var("RWS_SMTP_TLS")
            .unwrap_or_else(|_| "starttls".into())
            .to_lowercase()
            .as_str()
        {
            "smtps" | "ssl" => SmtpTls::Smtps,
            "none" | "plain" => SmtpTls::None,
            _ => SmtpTls::Starttls,
        };
        let timeout_ms: u64 = std::env::var("RWS_SMTP_TIMEOUT_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10_000);
        Ok(Mailer { host, port, user, password, from, tls, timeout_ms })
    }

    /// Send an email.
    pub fn send(&self, email: &Email) -> Result<(), MailerError> {
        match self.tls {
            SmtpTls::None     => self.send_plain(email),
            SmtpTls::Starttls => self.send_starttls(email),
            SmtpTls::Smtps    => self.send_smtps(email),
        }
    }

    fn connect(&self) -> Result<TcpStream, MailerError> {
        let timeout = Duration::from_millis(self.timeout_ms);
        let stream = TcpStream::connect(format!("{}:{}", self.host, self.port))?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        Ok(stream)
    }

    fn send_plain(&self, email: &Email) -> Result<(), MailerError> {
        let stream = self.connect()?;
        smtp_full_session(
            stream,
            self.user.as_deref(),
            self.password.as_deref(),
            &self.from,
            email,
        )
    }

    fn send_starttls(&self, email: &Email) -> Result<(), MailerError> {
        #[cfg(not(any(feature = "http-client", feature = "http2")))]
        {
            return Err(MailerError::Smtp(
                "STARTTLS requires the http-client or http2 feature; \
                 set RWS_SMTP_TLS=none to use a local relay without TLS"
                    .into(),
            ));
        }

        #[cfg(any(feature = "http-client", feature = "http2"))]
        {
            let stream = self.connect()?;
            let mut conn: SmtpConn<TcpStream> = SmtpConn::new(stream);

            // Pre-TLS SMTP session — EHLO + STARTTLS, no AUTH
            conn.expect(220)?;
            conn.cmd(&format!("EHLO {}", smtp_local_hostname()))?;
            conn.expect(250)?;
            conn.cmd("STARTTLS")?;
            conn.expect(220)?;

            // Extract TcpStream; the BufReader buffer is empty here because
            // the server sends no bytes after `220 Go ahead` until we initiate
            // the TLS handshake.
            let tcp = conn.reader.into_inner();
            let tls = smtp_tls_connect(tcp, &self.host)?;

            // Full session over TLS (includes EHLO, optional AUTH, mail exchange)
            smtp_full_session(
                tls,
                self.user.as_deref(),
                self.password.as_deref(),
                &self.from,
                email,
            )
        }
    }

    fn send_smtps(&self, email: &Email) -> Result<(), MailerError> {
        #[cfg(not(any(feature = "http-client", feature = "http2")))]
        {
            return Err(MailerError::Smtp(
                "SMTPS (implicit TLS) requires the http-client or http2 feature".into(),
            ));
        }

        #[cfg(any(feature = "http-client", feature = "http2"))]
        {
            let stream = self.connect()?;
            let tls = smtp_tls_connect(stream, &self.host)?;
            smtp_full_session(
                tls,
                self.user.as_deref(),
                self.password.as_deref(),
                &self.from,
                email,
            )
        }
    }
}

// ── SMTP session helpers ───────────────────────────────────────────────────────

/// Wraps a `BufReader<S>` so we can alternate reads and writes without
/// reborrowing the same `BufReader` for both simultaneously.
struct SmtpConn<S: Read + Write> {
    reader: BufReader<S>,
}

impl<S: Read + Write> SmtpConn<S> {
    fn new(stream: S) -> Self {
        SmtpConn { reader: BufReader::new(stream) }
    }

    /// Read a potentially multiline SMTP response and check for `code`.
    fn expect(&mut self, code: u16) -> Result<Vec<String>, MailerError> {
        let lines = smtp_read_response(&mut self.reader)?;
        let last = lines.last().cloned().unwrap_or_default();
        let actual: u16 = last.get(..3).and_then(|s| s.parse().ok()).unwrap_or(0);
        if actual != code {
            return Err(MailerError::Smtp(format!("expected {code}, got: {last}")));
        }
        Ok(lines)
    }

    /// Send a single SMTP command followed by CRLF.
    fn cmd(&mut self, cmd: &str) -> Result<(), MailerError> {
        smtp_write(self.reader.get_mut(), cmd)
    }
}

/// Read a single SMTP response (one or more lines; last line has `NNN ` prefix).
fn smtp_read_response(r: &mut impl BufRead) -> Result<Vec<String>, MailerError> {
    let mut lines = Vec::new();
    loop {
        let mut line = String::new();
        r.read_line(&mut line)?;
        let trimmed = line.trim_end_matches('\n').trim_end_matches('\r').to_string();
        let is_last = trimmed.len() < 4 || trimmed.as_bytes().get(3).copied() != Some(b'-');
        lines.push(trimmed);
        if is_last { break; }
    }
    Ok(lines)
}

/// Write `cmd\r\n` to `w` and flush.
fn smtp_write(w: &mut impl Write, cmd: &str) -> Result<(), MailerError> {
    w.write_all(cmd.as_bytes())?;
    w.write_all(b"\r\n")?;
    w.flush()?;
    Ok(())
}

/// AUTH PLAIN — encodes `\0user\0pass` in base64 and sends in one line.
fn smtp_auth_plain<S: Read + Write>(
    conn: &mut SmtpConn<S>,
    user: &str,
    pass: &str,
) -> Result<(), MailerError> {
    let payload = format!("\0{user}\0{pass}");
    let encoded = crate::core::base64::Base64::encode(payload.as_bytes())
        .unwrap_or_default();
    conn.cmd(&format!("AUTH PLAIN {encoded}"))?;
    conn.expect(235)?;
    Ok(())
}

/// MAIL FROM → RCPT TO(s) → DATA → body → QUIT.
fn smtp_deliver<S: Read + Write>(
    conn: &mut SmtpConn<S>,
    from: &str,
    email: &Email,
) -> Result<(), MailerError> {
    conn.cmd(&format!("MAIL FROM:<{from}>"))?;
    conn.expect(250)?;

    for addr in email.to.iter().chain(email.cc.iter()).chain(email.bcc.iter()) {
        conn.cmd(&format!("RCPT TO:<{addr}>"))?;
        conn.expect(250)?;
    }

    conn.cmd("DATA")?;
    conn.expect(354)?;

    let msg = build_message(from, email);
    conn.reader.get_mut().write_all(msg.as_bytes())?;
    // SMTP DATA terminator: end the message body then signal end-of-data.
    conn.reader.get_mut().write_all(b"\r\n.\r\n")?;
    conn.reader.get_mut().flush()?;
    conn.expect(250)?;

    conn.cmd("QUIT")?;
    // Read 221 Bye — synchronizes with the server so the connection is cleanly
    // drained before the caller drops the stream.
    let _ = conn.expect(221);
    Ok(())
}

/// Full SMTP session starting from the server banner (220).
fn smtp_full_session<S: Read + Write>(
    stream: S,
    user: Option<&str>,
    pass: Option<&str>,
    from: &str,
    email: &Email,
) -> Result<(), MailerError> {
    let mut conn = SmtpConn::new(stream);
    conn.expect(220)?;
    conn.cmd(&format!("EHLO {}", smtp_local_hostname()))?;
    conn.expect(250)?;
    if let (Some(u), Some(p)) = (user, pass) {
        smtp_auth_plain(&mut conn, u, p)?;
    }
    smtp_deliver(&mut conn, from, email)?;
    Ok(())
}

fn smtp_local_hostname() -> &'static str { "localhost" }

// ── TLS upgrade (requires rustls) ─────────────────────────────────────────────

#[cfg(any(feature = "http-client", feature = "http2"))]
fn smtp_tls_connect(
    tcp: TcpStream,
    host: &str,
) -> Result<rustls::StreamOwned<rustls::ClientConnection, TcpStream>, MailerError> {
    use std::sync::Arc;
    use rustls::{pki_types::ServerName, ClientConfig, ClientConnection};

    let root_store = rustls::RootCertStore::from_iter(
        webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
    );
    let config = Arc::new(
        ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth(),
    );
    let server_name = ServerName::try_from(host.to_string())
        .map_err(|e| MailerError::Smtp(format!("invalid SMTP hostname '{host}': {e}")))?;
    let conn = ClientConnection::new(config, server_name)
        .map_err(|e| MailerError::Smtp(e.to_string()))?;
    Ok(rustls::StreamOwned::new(conn, tcp))
}

// ── RFC 5322 message builder ───────────────────────────────────────────────────

/// Build a minimal RFC 5322 message string (headers + blank line + body).
/// The SMTP DATA terminator (`\r\n.\r\n`) is NOT included here.
pub(crate) fn build_message(from: &str, email: &Email) -> String {
    let mut msg = String::new();

    msg.push_str(&format!("From: {from}\r\n"));
    msg.push_str(&format!("To: {}\r\n", email.to.join(", ")));
    if !email.cc.is_empty() {
        msg.push_str(&format!("Cc: {}\r\n", email.cc.join(", ")));
    }
    msg.push_str(&format!("Subject: {}\r\n", email.subject));
    if let Some(ref rt) = email.reply_to {
        msg.push_str(&format!("Reply-To: {rt}\r\n"));
    }
    msg.push_str("MIME-Version: 1.0\r\n");

    match (&email.text, &email.html) {
        (Some(text), None) => {
            msg.push_str("Content-Type: text/plain; charset=utf-8\r\n\r\n");
            msg.push_str(&dot_stuff(text));
        }
        (None, Some(html)) => {
            msg.push_str("Content-Type: text/html; charset=utf-8\r\n\r\n");
            msg.push_str(&dot_stuff(html));
        }
        (Some(text), Some(html)) => {
            let boundary = "----=_Part_rws_alt";
            msg.push_str(&format!(
                "Content-Type: multipart/alternative; boundary=\"{boundary}\"\r\n\r\n"
            ));
            msg.push_str(&format!("--{boundary}\r\n"));
            msg.push_str("Content-Type: text/plain; charset=utf-8\r\n\r\n");
            msg.push_str(&dot_stuff(text));
            msg.push_str(&format!("\r\n--{boundary}\r\n"));
            msg.push_str("Content-Type: text/html; charset=utf-8\r\n\r\n");
            msg.push_str(&dot_stuff(html));
            msg.push_str(&format!("\r\n--{boundary}--\r\n"));
        }
        (None, None) => {} // caught by EmailBuilder::build
    }

    msg
}

/// SMTP dot-stuffing (RFC 5321 §4.5.2): prefix any line starting with `.`
/// with an extra `.` so the DATA terminator is not triggered early.
pub(crate) fn dot_stuff(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 8);
    for line in s.split('\n') {
        let line = line.trim_end_matches('\r');
        if line.starts_with('.') {
            result.push('.');
        }
        result.push_str(line);
        result.push_str("\r\n");
    }
    // Remove the trailing \r\n added after the last line
    if result.ends_with("\r\n") {
        result.truncate(result.len() - 2);
    }
    result
}
