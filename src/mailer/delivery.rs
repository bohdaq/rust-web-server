//! Attachments, transports, delivery later, templates and previews
//! (18.35.0, #93).

use std::sync::{Arc, Mutex};

use super::{Email, Mailer, MailerError};
#[cfg(feature = "tera")]
use super::EmailBuilder;

/// A file attached to an email.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailAttachment {
    /// The name the recipient sees (one line, ASCII; others become `_`).
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
}

impl MailAttachment {
    pub fn new(filename: &str, content_type: &str, data: impl Into<Vec<u8>>) -> Self {
        MailAttachment { filename: filename.to_string(), content_type: content_type.to_string(), data: data.into() }
    }
}

/// An [`Email`] with its attachments: what a [`Deliver`] sends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mail {
    pub email: Email,
    pub attachments: Vec<MailAttachment>,
}

impl PartialEq for Email {
    fn eq(&self, other: &Self) -> bool {
        (&self.to, &self.cc, &self.bcc, &self.subject, &self.text, &self.html, &self.reply_to)
            == (&other.to, &other.cc, &other.bcc, &other.subject, &other.text, &other.html, &other.reply_to)
    }
}

impl Eq for Email {}

impl From<Email> for Mail {
    fn from(email: Email) -> Self {
        Mail { email, attachments: Vec::new() }
    }
}

impl Mail {
    pub fn new(email: Email) -> Self {
        Mail::from(email)
    }

    /// Add an attachment.
    pub fn attach(mut self, attachment: MailAttachment) -> Self {
        self.attachments.push(attachment);
        self
    }

    /// Send it on `queue`'s worker threads through `transport` (with the
    /// queue's retries), returning at once. Lost if the process exits
    /// first: see [`enqueue`](Self::enqueue) for a persistent queue.
    #[cfg(feature = "jobs")]
    pub fn deliver_later(self, queue: &crate::jobs::JobQueue, transport: Arc<dyn Deliver>) {
        queue.submit(move || transport.deliver(&self).map_err(|e| e.to_string()));
    }

    /// Put it in `queue` as a [`MAIL_JOB_TYPE`] job; a process that called
    /// [`register_delivery`] sends it. Returns the job id.
    #[cfg(all(feature = "jobs", any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql")))]
    pub async fn enqueue(&self, queue: &crate::jobs::PersistentJobQueue, options: crate::jobs::EnqueueOptions) -> Result<String, crate::model::DbError> {
        queue.enqueue_with(MAIL_JOB_TYPE, &self.to_payload(), options).await
    }

    /// The mail as job payload text: one `key=value` line per field,
    /// values percent-encoded, attachments' data in base64.
    pub fn to_payload(&self) -> String {
        let e = &self.email;
        let mut lines = Vec::new();
        let mut push = |key: &str, value: &str| lines.push(format!("{key}={}", encode(value)));
        e.to.iter().for_each(|v| push("to", v));
        e.cc.iter().for_each(|v| push("cc", v));
        e.bcc.iter().for_each(|v| push("bcc", v));
        push("subject", &e.subject);
        if let Some(v) = &e.text {
            push("text", v);
        }
        if let Some(v) = &e.html {
            push("html", v);
        }
        if let Some(v) = &e.reply_to {
            push("reply_to", v);
        }
        for a in &self.attachments {
            let data = crate::core::base64::Base64::encode(&a.data).unwrap_or_default();
            lines.push(format!("attachment={};{};{}", encode(&a.filename), encode(&a.content_type), data));
        }
        lines.join("\n")
    }

    /// Parse [`to_payload`](Self::to_payload) text.
    pub fn from_payload(text: &str) -> Result<Mail, MailerError> {
        let bad = |what: &str| MailerError::Build(format!("not a mail payload: {what}"));
        let mut email = Email { to: Vec::new(), cc: Vec::new(), bcc: Vec::new(), subject: String::new(), text: None, html: None, reply_to: None };
        let mut attachments = Vec::new();
        for line in text.lines().filter(|l| !l.is_empty()) {
            let (key, value) = line.split_once('=').ok_or_else(|| bad(line))?;
            if key == "attachment" {
                let mut fields = value.splitn(3, ';');
                let (Some(name), Some(content_type), Some(data)) = (fields.next(), fields.next(), fields.next()) else {
                    return Err(bad("attachment"));
                };
                let data = crate::core::base64::Base64::decode(data.to_string()).map_err(|_| bad("attachment data"))?;
                attachments.push(MailAttachment { filename: decode(name).ok_or_else(|| bad(name))?, content_type: decode(content_type).ok_or_else(|| bad(content_type))?, data });
                continue;
            }
            let value = decode(value).ok_or_else(|| bad(value))?;
            match key {
                "to" => email.to.push(value),
                "cc" => email.cc.push(value),
                "bcc" => email.bcc.push(value),
                "subject" => email.subject = value,
                "text" => email.text = Some(value),
                "html" => email.html = Some(value),
                "reply_to" => email.reply_to = Some(value),
                other => return Err(bad(other)),
            }
        }
        if email.to.is_empty() {
            return Err(bad("no recipient"));
        }
        Ok(Mail { email, attachments })
    }
}

fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'@' | b' ') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            out.push(u8::from_str_radix(text.get(i + 1..i + 3)?, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Something that sends mail: [`Mailer`] (SMTP), [`TestMailbox`] (keeps
/// it, for tests and development), or your own (an HTTP email API).
pub trait Deliver: Send + Sync {
    fn deliver(&self, mail: &Mail) -> Result<(), MailerError>;
}

impl Deliver for Mailer {
    fn deliver(&self, mail: &Mail) -> Result<(), MailerError> {
        self.send_mail(mail)
    }
}

/// Keeps what it's given instead of sending it: the transport for tests
/// and local development. Clones share the mailbox.
///
/// ```rust
/// use rust_web_server::mailer::{Deliver, Email, Mail, TestMailbox};
///
/// let mailbox = TestMailbox::new();
/// let email = Email::builder().to("a@example.com").subject("Hi").text("Hello").build().unwrap();
/// mailbox.deliver(&Mail::new(email)).unwrap();
/// assert_eq!("Hi", mailbox.last().unwrap().email.subject);
/// ```
#[derive(Clone, Default)]
pub struct TestMailbox {
    sent: Arc<Mutex<Vec<Mail>>>,
}

impl TestMailbox {
    pub fn new() -> Self {
        TestMailbox::default()
    }

    /// Every mail delivered so far, oldest first.
    pub fn sent(&self) -> Vec<Mail> {
        self.sent.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// The latest.
    pub fn last(&self) -> Option<Mail> {
        self.sent.lock().unwrap_or_else(|e| e.into_inner()).last().cloned()
    }

    /// The latest sent to `address` (To, Cc or Bcc).
    pub fn last_to(&self, address: &str) -> Option<Mail> {
        let sent = self.sent.lock().unwrap_or_else(|e| e.into_inner());
        sent.iter()
            .rev()
            .find(|m| m.email.to.iter().chain(&m.email.cc).chain(&m.email.bcc).any(|a| a.eq_ignore_ascii_case(address)))
            .cloned()
    }

    pub fn count(&self) -> usize {
        self.sent.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn clear(&self) {
        self.sent.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

impl Deliver for TestMailbox {
    fn deliver(&self, mail: &Mail) -> Result<(), MailerError> {
        self.sent.lock().unwrap_or_else(|e| e.into_inner()).push(mail.clone());
        Ok(())
    }
}

/// The job type of mail put in a [`PersistentJobQueue`](crate::jobs::PersistentJobQueue)
/// by [`Mail::enqueue`].
pub const MAIL_JOB_TYPE: &str = "rws.mail";

/// Send the queue's [`MAIL_JOB_TYPE`] jobs through `transport` (a failed
/// send is retried like any job).
#[cfg(all(feature = "jobs", any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql")))]
pub fn register_delivery(queue: &crate::jobs::PersistentJobQueue, transport: Arc<dyn Deliver>) {
    queue.register(MAIL_JOB_TYPE, move |payload| {
        let mail = Mail::from_payload(payload).map_err(|e| e.to_string())?;
        transport.deliver(&mail).map_err(|e| e.to_string())
    });
}

// ── Templates ────────────────────────────────────────────────────────────────

#[cfg(feature = "tera")]
impl EmailBuilder {
    /// Set the bodies from the templates `<name>.txt` (text) and
    /// `<name>.html` (HTML) of the global engine
    /// ([`template::init`](crate::template::init)), whichever exist,
    /// rendered with `ctx`. An error when neither exists or one fails.
    pub fn render(self, name: &str, ctx: &crate::template::Context) -> Result<Self, MailerError> {
        let engine = crate::template::try_global().ok_or_else(|| MailerError::Build("the template engine isn't initialized: call template::init".into()))?;
        self.render_with(&engine, name, ctx)
    }

    /// [`render`](Self::render) with a given engine.
    pub fn render_with(mut self, engine: &crate::template::TeraEngine, name: &str, ctx: &crate::template::Context) -> Result<Self, MailerError> {
        let (text, html) = (format!("{name}.txt"), format!("{name}.html"));
        let mut found = false;
        if engine.has_template(&text) {
            self = self.text(&engine.render(&text, ctx).map_err(MailerError::Build)?);
            found = true;
        }
        if engine.has_template(&html) {
            self = self.html(&engine.render(&html, ctx).map_err(MailerError::Build)?);
            found = true;
        }
        if !found {
            return Err(MailerError::Build(format!("no template {text} or {html}")));
        }
        Ok(self)
    }
}

// ── Previews ─────────────────────────────────────────────────────────────────

type Preview = Arc<dyn Fn() -> Result<Mail, MailerError> + Send + Sync>;

/// Mail previews: pages showing what each email looks like, built from
/// sample data, without sending anything. A middleware: wrap the app in it
/// **in development only** (it serves whatever the previews render).
///
/// | Path | Shows |
/// |---|---|
/// | `<prefix>` | the list of previews |
/// | `<prefix>/<name>` | headers, attachments, and the HTML body in a frame (else the text) |
/// | `<prefix>/<name>.html`, `<prefix>/<name>.txt` | the bodies alone |
///
/// ```rust
/// use rust_web_server::mailer::{Email, MailPreviews};
///
/// let previews = MailPreviews::new("/_mail").add("welcome", || {
///     Ok(Email::builder().to("ann@example.com").subject("Welcome").html("<h1>Hi Ann</h1>").build()?.into())
/// });
/// ```
#[derive(Clone)]
pub struct MailPreviews {
    prefix: String,
    previews: Arc<Vec<(String, Preview)>>,
}

impl MailPreviews {
    pub fn new(prefix: &str) -> Self {
        MailPreviews { prefix: format!("/{}", prefix.trim_matches('/')), previews: Arc::new(Vec::new()) }
    }

    /// Add the preview `name` (letters, digits, `-`, `_`), built by `build`
    /// on every view.
    pub fn add(mut self, name: &str, build: impl Fn() -> Result<Mail, MailerError> + Send + Sync + 'static) -> Self {
        Arc::make_mut(&mut self.previews).push((name.to_string(), Arc::new(build)));
        self
    }

    fn page(&self, path: &str) -> Option<crate::response::Response> {
        use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
        let rest = path.strip_prefix(&self.prefix)?;
        if rest.is_empty() || rest == "/" {
            let items: String = self
                .previews
                .iter()
                .map(|(name, _)| format!("<li><a href=\"{0}/{1}\">{1}</a></li>", escape(&self.prefix), escape(name)))
                .collect();
            return Some(html(STATUS_CODE_REASON_PHRASE.n200_ok, &format!("<h1>Mail previews</h1><ul>{items}</ul>")));
        }
        let name = rest.strip_prefix('/')?;
        let (name, part) = match name.rsplit_once('.') {
            Some((n, p)) if p == "html" || p == "txt" => (n, Some(p)),
            _ => (name, None),
        };
        let Some((_, build)) = self.previews.iter().find(|(n, _)| n == name) else {
            return Some(Response::text(STATUS_CODE_REASON_PHRASE.n404_not_found, &format!("no mail preview {name:?}")));
        };
        let mail = match build() {
            Ok(mail) => mail,
            Err(e) => return Some(Response::text(STATUS_CODE_REASON_PHRASE.n500_internal_server_error, &format!("the preview failed: {e}"))),
        };
        let e = &mail.email;
        match part {
            Some("html") => Some(match &e.html {
                Some(body) => html(STATUS_CODE_REASON_PHRASE.n200_ok, body),
                None => Response::text(STATUS_CODE_REASON_PHRASE.n404_not_found, "this email has no HTML body"),
            }),
            Some(_) => Some(match &e.text {
                Some(body) => Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, body),
                None => Response::text(STATUS_CODE_REASON_PHRASE.n404_not_found, "this email has no text body"),
            }),
            None => {
                let mut page = format!("<p><a href=\"{}\">All previews</a></p><table>", escape(&self.prefix));
                let mut row = |k: &str, v: &str| page.push_str(&format!("<tr><th>{k}</th><td>{}</td></tr>", escape(v)));
                row("Subject", &e.subject);
                row("To", &e.to.join(", "));
                if !e.cc.is_empty() {
                    row("Cc", &e.cc.join(", "));
                }
                if !e.bcc.is_empty() {
                    row("Bcc", &e.bcc.join(", "));
                }
                if let Some(reply_to) = &e.reply_to {
                    row("Reply-To", reply_to);
                }
                for a in &mail.attachments {
                    row("Attachment", &format!("{} ({}, {} bytes)", a.filename, a.content_type, a.data.len()));
                }
                page.push_str("</table>");
                if e.html.is_some() {
                    // Sandboxed: the email's HTML can't script this page.
                    page.push_str(&format!("<iframe sandbox src=\"{}/{}.html\" style=\"width:100%;height:70vh;border:1px solid #ccc\"></iframe>", escape(&self.prefix), escape(name)));
                }
                if let Some(text) = &e.text {
                    page.push_str(&format!("<h2>Text</h2><pre>{}</pre>", escape(text)));
                }
                Some(html(STATUS_CODE_REASON_PHRASE.n200_ok, &page))
            }
        }
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn html(status: &crate::response::StatusCodeReasonPhrase, body: &str) -> crate::response::Response {
    let content = crate::range::Range::get_content_range(body.as_bytes().to_vec(), crate::mime_type::MimeType::TEXT_HTML.to_string());
    crate::response::Response::get_response(status, None, Some(vec![content]))
}

impl crate::middleware::Middleware for MailPreviews {
    fn handle(&self, request: &crate::request::Request, connection: &crate::server::ConnectionInfo, next: &dyn crate::application::Application) -> Result<crate::response::Response, String> {
        if request.method == "GET" {
            let path = request.request_uri.split('?').next().unwrap_or("");
            if let Some(response) = self.page(path) {
                return Ok(response);
            }
        }
        next.execute(request, connection)
    }

    fn is_passthrough(&self, request: &crate::request::Request, _connection: &crate::server::ConnectionInfo) -> bool {
        !request.request_uri.starts_with(&self.prefix)
    }

    fn accepts_streamed_body(&self, _request: &crate::request::Request, _connection: &crate::server::ConnectionInfo) -> bool {
        true
    }

    fn layer_name(&self) -> &'static str {
        "MailPreviews"
    }
}

#[cfg(test)]
mod tests;
