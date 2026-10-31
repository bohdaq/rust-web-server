//! Push notifications: webhooks a client registers for a task, called with
//! each update (a `StreamResponse`, as streams carry), from one delivery
//! thread per server with a bounded queue and retries.

use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::time::Duration;

use super::model::{quote, Obj};
use crate::service_discovery::json_lite::JsonValue;

/// Webhooks per task.
pub(crate) const MAX_CONFIGS: usize = 10;
/// Deliveries waiting; past this, updates are dropped (and logged).
const QUEUE: usize = 1024;
/// Attempts per delivery, with these pauses between them.
const BACKOFF: [Duration; 2] = [Duration::from_millis(500), Duration::from_secs(2)];
const TIMEOUT_MS: u64 = 10_000;

/// A registered webhook.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PushConfig {
    pub(crate) id: String,
    pub(crate) task_id: String,
    pub(crate) url: String,
    pub(crate) token: Option<String>,
    /// `(scheme, credentials)` for `Authorization`.
    pub(crate) authentication: Option<(String, String)>,
}

impl PushConfig {
    /// From a `TaskPushNotificationConfig` object; `task_id` when the object
    /// has none (a `SendMessage` configuration).
    pub(crate) fn from_json(value: &JsonValue, task_id: Option<&str>) -> Result<PushConfig, String> {
        let s = |key: &str| super::model::field(value, key).and_then(JsonValue::as_str).filter(|v| !v.is_empty()).map(str::to_string);
        let url = s("url").ok_or("url is required")?;
        let task_id = s("taskId").or_else(|| task_id.map(str::to_string)).ok_or("taskId is required")?;
        let authentication = match value.get("authentication") {
            Some(auth) => {
                let scheme = auth.get("scheme").and_then(JsonValue::as_str).filter(|v| !v.is_empty()).ok_or("authentication.scheme is required")?;
                if !scheme.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
                    return Err("authentication.scheme isn't a scheme name".into());
                }
                let credentials = auth.get("credentials").and_then(JsonValue::as_str).unwrap_or("");
                if credentials.bytes().any(|b| b < 0x20 || b == 0x7f) {
                    return Err("authentication.credentials can't hold control characters".into());
                }
                Some((scheme.to_string(), credentials.to_string()))
            }
            None => None,
        };
        let token = s("token");
        if token.as_deref().is_some_and(|t| t.bytes().any(|b| b < 0x20 || b == 0x7f)) {
            return Err("token can't hold control characters".into());
        }
        Ok(PushConfig { id: s("id").unwrap_or_else(super::new_id), task_id, url, token, authentication })
    }

    /// As returned to the client: no credentials.
    pub(crate) fn to_json(&self) -> String {
        let auth = self.authentication.as_ref().map(|(scheme, _)| format!("{{\"scheme\":{}}}", quote(scheme)));
        Obj::new()
            .str("id", &self.id)
            .str("taskId", &self.task_id)
            .str("url", &self.url)
            .opt_str("token", self.token.as_deref())
            .opt_raw("authentication", auth.as_deref())
            .done()
    }
}

/// Whether `url` may be called: it starts with one of the allowed prefixes
/// (scheme and host at least) and is HTTP(S). Webhook URLs come from
/// clients, so the operator lists where they may point (SSRF).
pub(crate) fn allowed(url: &str, prefixes: &[String]) -> bool {
    (url.starts_with("https://") || url.starts_with("http://"))
        && !url.bytes().any(|b| b <= b' ' || b == 0x7f)
        && prefixes.iter().any(|p| url.starts_with(p.as_str()))
}

pub(crate) struct Delivery {
    pub(crate) config: PushConfig,
    pub(crate) body: String,
}

/// The queue to the delivery thread, which starts with it.
pub(crate) fn start() -> SyncSender<Delivery> {
    let (tx, rx) = sync_channel(QUEUE);
    let _ = std::thread::Builder::new().name("rws-a2a-push".into()).spawn(move || deliver_all(rx));
    tx
}

/// Queue a delivery; full queue: dropped and logged.
pub(crate) fn enqueue(tx: &SyncSender<Delivery>, config: &PushConfig, kind: &str, json: &str) {
    let delivery = Delivery { config: config.clone(), body: format!(r#"{{"{kind}":{json}}}"#) };
    if let Err(TrySendError::Full(d)) = tx.try_send(delivery) {
        eprintln!("a2a: push queue full, dropped an update for task {}", d.config.task_id);
    }
}

fn deliver_all(rx: Receiver<Delivery>) {
    // No redirects: an allowed webhook mustn't bounce the call elsewhere.
    let client = crate::http_client::Client::new().max_redirects(0);
    for delivery in rx {
        for attempt in 0..=BACKOFF.len() {
            match send(&client, &delivery) {
                Ok(()) => break,
                Err(e) if attempt == BACKOFF.len() => eprintln!("a2a: push to {} for task {} failed: {e}", delivery.config.url, delivery.config.task_id),
                Err(_) => std::thread::sleep(BACKOFF[attempt]),
            }
        }
    }
}

fn send(client: &crate::http_client::Client, delivery: &Delivery) -> Result<(), String> {
    let mut request = client.post(&delivery.config.url).header("Content-Type", "application/a2a+json").timeout_ms(TIMEOUT_MS);
    if let Some((scheme, credentials)) = &delivery.config.authentication {
        request = request.header("Authorization", &format!("{scheme} {credentials}"));
    }
    if let Some(token) = &delivery.config.token {
        request = request.header("X-A2A-Notification-Token", token);
    }
    let response = request.body(delivery.body.clone().into_bytes()).send().map_err(|e| e.to_string())?;
    if response.is_success() {
        Ok(())
    } else {
        Err(format!("HTTP {}", response.status()))
    }
}
