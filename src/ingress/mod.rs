//! Kubernetes Ingress watcher and router.
//!
//! [`KubernetesIngressWatcher`] watches the Kubernetes API for Ingress
//! resources and maintains a live route table. [`IngressRouter`] implements
//! [`Application`] and routes incoming HTTP requests to the appropriate
//! upstream service using the live rule table.
//!
//! # Prerequisites
//!
//! The watcher communicates with the Kubernetes API over plain HTTP/1.1 by
//! default. For in-cluster use, expose the API via `kubectl proxy` and point
//! the watcher at `http://localhost:8001`:
//!
//! ```text
//! kubectl proxy &
//! export RWS_K8S_API_SERVER=http://localhost:8001
//! export RWS_K8S_TOKEN=
//! export RWS_K8S_NAMESPACE=default
//! ```
//!
//! Or, with the `http-client` or `http2` feature enabled, connect directly
//! to the in-cluster API server over TLS — see
//! [`KubernetesIngressWatcher::from_service_account`].
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::ingress::{IngressRouter, KubernetesIngressWatcher};
//! use rust_web_server::server::Server;
//!
//! let watcher = KubernetesIngressWatcher::from_env().expect("K8s env not set");
//! watcher.start();
//!
//! let app = IngressRouter::new(watcher);
//! // Server::run(app);  // pass to your server
//! ```

#[cfg(test)]
mod tests;

#[cfg(any(feature = "http-client", feature = "http2"))]
mod tls;
// `pub(crate)` (not just `mod`) so `service_discovery::etcd` can reuse
// `read_chunked_lines` for its own gRPC-gateway JSON watch stream — the same
// "chunked NDJSON-shaped event stream" problem this module already solved.
pub(crate) mod watch;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use crate::application::Application;
use crate::core::New;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

/// How long a watch connection is allowed to sit idle before we treat it
/// as stalled and reconnect. Generous — legitimate watch connections can
/// be quiet for long stretches on an unchanging cluster.
const WATCH_READ_TIMEOUT: Duration = Duration::from_secs(300);
/// Backoff between watch reconnect attempts (including the very first
/// connection failing, e.g. because the API server doesn't support watch).
const WATCH_RECONNECT_BACKOFF: Duration = Duration::from_secs(2);

// ── PathType ──────────────────────────────────────────────────────────────────

/// Mirrors Kubernetes' `Ingress.spec.rules[].http.paths[].pathType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathType {
    /// Element-wise path-segment prefix match (the Kubernetes default and
    /// this crate's fallback when `pathType` is absent from older/lenient
    /// API responses).
    Prefix,
    /// The request path must equal the rule's path exactly (ignoring any
    /// query string).
    Exact,
    /// Matching is left to the Ingress controller. This router treats it
    /// the same as `Prefix`, same as most controllers do in practice.
    ImplementationSpecific,
}

impl PathType {
    fn parse(s: &str) -> Self {
        match s {
            "Exact" => PathType::Exact,
            "ImplementationSpecific" => PathType::ImplementationSpecific,
            _ => PathType::Prefix,
        }
    }
}

// ── IngressRule ───────────────────────────────────────────────────────────────

/// A single routing rule parsed from a Kubernetes Ingress resource.
#[derive(Debug, Clone, PartialEq)]
pub struct IngressRule {
    /// Value of `spec.rules[].host`.  Empty string means match-all.
    pub host: String,
    /// Value of `spec.rules[].http.paths[].path`.
    pub path: String,
    /// Value of `spec.rules[].http.paths[].pathType`. Defaults to
    /// [`PathType::Prefix`] if the field is absent.
    pub path_type: PathType,
    /// Kubernetes service name (`spec.rules[].http.paths[].backend.service.name`).
    pub service_name: String,
    /// Kubernetes service port number.
    pub service_port: u16,
    /// Namespace the Ingress lives in.
    pub namespace: String,
}

impl IngressRule {
    /// Build the upstream Kubernetes DNS address.
    ///
    /// Returns `"{service_name}.{namespace}.svc.cluster.local:{service_port}"`.
    pub fn upstream_addr(&self) -> String {
        format!(
            "{}.{}.svc.cluster.local:{}",
            self.service_name, self.namespace, self.service_port
        )
    }

    /// Returns `true` if this rule matches the given `host` header value and
    /// request `uri`.
    ///
    /// * If `self.host` is non-empty, the incoming `host` must match
    ///   (case-insensitive).
    /// * Any query string on `uri` is ignored for path matching.
    /// * [`PathType::Exact`] requires the request path to equal `self.path`
    ///   exactly.
    /// * [`PathType::Prefix`] (and [`PathType::ImplementationSpecific`])
    ///   match on whole path segments — `/foo` matches `/foo`, `/foo/`, and
    ///   `/foo/bar`, but **not** `/foobar` — not a raw byte prefix.
    pub fn matches(&self, host: &str, uri: &str) -> bool {
        if !self.host.is_empty() && !self.host.eq_ignore_ascii_case(host) {
            return false;
        }
        let request_path = uri.split('?').next().unwrap_or(uri);
        match self.path_type {
            PathType::Exact => request_path == self.path,
            PathType::Prefix | PathType::ImplementationSpecific => {
                path_prefix_matches(&self.path, request_path)
            }
        }
    }
}

/// Element-wise prefix match per Kubernetes' `pathType: Prefix` semantics.
fn path_prefix_matches(prefix: &str, request_path: &str) -> bool {
    if prefix == "/" {
        return true;
    }
    let trimmed = prefix.trim_end_matches('/');
    request_path == trimmed || request_path.starts_with(&format!("{trimmed}/"))
}

// ── JSON helpers ──────────────────────────────────────────────────────────────

/// Find the first occurrence of `"field": "VALUE"` in `json` and return `VALUE`.
fn extract_str_field<'a>(json: &'a str, field: &str) -> Option<&'a str> {
    let needle = format!("\"{}\":", field);
    let start = json.find(needle.as_str())?;
    let after_colon = &json[start + needle.len()..];
    let after_colon = after_colon.trim_start_matches(' ');
    if !after_colon.starts_with('"') {
        return None;
    }
    let inner = &after_colon[1..];
    let end = inner.find('"')?;
    Some(&inner[..end])
}

/// Find the *last* occurrence of `"field": "VALUE"` in `json` and return
/// `VALUE` — used to find the `namespace` field belonging to *this*
/// Ingress item by searching backward from its `spec`, since `metadata`
/// (and the `namespace` field within it) always precedes `spec` in a real
/// Kubernetes object's JSON encoding.
fn extract_last_str_field<'a>(json: &'a str, field: &str) -> Option<&'a str> {
    let needle = format!("\"{}\":", field);
    let start = json.rfind(needle.as_str())?;
    let after_colon = &json[start + needle.len()..];
    let after_colon = after_colon.trim_start_matches(' ');
    if !after_colon.starts_with('"') {
        return None;
    }
    let inner = &after_colon[1..];
    let end = inner.find('"')?;
    Some(&inner[..end])
}

/// Find the first occurrence of `"field": NUMBER` in `json` and parse as `u16`.
fn extract_u16_field(json: &str, field: &str) -> Option<u16> {
    let needle = format!("\"{}\":", field);
    let start = json.find(needle.as_str())?;
    let after_colon = &json[start + needle.len()..];
    let after_colon = after_colon.trim_start_matches(' ');
    let end = after_colon.find(|c: char| !c.is_ascii_digit())?;
    after_colon[..end].parse().ok()
}

// ── parse_ingress_list ────────────────────────────────────────────────────────

/// Parse a Kubernetes Ingress list JSON body into a `Vec<IngressRule>`.
///
/// This is a minimal, hand-rolled parser that handles the common formatting
/// returned by the Kubernetes API server.  It does not depend on any external
/// JSON library.
///
/// `ingress_class`, if `Some`, restricts the result to Ingress objects whose
/// `spec.ingressClassName` equals it exactly — an Ingress with no
/// `ingressClassName` at all never matches a `Some` filter. Pass `None` to
/// accept every Ingress regardless of class (the historical, and still
/// default, behavior — see [`KubernetesIngressWatcher::ingress_class`]).
pub fn parse_ingress_list(json: &str, ingress_class: Option<&str>) -> Vec<IngressRule> {
    let mut rules = Vec::new();

    // Every `"spec"` occurrence starts one Ingress item's spec object. The
    // section between this occurrence and the next one (or end of string)
    // is exactly what the old split()-based version of this parser treated
    // as "the current item" — kept as-is here, just computed via explicit
    // byte offsets instead of `str::split` so we can also look *backward*
    // from each occurrence (see `extract_last_str_field` below).
    let spec_positions: Vec<usize> = json.match_indices("\"spec\"").map(|(i, _)| i).collect();
    for (idx, &pos) in spec_positions.iter().enumerate() {
        let section_start = pos + "\"spec\"".len();
        let section_end = spec_positions.get(idx + 1).copied().unwrap_or(json.len());
        let section = &json[section_start..section_end];

        // `namespace` lives in `metadata`, which precedes `spec` for every
        // real Kubernetes object — search backward from this `spec`
        // occurrence, not forward into `section`. (A prior version of this
        // parser searched `section` itself, which never actually contains
        // `namespace` for a real API response — metadata always comes
        // first — so it silently always fell back to `"default"` unless
        // the real namespace happened to also be "default".)
        let namespace = extract_last_str_field(&json[..pos], "namespace")
            .unwrap_or("default")
            .to_string();

        if let Some(wanted) = ingress_class {
            if extract_str_field(section, "ingressClassName") != Some(wanted) {
                continue;
            }
        }

        // Within this spec section, look for rules.
        let rules_sections: Vec<&str> = section.split("\"rules\"").collect();
        for rules_section in rules_sections.iter().skip(1) {
            // Extract host (may be absent).
            let host = extract_str_field(rules_section, "host").unwrap_or("").to_string();

            // Within the rules section, split on "paths".
            let paths_sections: Vec<&str> = rules_section.split("\"paths\"").collect();
            for paths_section in paths_sections.iter().skip(1) {
                // Within each paths entry, split on path objects.
                // Each path entry looks like: {"path":"/foo","backend":...}
                // We split on `"path"` and take alternating sections.
                let path_entries: Vec<&str> = paths_section.split("\"path\"").collect();
                for path_entry in path_entries.iter().skip(1) {
                    let path = extract_str_field(path_entry, "path")
                        .or_else(|| {
                            // The split consumed "path" so the value comes right after ":"
                            let after_colon = path_entry.trim_start_matches(':').trim_start_matches(' ');
                            if after_colon.starts_with('"') {
                                let inner = &after_colon[1..];
                                inner.find('"').map(|end| &inner[..end])
                            } else {
                                None
                            }
                        })
                        .unwrap_or("/")
                        .to_string();
                    let path_type = extract_str_field(path_entry, "pathType")
                        .map(PathType::parse)
                        .unwrap_or(PathType::Prefix);

                    let service_name =
                        extract_str_field(path_entry, "name").unwrap_or("").to_string();
                    let service_port =
                        extract_u16_field(path_entry, "number").unwrap_or(80);

                    if !service_name.is_empty() {
                        rules.push(IngressRule {
                            host: host.clone(),
                            path,
                            path_type,
                            service_name,
                            service_port,
                            namespace: namespace.clone(),
                        });
                    }
                }
            }
        }
    }

    rules
}

// ── KubernetesIngressWatcher ──────────────────────────────────────────────────

/// Watches a Kubernetes API server for Ingress resources and maintains a live
/// routing table.
pub struct KubernetesIngressWatcher {
    api_server: String,
    token: String,
    namespace: String,
    ingress_class: Option<String>,
    poll_interval_secs: u64,
    rules: Arc<RwLock<Vec<IngressRule>>>,
    #[cfg(any(feature = "http-client", feature = "http2"))]
    tls: Option<Arc<tls::InClusterConfig>>,
}

impl KubernetesIngressWatcher {
    /// Create a watcher from explicit values.
    ///
    /// `api_server` should be a plain-HTTP URL such as `http://localhost:8001`.
    /// The default namespace is `"default"`.
    pub fn new(api_server: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            api_server: api_server.into(),
            token: token.into(),
            namespace: "default".to_string(),
            ingress_class: None,
            poll_interval_secs: 30,
            rules: Arc::new(RwLock::new(Vec::new())),
            #[cfg(any(feature = "http-client", feature = "http2"))]
            tls: None,
        }
    }

    /// Configure from the Kubernetes service account files mounted at
    /// `/var/run/secrets/kubernetes.io/serviceaccount/` and the
    /// `KUBERNETES_SERVICE_HOST`/`KUBERNETES_SERVICE_PORT` environment
    /// variables Kubernetes injects into every pod, connecting to the
    /// in-cluster API server directly over TLS — no `kubectl proxy`
    /// sidecar needed.
    ///
    /// Requires the `http-client` or `http2` feature (both already pull in
    /// `rustls`); trusts only the cluster's own CA certificate
    /// (`.../ca.crt`), not the public root store `crate::http_client` uses,
    /// since the API server's certificate is signed by that private CA.
    #[cfg(any(feature = "http-client", feature = "http2"))]
    pub fn from_service_account() -> Result<Self, String> {
        let cfg = tls::load()?;
        let namespace = cfg.namespace.clone();
        let mut watcher = Self::new("", "");
        watcher.namespace = namespace;
        watcher.tls = Some(Arc::new(cfg));
        Ok(watcher)
    }

    /// Attempt to configure from the Kubernetes service account files at
    /// `/var/run/secrets/kubernetes.io/serviceaccount/`.
    ///
    /// Building without the `http-client` or `http2` feature has no TLS
    /// implementation available, so this always fails — use `kubectl
    /// proxy` and configure via environment variables instead:
    ///
    /// ```text
    /// kubectl proxy &
    /// export RWS_K8S_API_SERVER=http://localhost:8001
    /// ```
    #[cfg(not(any(feature = "http-client", feature = "http2")))]
    pub fn from_service_account() -> Result<Self, String> {
        Err(
            "In-cluster TLS (https://kubernetes.default.svc) requires the `http-client` or \
             `http2` feature. Enable one of those, or use `kubectl proxy` and set \
             RWS_K8S_API_SERVER=http://localhost:8001 along with RWS_K8S_TOKEN and \
             RWS_K8S_NAMESPACE, then call KubernetesIngressWatcher::from_env()."
                .to_string(),
        )
    }

    /// Configure from environment variables `RWS_K8S_API_SERVER`, `RWS_K8S_TOKEN`,
    /// and (optionally) `RWS_K8S_NAMESPACE`.
    ///
    /// Returns `Err` if `RWS_K8S_API_SERVER` is not set.
    pub fn from_env() -> Result<Self, String> {
        let api_server = std::env::var("RWS_K8S_API_SERVER").map_err(|_| {
            "RWS_K8S_API_SERVER environment variable is not set".to_string()
        })?;
        let token = std::env::var("RWS_K8S_TOKEN").unwrap_or_default();
        let namespace = std::env::var("RWS_K8S_NAMESPACE").unwrap_or_else(|_| "default".to_string());
        let mut watcher = Self::new(api_server, token);
        watcher.namespace = namespace;
        Ok(watcher)
    }

    /// Override the namespace filter.  Use `"all"` or empty string for all namespaces.
    pub fn namespace(mut self, ns: impl Into<String>) -> Self {
        self.namespace = ns.into();
        self
    }

    /// Restrict watched Ingress objects to those whose
    /// `spec.ingressClassName` equals `class` exactly. Unset (the default)
    /// accepts every Ingress cluster- or namespace-wide regardless of
    /// class — fine for a single-controller cluster, but on a
    /// multi-controller cluster (e.g. running alongside `nginx-ingress`)
    /// leaves this watcher picking up Ingress objects meant for the other
    /// controller too, so set this explicitly in that case.
    pub fn ingress_class(mut self, class: impl Into<String>) -> Self {
        self.ingress_class = Some(class.into());
        self
    }

    /// Override the polling interval in seconds (default: 30).
    ///
    /// This remains a periodic full resync even when the watch connection
    /// (always attempted alongside it — see the [module docs](self)) is
    /// healthy, the same "trust but verify" pattern real Kubernetes
    /// controllers use: the watch stream delivers low-latency updates, and
    /// this interval is the safety net against a missed or silently
    /// swallowed event.
    pub fn poll_interval_secs(mut self, secs: u64) -> Self {
        self.poll_interval_secs = secs;
        self
    }

    /// Spawn background threads that keep the rule table up to date: a
    /// periodic full resync every `poll_interval_secs`, and (best-effort) a
    /// long-lived watch connection that triggers an immediate resync on any
    /// change instead of waiting for the next interval. Call once at
    /// startup.
    pub fn start(&self) {
        self.clone_inner().poll_loop();
        self.clone_inner().watch_loop();
    }

    fn clone_inner(&self) -> WatcherHandle {
        WatcherHandle {
            api_server: self.api_server.clone(),
            token: self.token.clone(),
            namespace: self.namespace.clone(),
            ingress_class: self.ingress_class.clone(),
            poll_interval_secs: self.poll_interval_secs,
            rules: Arc::clone(&self.rules),
            #[cfg(any(feature = "http-client", feature = "http2"))]
            tls: self.tls.clone(),
        }
    }

    /// Return a snapshot of the current rule list.
    pub fn rules(&self) -> Vec<IngressRule> {
        self.rules.read().unwrap().clone()
    }

    /// Perform one synchronous poll cycle.
    ///
    /// Exported for testing without starting the background thread.
    pub fn poll(&self) -> Result<(), String> {
        let new_rules = self.do_poll()?;
        *self.rules.write().unwrap() = new_rules;
        Ok(())
    }

    fn list_path(&self) -> String {
        list_path(&self.namespace)
    }

    fn do_poll(&self) -> Result<Vec<IngressRule>, String> {
        let path = self.list_path();
        #[cfg(any(feature = "http-client", feature = "http2"))]
        let body = fetch(&self.api_server, &self.token, self.tls.as_deref(), &path)?;
        #[cfg(not(any(feature = "http-client", feature = "http2")))]
        let body = http_get_plain(&self.api_server, &path, &self.token)?;
        Ok(parse_ingress_list(&body, self.ingress_class.as_deref()))
    }
}

fn list_path(namespace: &str) -> String {
    if namespace.is_empty() || namespace == "all" {
        "/apis/networking.k8s.io/v1/ingresses".to_string()
    } else {
        format!("/apis/networking.k8s.io/v1/namespaces/{namespace}/ingresses")
    }
}

/// Fetch the ingress list body, over TLS if `tls_cfg` is configured,
/// falling back to plain HTTP otherwise. Shared by
/// [`KubernetesIngressWatcher::do_poll`] and [`WatcherHandle::poll_once`].
#[cfg(any(feature = "http-client", feature = "http2"))]
fn fetch(
    api_server: &str,
    token: &str,
    tls_cfg: Option<&tls::InClusterConfig>,
    path: &str,
) -> Result<String, String> {
    if let Some(cfg) = tls_cfg {
        return tls::https_get(
            &cfg.host,
            cfg.port,
            "kubernetes.default.svc",
            cfg.client_config.clone(),
            &cfg.token,
            path,
            Duration::from_secs(10),
        );
    }
    http_get_plain(api_server, path, token)
}

// Internal handle used by the background threads (avoids having to make
// KubernetesIngressWatcher Clone while sharing the rules Arc).
struct WatcherHandle {
    api_server: String,
    token: String,
    namespace: String,
    ingress_class: Option<String>,
    poll_interval_secs: u64,
    rules: Arc<RwLock<Vec<IngressRule>>>,
    #[cfg(any(feature = "http-client", feature = "http2"))]
    tls: Option<Arc<tls::InClusterConfig>>,
}

impl WatcherHandle {
    fn poll_loop(self) {
        // Do an initial poll before sleeping.
        self.poll_once();
        let interval = Duration::from_secs(self.poll_interval_secs);
        std::thread::spawn(move || loop {
            std::thread::sleep(interval);
            self.poll_once();
        });
    }

    fn poll_once(&self) {
        let path = list_path(&self.namespace);
        let result = {
            #[cfg(any(feature = "http-client", feature = "http2"))]
            {
                fetch(&self.api_server, &self.token, self.tls.as_deref(), &path)
            }
            #[cfg(not(any(feature = "http-client", feature = "http2")))]
            {
                http_get_plain(&self.api_server, &path, &self.token)
            }
        };
        match result {
            Ok(body) => {
                let new_rules = parse_ingress_list(&body, self.ingress_class.as_deref());
                *self.rules.write().unwrap() = new_rules;
            }
            Err(e) => {
                eprintln!("ingress watcher: poll failed: {}", e);
            }
        }
    }

    /// Spawn the watch-connection thread — see the [module docs](self) (and
    /// `watch`'s own docs) for why this triggers a full re-list on any event
    /// rather than applying deltas incrementally.
    fn watch_loop(self) {
        std::thread::spawn(move || loop {
            if let Err(e) = self.watch_once() {
                eprintln!("ingress watcher: watch connection error: {e}");
            }
            std::thread::sleep(WATCH_RECONNECT_BACKOFF);
        });
    }

    fn watch_once(&self) -> Result<(), String> {
        let path = format!("{}?watch=true", list_path(&self.namespace));

        #[cfg(any(feature = "http-client", feature = "http2"))]
        if let Some(cfg) = &self.tls {
            let stream = tls::tls_connect(
                &cfg.host,
                cfg.port,
                "kubernetes.default.svc",
                cfg.client_config.clone(),
                WATCH_READ_TIMEOUT,
            )?;
            return self.stream_watch(stream, "kubernetes.default.svc", &cfg.token, &path);
        }

        let (host, addr) = parse_plain_host_addr(&self.api_server)?;
        let tcp = TcpStream::connect(&addr)
            .map_err(|e| format!("ingress watcher: connect to {addr} failed: {e}"))?;
        tcp.set_read_timeout(Some(WATCH_READ_TIMEOUT)).map_err(|e| e.to_string())?;
        tcp.set_write_timeout(Some(Duration::from_secs(5))).map_err(|e| e.to_string())?;
        self.stream_watch(tcp, &host, &self.token, &path)
    }

    fn stream_watch<S: Read + Write>(
        &self,
        mut stream: S,
        host_header: &str,
        token: &str,
        path: &str,
    ) -> Result<(), String> {
        let auth_header = if token.is_empty() {
            String::new()
        } else {
            format!("Authorization: Bearer {token}\r\n")
        };
        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: {host_header}\r\n{auth_header}Accept: application/json\r\nConnection: keep-alive\r\n\r\n"
        );
        stream
            .write_all(request.as_bytes())
            .map_err(|e| format!("ingress watcher: watch request write failed: {e}"))?;

        watch::read_chunked_lines(stream, |_line| {
            // Every event (ADDED/MODIFIED/DELETED/ERROR) is treated
            // identically: it means *something* changed, so re-list from
            // scratch rather than trying to apply it as a delta — see the
            // module docs for why.
            self.poll_once();
        })
    }
}

/// Parse `host:port` out of a plain-HTTP `api_server` URL, returning both
/// the bare host (for the `Host:` header) and the `host:port` dial address.
fn parse_plain_host_addr(api_server: &str) -> Result<(String, String), String> {
    let rest = api_server
        .strip_prefix("http://")
        .ok_or_else(|| format!("ingress watcher: api_server must start with http://, got: {api_server}"))?;
    let host_port = rest.split('/').next().unwrap_or(rest);
    let (host, port) = if let Some(colon) = host_port.rfind(':') {
        let port_str = &host_port[colon + 1..];
        if let Ok(p) = port_str.parse::<u16>() {
            (&host_port[..colon], p)
        } else {
            (host_port, 80u16)
        }
    } else {
        (host_port, 80u16)
    };
    Ok((host.to_string(), format!("{host}:{port}")))
}

// ── plain-HTTP/1.1 GET helper ─────────────────────────────────────────────────

/// Issue a plain-HTTP/1.1 GET to `{api_server}{path}` with an optional Bearer
/// token and return the response body as a string.
fn http_get_plain(api_server: &str, path: &str, token: &str) -> Result<String, String> {
    let (host, addr) = parse_plain_host_addr(api_server)?;
    let mut stream = TcpStream::connect(&addr)
        .map_err(|e| format!("ingress watcher: connect to {} failed: {}", addr, e))?;
    stream.set_read_timeout(Some(Duration::from_secs(10))).map_err(|e| e.to_string())?;
    stream.set_write_timeout(Some(Duration::from_secs(5))).map_err(|e| e.to_string())?;

    let auth_header = if token.is_empty() {
        String::new()
    } else {
        format!("Authorization: Bearer {}\r\n", token)
    };

    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\n{}Accept: application/json\r\nConnection: close\r\n\r\n",
        path, host, auth_header
    );

    stream.write_all(request.as_bytes()).map_err(|e| e.to_string())?;

    let mut buf = Vec::with_capacity(8192);
    let mut tmp = [0u8; 4096];
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(e) => return Err(format!("ingress watcher: read failed: {}", e)),
        }
    }

    parse_http1_response(&buf)
}

/// Split a complete HTTP/1.1 response into its status code and body,
/// returning the body as a string if the status is 2xx. Shared by the
/// plain-HTTP path above and the TLS path in `tls::https_get`.
fn parse_http1_response(buf: &[u8]) -> Result<String, String> {
    let header_end = buf
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| "ingress watcher: incomplete HTTP response (no header end)".to_string())?;

    let header_str = std::str::from_utf8(&buf[..header_end]).unwrap_or("");
    let status_line = header_str.lines().next().unwrap_or("");
    let parts: Vec<&str> = status_line.splitn(3, ' ').collect();
    if parts.len() < 2 {
        return Err(format!("ingress watcher: malformed status line: {}", status_line));
    }
    let status: u16 = parts[1].parse().unwrap_or(0);
    if status < 200 || status >= 300 {
        return Err(format!("ingress watcher: API returned status {}", status));
    }

    let body_bytes = &buf[header_end + 4..];
    std::str::from_utf8(body_bytes)
        .map(|s| s.to_string())
        .map_err(|e| format!("ingress watcher: non-UTF-8 response body: {}", e))
}

// ── IngressRouter ─────────────────────────────────────────────────────────────

/// An [`Application`] that routes incoming requests using the live Ingress rule table.
///
/// Finds the first matching [`IngressRule`] and forwards the request to
/// `{service_name}.{namespace}.svc.cluster.local:{service_port}` over HTTP/1.1.
/// Returns `404 Not Found` when no rule matches.
pub struct IngressRouter {
    watcher: KubernetesIngressWatcher,
    connect_timeout: Duration,
    read_timeout: Duration,
}

impl IngressRouter {
    /// Wrap a watcher in an `IngressRouter` with default timeouts.
    pub fn new(watcher: KubernetesIngressWatcher) -> Self {
        Self {
            watcher,
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(30),
        }
    }

    /// Override the TCP connect timeout (default: 5 000 ms).
    pub fn connect_timeout_ms(mut self, ms: u64) -> Self {
        self.connect_timeout = Duration::from_millis(ms);
        self
    }

    /// Override the response read timeout (default: 30 000 ms).
    pub fn read_timeout_ms(mut self, ms: u64) -> Self {
        self.read_timeout = Duration::from_millis(ms);
        self
    }
}

impl Application for IngressRouter {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        let host = request
            .get_header("host".to_string())
            .map(|h| h.value.as_str())
            .unwrap_or("");

        let rules = self.watcher.rules();
        let matched = rules.iter().find(|r| r.matches(host, &request.request_uri));

        match matched {
            Some(rule) => {
                let upstream_host = format!(
                    "{}.{}.svc.cluster.local",
                    rule.service_name, rule.namespace
                );
                crate::proxy::proxy_http1(
                    request,
                    &connection.client.ip,
                    &upstream_host,
                    rule.service_port,
                    self.connect_timeout,
                    self.read_timeout,
                )
                .or_else(|_| Ok(bad_gateway()))
            }
            None => Ok(not_found()),
        }
    }
}

fn bad_gateway() -> Response {
    let cr = Range::get_content_range(
        b"502 Bad Gateway".to_vec(),
        MimeType::TEXT_PLAIN.to_string(),
    );
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n502_bad_gateway.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n502_bad_gateway.reason_phrase.to_string();
    r.content_range_list = vec![cr];
    r
}

fn not_found() -> Response {
    let cr = Range::get_content_range(
        b"404 No matching ingress rule".to_vec(),
        MimeType::TEXT_PLAIN.to_string(),
    );
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n404_not_found.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n404_not_found.reason_phrase.to_string();
    r.content_range_list = vec![cr];
    r
}
