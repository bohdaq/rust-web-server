//! The controller's mesh configuration endpoint: `GET /mesh/v1/snapshot`
//! answers the current [`MeshSnapshot`] as JSON with its version in
//! `X-Rws-Mesh-Version`; with `?version=<n>` equal to the current one it
//! waits (up to [`LONG_POLL`]) for the next. Every controller replica serves
//! it from its own watches.
//!
//! Stage 1 serves it without authentication: it holds routes, Service
//! addresses and endpoint addresses — what every pod can already learn from
//! DNS and traffic — and nothing secret.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::application::Application;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

use super::MeshSnapshot;

/// Where the endpoint answers.
pub const PATH: &str = "/mesh/v1/snapshot";
/// The version header.
pub const VERSION_HEADER: &str = "X-Rws-Mesh-Version";
/// The default port of the endpoint.
pub const DEFAULT_PORT: u16 = 15010;
/// How long a request for a newer version waits.
pub const LONG_POLL: Duration = Duration::from_secs(30);

/// The snapshot being served, and a signal when it changes.
#[derive(Clone)]
pub struct Publisher {
    current: Arc<Mutex<(u64, Arc<String>)>>,
    changed: tokio::sync::watch::Sender<u64>,
}

impl Default for Publisher {
    fn default() -> Self {
        Publisher { current: Arc::new(Mutex::new((0, Arc::new(String::new())))), changed: tokio::sync::watch::channel(0).0 }
    }
}

impl Publisher {
    pub fn new() -> Publisher {
        Publisher::default()
    }

    /// Serve `mesh` from now on; a snapshot equal to the current one keeps
    /// its version (no wake-up for the sidecars).
    pub fn publish(&self, mesh: &MeshSnapshot) {
        let Ok(body) = serde_json::to_string(mesh) else { return };
        let mut current = self.current.lock().unwrap_or_else(|e| e.into_inner());
        if *current.1 == body {
            return;
        }
        *current = (current.0 + 1, Arc::new(body));
        self.changed.send_replace(current.0);
    }

    /// The version and JSON being served (version 0: nothing yet).
    pub fn current(&self) -> (u64, Arc<String>) {
        self.current.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn response(&self) -> Response {
        let (version, body) = self.current();
        if version == 0 {
            return Response::text(STATUS_CODE_REASON_PHRASE.n503_service_unavailable, "no snapshot yet");
        }
        let mut response = Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, &body);
        response.headers.retain(|h| !h.name.eq_ignore_ascii_case("content-type"));
        response.headers.push(crate::header::Header::new("Content-Type", "application/json"));
        response.headers.push(crate::header::Header::new(VERSION_HEADER, version.to_string()));
        response.headers.push(crate::header::Header::new("Cache-Control", "no-store"));
        response
    }

    /// Wait until the version isn't `seen` (or [`LONG_POLL`] passes).
    async fn wait_for_change(&self, seen: u64) {
        let mut changed = self.changed.subscribe();
        if *changed.borrow_and_update() != seen {
            return;
        }
        let _ = tokio::time::timeout(LONG_POLL, changed.changed()).await;
    }
}

/// `version=<n>` of a request target.
pub fn requested_version(uri: &str) -> Option<u64> {
    let query = uri.split_once('?')?.1;
    query.split('&').find_map(|pair| pair.strip_prefix("version=")?.parse().ok())
}

fn is_snapshot_request(request: &Request) -> bool {
    request.method == "GET" && request.request_uri.split('?').next() == Some(PATH)
}

impl Application for Publisher {
    fn execute(&self, request: &Request, _connection: &ConnectionInfo) -> Result<Response, String> {
        if !is_snapshot_request(request) {
            return Ok(Response::text(STATUS_CODE_REASON_PHRASE.n404_not_found, "not found"));
        }
        Ok(self.response())
    }

    fn execute_async<'a>(&'a self, request: &'a Request, _connection: &'a ConnectionInfo) -> Option<crate::application::AsyncResponse<'a>> {
        if !is_snapshot_request(request) {
            return None;
        }
        Some(Box::pin(async move {
            if let Some(seen) = requested_version(&request.request_uri) {
                self.wait_for_change(seen).await;
            }
            Ok(self.response())
        }))
    }
}

/// Serve the endpoint on `port` (all interfaces) until the runtime stops.
pub async fn serve(publisher: Publisher, port: u16) {
    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", port)).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("[gateway-api] mesh: can't serve the configuration on port {port}: {e}");
            return;
        }
    };
    loop {
        let Ok((stream, peer)) = listener.accept().await else {
            tokio::time::sleep(Duration::from_millis(50)).await;
            continue;
        };
        tokio::spawn(crate::server::serve_plain_connection(stream, peer, publisher.clone()));
    }
}
