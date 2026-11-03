//! Web application firewall (18.22.0, #77): rws's own rules, no rule
//! language to learn and no external engine.
//!
//! Every request's path, query arguments, form / JSON / multipart field
//! values (the first [`WafConfig::inspect_body_bytes`]), cookies and a few
//! headers are decoded ([`decode::normalize`]) and checked against the
//! built-in detections ([`detect::RULES`]: SQL injection, XSS, path
//! traversal and sensitive files, file-inclusion wrappers, command
//! injection, cloud-metadata SSRF, Log4Shell, scanners, argument limits).
//! Each match adds its score; at [`WafConfig::threshold`] (5: one critical
//! match) the request is refused with `403` problem+json naming the rules —
//! or, in `detect` mode, only logged. `paranoia` (1–3) turns on stricter,
//! noisier rules.
//!
//! False positives are excluded by rule id and path prefix
//! (`exclude = ["942100@/api/search"]`), and custom rules match a field
//! with `contains` / `prefix` / `equals` / `cidr` (linear-time matching,
//! no regular expressions) to deny, allow, log or score.
//!
//! Config: `[middleware.waf]` (every route) or `[route.middleware.waf]`;
//! code: [`WafLayer`].

pub mod decode;
pub mod detect;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::application::Application;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

/// Block, only log, or do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Mode {
    #[default]
    Block,
    Detect,
    Off,
}

impl Mode {
    pub fn parse(s: &str) -> Option<Mode> {
        match s.trim().to_ascii_lowercase().as_str() {
            "block" | "" => Some(Mode::Block),
            "detect" | "log" => Some(Mode::Detect),
            "off" => Some(Mode::Off),
            _ => None,
        }
    }
}

/// Which part of the request a custom rule looks at.
#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    Path,
    /// The raw query string.
    Query,
    /// One argument (query or form) by name.
    Arg(String),
    /// Every argument value.
    Args,
    Header(String),
    Method,
    /// The client address (`cidr` matching).
    Ip,
    /// The body as text (the inspected part).
    Body,
}

impl Field {
    pub fn parse(s: &str) -> Option<Field> {
        let s = s.trim();
        let lower = s.to_ascii_lowercase();
        Some(match lower.as_str() {
            "path" => Field::Path,
            "query" => Field::Query,
            "args" => Field::Args,
            "method" => Field::Method,
            "ip" => Field::Ip,
            "body" => Field::Body,
            _ if lower.starts_with("arg:") => Field::Arg(s[4..].to_string()),
            _ if lower.starts_with("header:") => Field::Header(s[7..].to_ascii_lowercase()),
            _ => return None,
        })
    }
}

/// What a matching custom rule does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    /// Refuse the request (whatever the mode).
    Deny,
    /// Skip the WAF for the request (a false-positive escape hatch).
    Allow,
    /// Log only.
    Log,
    /// Add `score` to the anomaly score.
    Score(u32),
}

/// A custom rule.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomRule {
    pub id: u32,
    pub field: Field,
    /// Case-insensitive substring / prefix / whole value; or for `ip`, CIDR
    /// ranges. Any one matching is a match.
    pub contains: Vec<String>,
    pub prefix: Vec<String>,
    pub equals: Vec<String>,
    pub cidr: Vec<String>,
    pub action: Action,
}

/// An exclusion: rule ids (`942100`, `942*`, `*`) off for a path prefix.
#[derive(Debug, Clone, PartialEq)]
pub struct Exclusion {
    pub rule: String,
    pub path_prefix: String,
}

impl Exclusion {
    /// `942100`, `942*@/api/search`, `*@/webhooks/`.
    pub fn parse(s: &str) -> Exclusion {
        let (rule, path) = s.trim().split_once('@').unwrap_or((s.trim(), ""));
        Exclusion { rule: rule.trim().to_string(), path_prefix: path.trim().to_string() }
    }

    fn covers(&self, id: u32, path: &str) -> bool {
        let rule_ok = self.rule == "*" || self.rule == id.to_string() || self.rule.strip_suffix('*').is_some_and(|p| id.to_string().starts_with(p));
        rule_ok && path.starts_with(&self.path_prefix)
    }
}

/// WAF settings.
#[derive(Debug, Clone, PartialEq)]
pub struct WafConfig {
    pub mode: Mode,
    /// 1 (default): rules with very few false positives; 2 and 3 add
    /// stricter ones.
    pub paranoia: u8,
    /// Anomaly score at which a request is refused (default 5).
    pub threshold: u32,
    /// How much of a body is inspected (default 64 KiB; 0: bodies aren't).
    pub inspect_body_bytes: usize,
    /// More arguments than this is a match of 920100 (default 255).
    pub max_args: usize,
    pub exclude: Vec<Exclusion>,
    pub rules: Vec<CustomRule>,
}

impl Default for WafConfig {
    fn default() -> Self {
        WafConfig { mode: Mode::Block, paranoia: 1, threshold: 5, inspect_body_bytes: 64 * 1024, max_args: 255, exclude: Vec::new(), rules: Vec::new() }
    }
}

/// One match.
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub rule: u32,
    pub score: u32,
    /// Where: `path`, `arg:q`, `body:email`, `header:user-agent`, …
    pub location: String,
}

/// The outcome of [`Waf::inspect`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Verdict {
    pub score: u32,
    pub matches: Vec<Match>,
    /// A custom `deny` rule matched.
    pub denied: bool,
    /// A custom `allow` rule matched: nothing else counts.
    pub allowed: bool,
}

impl Verdict {
    pub fn rule_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.matches.iter().map(|m| m.rule).collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}

/// The engine: config plus [`inspect`](Self::inspect).
#[derive(Debug, Clone)]
pub struct Waf {
    config: WafConfig,
}

/// Name/value pairs of a request's parts.
fn form_pairs(text: &str) -> Vec<(String, String)> {
    text.split('&')
        .filter(|p| !p.is_empty())
        .take(4096)
        .map(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            let dec = |s: &str| crate::url::URL::percent_decode(&s.replace('+', " "));
            (dec(k), dec(v))
        })
        .collect()
}

/// The string tokens of a JSON document (keys and values), bounded.
fn json_strings(text: &str) -> Vec<String> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() && out.len() < 4096 {
        if b[i] == b'"' {
            let mut s = Vec::new();
            i += 1;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' && i + 1 < b.len() {
                    match b[i + 1] {
                        b'n' => s.push(b'\n'),
                        b't' => s.push(b'\t'),
                        b'u' if i + 5 < b.len() => {
                            if let Some(ch) = std::str::from_utf8(&b[i + 2..i + 6]).ok().and_then(|h| u32::from_str_radix(h, 16).ok()).and_then(char::from_u32) {
                                let mut buf = [0u8; 4];
                                s.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                            }
                            i += 4;
                        }
                        other => s.push(other),
                    }
                    i += 2;
                } else {
                    s.push(b[i]);
                    i += 1;
                }
            }
            out.push(String::from_utf8_lossy(&s).into_owned());
        }
        i += 1;
    }
    out
}

/// The non-file fields of a multipart body: (name, value).
fn multipart_fields(body: &[u8], content_type: &str) -> Vec<(String, String)> {
    // The boundary is case-sensitive; the parameter name isn't.
    let Some(boundary) = content_type.split(';').find_map(|p| {
        let p = p.trim();
        p.get(..9).filter(|k| k.eq_ignore_ascii_case("boundary=")).map(|_| p[9..].trim_matches('"').to_string())
    }) else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(body);
    let delimiter = format!("--{boundary}");
    let mut out = Vec::new();
    for part in text.split(delimiter.as_str()).skip(1).take(256) {
        let Some((head, value)) = part.split_once("\r\n\r\n") else { continue };
        let head_lower = head.to_ascii_lowercase();
        let name = head_lower.split("name=\"").nth(1).and_then(|n| n.split('"').next()).unwrap_or("").to_string();
        if head_lower.contains("filename=") {
            // Uploaded files: only their names.
            if let Some(file) = head.split("filename=\"").nth(1).and_then(|n| n.split('"').next()) {
                out.push((format!("{name}.filename"), file.to_string()));
            }
            continue;
        }
        out.push((name, value.trim_end_matches("\r\n").trim_end_matches("--").to_string()));
    }
    out
}

impl Waf {
    pub fn new(config: WafConfig) -> Waf {
        Waf { config }
    }

    pub fn config(&self) -> &WafConfig {
        &self.config
    }

    fn enabled(&self, id: u32, path: &str) -> Option<&'static detect::Rule> {
        let rule = detect::rule(id)?;
        (rule.paranoia <= self.config.paranoia && !self.config.exclude.iter().any(|e| e.covers(id, path))).then_some(rule)
    }

    /// Check `request`.
    pub fn inspect(&self, request: &Request, connection: &ConnectionInfo) -> Verdict {
        let mut verdict = Verdict::default();
        if self.config.mode == Mode::Off {
            return verdict;
        }
        let target = &request.request_uri;
        let (raw_path, query) = target.split_once('?').unwrap_or((target, ""));
        let path = crate::url::URL::percent_decode(raw_path);
        let header = |name: &str| request.header(name).map(|h| h.value.to_string()).unwrap_or_default();
        let raw_content_type = header("Content-Type");
        let content_type = raw_content_type.to_ascii_lowercase();

        let mut args = form_pairs(query);
        let body = if self.config.inspect_body_bytes > 0 { &request.body[..request.body.len().min(self.config.inspect_body_bytes)] } else { &[][..] };
        let mut body_values: Vec<(String, String)> = Vec::new();
        if !body.is_empty() {
            if content_type.starts_with("application/x-www-form-urlencoded") {
                args.extend(form_pairs(&String::from_utf8_lossy(body)));
            } else if content_type.contains("json") {
                body_values.extend(json_strings(&String::from_utf8_lossy(body)).into_iter().map(|s| ("json".to_string(), s)));
            } else if content_type.starts_with("multipart/form-data") {
                args.extend(multipart_fields(body, &raw_content_type));
            } else if content_type.starts_with("text/") || content_type.contains("xml") {
                body_values.push(("body".to_string(), String::from_utf8_lossy(body).into_owned()));
            }
        }

        // Custom rules first: deny / allow decide on their own.
        let client = crate::ip_filter::client_ip(request, &connection.client.ip);
        let ip = client.as_ref();
        for rule in &self.config.rules {
            if self.custom_matches(rule, request, &path, query, &args, body, ip) {
                match rule.action {
                    Action::Deny => verdict.denied = true,
                    Action::Allow => {
                        verdict.allowed = true;
                        return verdict;
                    }
                    Action::Log => verdict.matches.push(Match { rule: rule.id, score: 0, location: "custom".into() }),
                    Action::Score(n) => {
                        verdict.score += n;
                        verdict.matches.push(Match { rule: rule.id, score: n, location: "custom".into() });
                    }
                }
            }
        }

        let add = |verdict: &mut Verdict, hits: &[u32], location: &str| {
            let mut seen: Vec<u32> = Vec::new();
            for id in hits {
                if seen.contains(id) {
                    continue;
                }
                seen.push(*id);
                if let Some(rule) = self.enabled(*id, &path) {
                    verdict.score += rule.score;
                    verdict.matches.push(Match { rule: *id, score: rule.score, location: location.to_string() });
                }
            }
        };
        let mut hits = Vec::new();
        // The path: traversal, files, wrappers; and markup or SQL in it.
        detect::value(&decode::normalize(&path), &mut hits);
        add(&mut verdict, &hits, "path");
        if args.len() > self.config.max_args {
            add(&mut verdict, &[920100], "args");
        }
        for (name, value) in &args {
            hits.clear();
            detect::value(&decode::normalize(value), &mut hits);
            if name.len() > 1 {
                detect::value(&decode::normalize(name), &mut hits);
            }
            if value.len() > 8192 {
                hits.push(920200);
            }
            add(&mut verdict, &hits, &format!("arg:{}", truncate(name, 64)));
        }
        for (name, value) in &body_values {
            hits.clear();
            detect::value(&decode::normalize(value), &mut hits);
            add(&mut verdict, &hits, name);
        }
        // Cookies: their values.
        for cookie in request.headers.iter().filter(|h| h.name.eq_ignore_ascii_case("cookie")) {
            for pair in cookie.value.split(';').take(64) {
                let (name, value) = pair.split_once('=').unwrap_or(("", pair));
                hits.clear();
                detect::value(&decode::normalize(value), &mut hits);
                add(&mut verdict, &hits, &format!("cookie:{}", truncate(name.trim(), 64)));
            }
        }
        // Headers: scanners and Log4Shell everywhere; SQL/XSS in Referer and
        // User-Agent from paranoia 2.
        let user_agent = decode::normalize(&header("User-Agent"));
        if detect::scanner(&user_agent) {
            add(&mut verdict, &[913100], "header:user-agent");
        }
        for h in request.headers.iter().take(128) {
            if h.value.contains("${") && detect::log4shell(&decode::normalize(&h.value)) {
                add(&mut verdict, &[944100], &format!("header:{}", h.name.to_ascii_lowercase()));
            }
        }
        if self.config.paranoia >= 2 {
            for name in ["Referer", "User-Agent"] {
                hits.clear();
                detect::value(&decode::normalize(&header(name)), &mut hits);
                add(&mut verdict, &hits, &format!("header:{}", name.to_ascii_lowercase()));
            }
        }
        verdict
    }

    #[allow(clippy::too_many_arguments)]
    fn custom_matches(&self, rule: &CustomRule, request: &Request, path: &str, query: &str, args: &[(String, String)], body: &[u8], ip: &str) -> bool {
        if rule.field == Field::Ip {
            return rule.cidr.iter().any(|c| crate::ip_filter::matches(c, ip));
        }
        let values: Vec<String> = match &rule.field {
            Field::Path => vec![path.to_string()],
            Field::Query => vec![query.to_string()],
            Field::Method => vec![request.method.clone()],
            Field::Arg(name) => args.iter().filter(|(k, _)| k == name).map(|(_, v)| v.clone()).collect(),
            Field::Args => args.iter().map(|(_, v)| v.clone()).collect(),
            Field::Header(name) => request.headers.iter().filter(|h| h.name.eq_ignore_ascii_case(name)).map(|h| h.value.to_string()).collect(),
            Field::Body => vec![String::from_utf8_lossy(body).into_owned()],
            Field::Ip => Vec::new(),
        };
        values.iter().any(|v| {
            let v = v.to_ascii_lowercase();
            rule.contains.iter().any(|c| v.contains(&c.to_ascii_lowercase()))
                || rule.prefix.iter().any(|p| v.starts_with(&p.to_ascii_lowercase()))
                || rule.equals.iter().any(|e| v == e.to_ascii_lowercase())
        })
    }
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

// ── Metrics ──────────────────────────────────────────────────────────────────

struct Counters {
    blocked: AtomicU64,
    detected: AtomicU64,
    rules: Mutex<BTreeMap<u32, u64>>,
}

fn counters() -> &'static Counters {
    static C: OnceLock<Counters> = OnceLock::new();
    C.get_or_init(|| {
        crate::metrics::register_section(crate::metrics::section::WAF, prometheus_text);
        Counters { blocked: AtomicU64::new(0), detected: AtomicU64::new(0), rules: Mutex::new(BTreeMap::new()) }
    })
}

/// `rws_waf_requests_total{action}` and `rws_waf_rule_matches_total{rule}`
/// (appended to `/metrics`; empty until the WAF has seen a match).
pub fn prometheus_text() -> String {
    let c = counters();
    let rules = c.rules.lock().unwrap_or_else(|e| e.into_inner());
    if rules.is_empty() && c.blocked.load(Ordering::Relaxed) == 0 && c.detected.load(Ordering::Relaxed) == 0 {
        return String::new();
    }
    let mut out = String::from("# HELP rws_waf_requests_total Requests the web application firewall matched, by what it did.\n# TYPE rws_waf_requests_total counter\n");
    out.push_str(&format!("rws_waf_requests_total{{action=\"blocked\"}} {}\nrws_waf_requests_total{{action=\"detected\"}} {}\n", c.blocked.load(Ordering::Relaxed), c.detected.load(Ordering::Relaxed)));
    out.push_str("# HELP rws_waf_rule_matches_total Matches per WAF rule.\n# TYPE rws_waf_rule_matches_total counter\n");
    for (rule, n) in rules.iter() {
        out.push_str(&format!("rws_waf_rule_matches_total{{rule=\"{rule}\"}} {n}\n"));
    }
    out
}

// ── The middleware ───────────────────────────────────────────────────────────

/// The WAF in front of an application. See the [module docs](self).
pub struct WafLayer {
    waf: Waf,
}

impl WafLayer {
    pub fn new(config: WafConfig) -> WafLayer {
        WafLayer { waf: Waf::new(config) }
    }
}

fn refused(verdict: &Verdict) -> Response {
    let ids: Vec<String> = verdict.rule_ids().iter().map(u32::to_string).collect();
    let detail = if verdict.denied && ids.is_empty() { "refused by a firewall rule".to_string() } else { format!("refused by the web application firewall (rules {})", ids.join(", ")) };
    let mut response = crate::error::problem(STATUS_CODE_REASON_PHRASE.n403_forbidden, &detail);
    response.headers.push(crate::header::Header::new("Cache-Control", "no-store"));
    response
}

impl Middleware for WafLayer {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        let verdict = self.waf.inspect(request, connection);
        if verdict.allowed || (verdict.matches.is_empty() && !verdict.denied) {
            return next.execute(request, connection);
        }
        let c = counters();
        {
            let mut rules = c.rules.lock().unwrap_or_else(|e| e.into_inner());
            for id in verdict.rule_ids() {
                *rules.entry(id).or_insert(0) += 1;
            }
        }
        let block = verdict.denied || (self.waf.config.mode == Mode::Block && verdict.score >= self.waf.config.threshold);
        let locations: Vec<String> = verdict.matches.iter().map(|m| format!("{}@{}", m.rule, m.location)).collect();
        eprintln!(
            "[waf] {} {} {} {} score={} {}",
            if block { "blocked" } else { "detected" },
            crate::ip_filter::client_ip(request, &connection.client.ip),
            request.method,
            truncate(&request.request_uri, 200),
            verdict.score,
            locations.join(" ")
        );
        if block {
            c.blocked.fetch_add(1, Ordering::Relaxed);
            return Ok(refused(&verdict));
        }
        c.detected.fetch_add(1, Ordering::Relaxed);
        next.execute(request, connection)
    }

    /// CPU only, bounded by the inspected sizes.
    fn runs_inline(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }

    /// Bodies are inspected, so they arrive whole (unless body inspection
    /// is off).
    fn accepts_streamed_body(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        self.waf.config.inspect_body_bytes == 0
    }
}

#[doc(hidden)]
pub mod fuzz {
    /// Fuzz entry: normalize and run every detection on arbitrary text.
    pub fn inspect(data: &[u8]) {
        let text = String::from_utf8_lossy(data);
        let v = super::decode::normalize(&text);
        let mut hits = Vec::new();
        super::detect::value(&v, &mut hits);
        let _ = super::detect::scanner(&v);
        let _ = super::json_strings(&text);
        let _ = super::multipart_fields(data, "multipart/form-data; boundary=x");
    }
}

#[cfg(test)]
mod tests;
