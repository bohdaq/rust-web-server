//! FastCGI (18.21.0, #72): PHP through php-fpm, and any other FastCGI
//! responder, without FrankenPHP or a second web server.
//!
//! [`FastCgi`] is an [`Application`]: it maps a request onto a script under
//! its root the way nginx's and Caddy's PHP setups do, sends it to the
//! FastCGI server over TCP or a Unix socket, and turns the CGI response
//! (RFC 3875 §6) back into a [`Response`]. Connections are kept alive and
//! pooled. The config file's `type = "fastcgi"` action builds one.
//!
//! Which script runs for a request path `P` (with `split_path = ".php"`):
//!
//! 1. `P` names a file under the root that isn't a script → served as a
//!    static file (when a static handler is attached).
//! 2. `P` is a directory with an index script (`index.php`) → that script
//!    (`/dir` without the slash gets a redirect to `/dir/`).
//! 3. `P` contains `.php` and the part up to it is a script file → that
//!    script, the rest is `PATH_INFO` (`/index.php/users/7`).
//! 4. Otherwise the front controller (`/index.php`, the Laravel / Symfony /
//!    WordPress pattern), with `REQUEST_URI` unchanged.
//!
//! A script must be a regular file under the root whose path ends in the
//! split extension and has no dot-file segment, so `/uploads/x.jpg/a.php`
//! never runs `x.jpg` (the old `cgi.fix_pathinfo` hole).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::application::Application;
use crate::core::New;
use crate::header::Header;
use crate::range::Range;
use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

// ── Records (FastCGI 1.0 spec, §3, §8) ──────────────────────────────────────

pub(crate) const VERSION: u8 = 1;
pub(crate) const BEGIN_REQUEST: u8 = 1;
pub(crate) const ABORT_REQUEST: u8 = 2;
pub(crate) const END_REQUEST: u8 = 3;
pub(crate) const PARAMS: u8 = 4;
pub(crate) const STDIN: u8 = 5;
pub(crate) const STDOUT: u8 = 6;
pub(crate) const STDERR: u8 = 7;
const RESPONDER: u16 = 1;
const KEEP_CONN: u8 = 1;
const MAX_CONTENT: usize = 65535;

/// One record: type, request id and content.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub kind: u8,
    pub request_id: u16,
    pub content: Vec<u8>,
}

/// Append a record (content ≤ 65535 bytes) to `out`, padded to 8 bytes.
pub fn encode_record(out: &mut Vec<u8>, kind: u8, request_id: u16, content: &[u8]) {
    debug_assert!(content.len() <= MAX_CONTENT);
    let padding = (8 - content.len() % 8) % 8;
    out.extend_from_slice(&[VERSION, kind]);
    out.extend_from_slice(&request_id.to_be_bytes());
    out.extend_from_slice(&(content.len() as u16).to_be_bytes());
    out.extend_from_slice(&[padding as u8, 0]);
    out.extend_from_slice(content);
    out.extend(std::iter::repeat_n(0u8, padding));
}

/// Append a stream (PARAMS, STDIN) as records, then its empty terminator.
pub fn encode_stream(out: &mut Vec<u8>, kind: u8, request_id: u16, data: &[u8]) {
    for chunk in data.chunks(MAX_CONTENT) {
        encode_record(out, kind, request_id, chunk);
    }
    encode_record(out, kind, request_id, &[]);
}

/// Name-value pairs (§3.4): lengths < 128 in one byte, others in four with
/// the top bit set.
pub fn encode_params(params: &[(String, String)]) -> Vec<u8> {
    let mut out = Vec::new();
    let length = |out: &mut Vec<u8>, n: usize| {
        if n < 128 {
            out.push(n as u8);
        } else {
            out.extend_from_slice(&((n as u32) | 0x8000_0000).to_be_bytes());
        }
    };
    for (name, value) in params {
        length(&mut out, name.len());
        length(&mut out, value.len());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(value.as_bytes());
    }
    out
}

/// Decode name-value pairs; `None` on truncated or oversized input.
pub fn decode_params(mut data: &[u8]) -> Option<Vec<(String, String)>> {
    fn length(data: &mut &[u8]) -> Option<usize> {
        let first = *data.first()?;
        if first < 128 {
            *data = &data[1..];
            return Some(first as usize);
        }
        let bytes: [u8; 4] = data.get(..4)?.try_into().ok()?;
        *data = &data[4..];
        Some((u32::from_be_bytes(bytes) & 0x7fff_ffff) as usize)
    }
    let mut out = Vec::new();
    while !data.is_empty() {
        let (n, v) = (length(&mut data)?, length(&mut data)?);
        if data.len() < n.checked_add(v)? {
            return None;
        }
        out.push((String::from_utf8_lossy(&data[..n]).into_owned(), String::from_utf8_lossy(&data[n..n + v]).into_owned()));
        data = &data[n + v..];
    }
    Some(out)
}

/// Read one record; `Ok(None)` at a clean end of stream.
pub fn read_record(reader: &mut impl Read) -> std::io::Result<Option<Record>> {
    let mut header = [0u8; 8];
    match reader.read(&mut header[..1])? {
        0 => return Ok(None),
        _ => reader.read_exact(&mut header[1..])?,
    }
    if header[0] != VERSION {
        return Err(std::io::Error::other(format!("FastCGI: unsupported record version {}", header[0])));
    }
    let length = u16::from_be_bytes([header[4], header[5]]) as usize;
    let mut content = vec![0u8; length + header[6] as usize];
    reader.read_exact(&mut content)?;
    content.truncate(length);
    Ok(Some(Record { kind: header[1], request_id: u16::from_be_bytes([header[2], header[3]]), content }))
}

/// Parse a buffer of records completely (for the fuzzer and tests):
/// `None` on a malformed record.
pub fn parse_records(mut data: &[u8]) -> Option<Vec<Record>> {
    let mut out = Vec::new();
    while !data.is_empty() {
        out.push(read_record(&mut data).ok()??);
    }
    Some(out)
}

// ── CGI response (RFC 3875 §6) ──────────────────────────────────────────────

/// The end of the CGI header block in `data` (index after the blank line).
fn header_end(data: &[u8]) -> Option<usize> {
    let crlf = data.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4);
    let lf = data.windows(2).position(|w| w == b"\n\n").map(|i| i + 2);
    match (crlf, lf) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// Status, reason and headers of a CGI response header block. `Status:`
/// sets the status; a `Location:` without one is a `302`.
pub fn parse_cgi_headers(block: &[u8]) -> Result<(u16, String, Vec<(String, String)>), String> {
    let text = String::from_utf8_lossy(block);
    let mut status: Option<(u16, String)> = None;
    let mut headers = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(format!("FastCGI: bad header line {line:?}"));
        };
        let (name, value) = (name.trim(), value.trim());
        if name.is_empty() || name.bytes().any(|b| !(b.is_ascii_graphic()) || b == b':') || value.contains(['\r', '\n']) {
            return Err(format!("FastCGI: bad header line {line:?}"));
        }
        if name.eq_ignore_ascii_case("status") {
            let (code, reason) = value.split_once(' ').unwrap_or((value, ""));
            let code: u16 = code.parse().ok().filter(|c| (100..1000).contains(c)).ok_or_else(|| format!("FastCGI: bad Status {value:?}"))?;
            status = Some((code, reason.trim().to_string()));
        } else {
            headers.push((name.to_string(), value.to_string()));
        }
    }
    let (code, reason) = match status {
        Some(s) => s,
        None if headers.iter().any(|(n, _)| n.eq_ignore_ascii_case("location")) => (302, String::new()),
        None => (200, String::new()),
    };
    Ok((code, reason, headers))
}

fn reason_for(code: u16) -> String {
    Response::status_code_reason_phrase_list()
        .iter()
        .find(|s| *s.status_code == code as i16)
        .map(|s| s.reason_phrase.to_string())
        .unwrap_or_else(|| if code < 400 { "OK".to_string() } else { "Error".to_string() })
}

// ── Connections ─────────────────────────────────────────────────────────────

/// Where the FastCGI server listens.
#[derive(Debug, Clone, PartialEq)]
pub enum Address {
    Tcp(String),
    #[cfg(unix)]
    Unix(PathBuf),
}

impl Address {
    /// `unix:/run/php/php-fpm.sock`, `/run/php/php-fpm.sock`, `tcp://host:port`
    /// or `host:port`.
    pub fn parse(address: &str) -> Result<Address, String> {
        let address = address.trim();
        if let Some(path) = address.strip_prefix("unix:").or_else(|| address.starts_with('/').then_some(address)) {
            #[cfg(unix)]
            return Ok(Address::Unix(PathBuf::from(path)));
            #[cfg(not(unix))]
            return Err(format!("FastCGI: Unix sockets aren't available here ({path})"));
        }
        let hostport = address.strip_prefix("tcp://").unwrap_or(address);
        if hostport.rsplit_once(':').is_some_and(|(h, p)| !h.is_empty() && p.parse::<u16>().is_ok()) {
            Ok(Address::Tcp(hostport.to_string()))
        } else {
            Err(format!("FastCGI: address {address:?}: expected unix:/path or host:port"))
        }
    }
}

trait Stream: Read + Write + Send {}
impl<T: Read + Write + Send> Stream for T {}

struct Conn {
    stream: Box<dyn Stream>,
}

impl Conn {
    fn open(address: &Address, connect: Duration, io: Duration) -> std::io::Result<Conn> {
        match address {
            Address::Tcp(hostport) => {
                let mut last = std::io::Error::other(format!("no address for {hostport}"));
                for addr in std::net::ToSocketAddrs::to_socket_addrs(hostport.as_str())? {
                    match std::net::TcpStream::connect_timeout(&addr, connect) {
                        Ok(s) => {
                            s.set_read_timeout(Some(io))?;
                            s.set_write_timeout(Some(io))?;
                            let _ = s.set_nodelay(true);
                            return Ok(Conn { stream: Box::new(s) });
                        }
                        Err(e) => last = e,
                    }
                }
                Err(last)
            }
            #[cfg(unix)]
            Address::Unix(path) => {
                let s = std::os::unix::net::UnixStream::connect(path)?;
                s.set_read_timeout(Some(io))?;
                s.set_write_timeout(Some(io))?;
                Ok(Conn { stream: Box::new(s) })
            }
        }
    }
}

/// Idle kept-alive connections, at most `max` per FastCGI server.
struct Pool {
    idle: Mutex<Vec<Conn>>,
    max: usize,
}

// ── The application ─────────────────────────────────────────────────────────

/// Settings of a [`FastCgi`] handler.
#[derive(Debug, Clone)]
pub struct FastCgiConfig {
    /// The FastCGI server.
    pub address: String,
    /// Document root: scripts and static files live under it.
    pub root: PathBuf,
    /// Index scripts of a directory (default `index.php`).
    pub index: Vec<String>,
    /// The script extension that splits `PATH_INFO` off (default `.php`).
    pub split_path: String,
    /// Script for paths that match nothing (default `index.php`; empty: 404).
    pub front_controller: String,
    /// Extra CGI variables (`APP_ENV = "prod"`).
    pub params: Vec<(String, String)>,
    pub connect_timeout: Duration,
    /// Per read/write on the connection (a slow script waits this long).
    pub read_timeout: Duration,
    /// Idle connections kept per server.
    pub max_idle: usize,
}

impl FastCgiConfig {
    pub fn new(address: impl Into<String>, root: impl Into<PathBuf>) -> FastCgiConfig {
        FastCgiConfig {
            address: address.into(),
            root: root.into(),
            index: vec!["index.php".to_string()],
            split_path: ".php".to_string(),
            front_controller: "index.php".to_string(),
            params: Vec::new(),
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(60),
            max_idle: 16,
        }
    }
}

/// FastCGI as an [`Application`]. See the [module docs](self).
#[derive(Clone)]
pub struct FastCgi {
    inner: Arc<Inner>,
}

struct Inner {
    config: FastCgiConfig,
    address: Address,
    root: PathBuf,
    pool: Pool,
    statics: Option<Arc<dyn Application + Send + Sync>>,
}

/// What a request path maps to.
#[derive(Debug, PartialEq)]
pub(crate) enum Target {
    /// A script: `SCRIPT_NAME`, its file, `PATH_INFO`.
    Script { script_name: String, filename: PathBuf, path_info: String },
    /// A file that isn't a script.
    Static,
    /// A directory without its trailing slash: redirect here.
    Redirect(String),
    NotFound,
}

impl FastCgi {
    /// A handler for `config`; `Err` for a bad address or a missing root.
    pub fn new(config: FastCgiConfig) -> Result<FastCgi, String> {
        let address = Address::parse(&config.address)?;
        let root = config.root.canonicalize().map_err(|e| format!("FastCGI root {}: {e}", config.root.display()))?;
        let max = config.max_idle;
        Ok(FastCgi { inner: Arc::new(Inner { config, address, root, pool: Pool { idle: Mutex::new(Vec::new()), max }, statics: None }) })
    }

    /// Serve files under the root that aren't scripts with `app`.
    pub fn with_static(self, app: Arc<dyn Application + Send + Sync>) -> FastCgi {
        let inner = &self.inner;
        FastCgi {
            inner: Arc::new(Inner {
                config: inner.config.clone(),
                address: inner.address.clone(),
                root: inner.root.clone(),
                pool: Pool { idle: Mutex::new(Vec::new()), max: inner.pool.max },
                statics: Some(app),
            }),
        }
    }

    fn is_script(&self, name: &str) -> bool {
        !self.inner.config.split_path.is_empty() && name.ends_with(&self.inner.config.split_path)
    }

    /// A regular file under the root (symlinks resolved), or `None`.
    fn file_under_root(&self, relative: &str) -> Option<PathBuf> {
        let path = self.inner.root.join(relative.trim_start_matches('/'));
        let real = path.canonicalize().ok()?;
        (real.starts_with(&self.inner.root) && real.is_file()).then_some(real)
    }

    pub(crate) fn resolve(&self, uri_path: &str) -> Target {
        let decoded = crate::url::URL::percent_decode(uri_path);
        let segments: Vec<&str> = decoded.split('/').collect();
        if segments.iter().any(|s| *s == ".." || (s.starts_with('.') && !s.is_empty())) || decoded.contains('\0') {
            return Target::NotFound;
        }
        let path = if decoded.starts_with('/') { decoded.clone() } else { format!("/{decoded}") };
        let full = self.inner.root.join(path.trim_start_matches('/'));
        // 1. A file.
        if let Some(file) = self.file_under_root(&path) {
            if self.is_script(&path) {
                return Target::Script { script_name: path, filename: file, path_info: String::new() };
            }
            return Target::Static;
        }
        // 2. A directory with an index script.
        if full.is_dir() && full.canonicalize().is_ok_and(|r| r.starts_with(&self.inner.root)) {
            for index in &self.inner.config.index {
                let script = format!("{}/{}", path.trim_end_matches('/'), index);
                if let Some(file) = self.file_under_root(&script) {
                    if !path.ends_with('/') {
                        return Target::Redirect(format!("{path}/"));
                    }
                    if self.is_script(&script) {
                        return Target::Script { script_name: script, filename: file, path_info: String::new() };
                    }
                    return Target::Static;
                }
            }
        }
        // 3. /script.php/path/info
        let split = &self.inner.config.split_path;
        if !split.is_empty() {
            let mut from = 0;
            while let Some(found) = path[from..].find(split.as_str()) {
                let end = from + found + split.len();
                if path[end..].starts_with('/') {
                    let script = &path[..end];
                    if let Some(file) = self.file_under_root(script) {
                        return Target::Script { script_name: script.to_string(), filename: file, path_info: path[end..].to_string() };
                    }
                }
                from = end;
            }
        }
        // 4. The front controller.
        let front = &self.inner.config.front_controller;
        if !front.is_empty() {
            let script = format!("/{}", front.trim_start_matches('/'));
            if let Some(file) = self.file_under_root(&script) {
                return Target::Script { script_name: script, filename: file, path_info: String::new() };
            }
        }
        Target::NotFound
    }

    /// The CGI/1.1 variables for `request` running `script_name`.
    pub(crate) fn params(&self, request: &Request, connection: &ConnectionInfo, script_name: &str, filename: &Path, path_info: &str) -> Vec<(String, String)> {
        let (_, query) = request.request_uri.split_once('?').unwrap_or((&request.request_uri, ""));
        let host = request.header("Host").map(|h| h.value.to_string()).unwrap_or_default();
        let server_name = match host.rsplit_once(':') {
            Some((h, p)) if p.bytes().all(|b| b.is_ascii_digit()) && !h.ends_with(':') => h.to_string(),
            _ => host.clone(),
        };
        let root = self.inner.root.to_string_lossy().to_string();
        let mut params: Vec<(String, String)> = vec![
            ("GATEWAY_INTERFACE".into(), "CGI/1.1".into()),
            ("SERVER_SOFTWARE".into(), format!("rws/{}", env!("CARGO_PKG_VERSION"))),
            ("SERVER_PROTOCOL".into(), "HTTP/1.1".into()),
            ("REQUEST_METHOD".into(), request.method.clone()),
            ("REQUEST_URI".into(), request.request_uri.clone()),
            ("QUERY_STRING".into(), query.to_string()),
            ("SCRIPT_NAME".into(), script_name.to_string()),
            ("SCRIPT_FILENAME".into(), filename.to_string_lossy().to_string()),
            ("DOCUMENT_ROOT".into(), root.clone()),
            ("DOCUMENT_URI".into(), format!("{script_name}{path_info}")),
            ("PATH_INFO".into(), path_info.to_string()),
            ("REMOTE_ADDR".into(), connection.client.ip.clone()),
            ("REMOTE_PORT".into(), connection.client.port.to_string()),
            ("SERVER_ADDR".into(), connection.server.ip.clone()),
            ("SERVER_PORT".into(), connection.server.port.to_string()),
            ("SERVER_NAME".into(), server_name),
            ("REQUEST_SCHEME".into(), if connection.tls { "https" } else { "http" }.into()),
            // php-cgi built with force-cgi-redirect wants it.
            ("REDIRECT_STATUS".into(), "200".into()),
        ];
        if !path_info.is_empty() {
            params.push(("PATH_TRANSLATED".into(), format!("{root}{path_info}")));
        }
        if connection.tls {
            params.push(("HTTPS".into(), "on".into()));
        }
        params.push(("CONTENT_LENGTH".into(), if request.body.is_empty() { String::new() } else { request.body.len().to_string() }));
        params.push(("CONTENT_TYPE".into(), request.header("Content-Type").map(|h| h.value.to_string()).unwrap_or_default()));
        for header in &request.headers {
            let name = header.name.to_ascii_uppercase().replace('-', "_");
            // httpoxy: a client's `Proxy:` header must never become HTTP_PROXY.
            if name == "PROXY" || name == "CONTENT_TYPE" || name == "CONTENT_LENGTH" || name.bytes().any(|b| !(b.is_ascii_alphanumeric() || b == b'_')) {
                continue;
            }
            let key = format!("HTTP_{name}");
            match params.iter_mut().find(|(k, _)| *k == key) {
                Some((_, v)) => {
                    v.push_str(if name == "COOKIE" { "; " } else { ", " });
                    v.push_str(&header.value);
                }
                None => params.push((key, header.value.to_string())),
            }
        }
        for (k, v) in &self.inner.config.params {
            match params.iter_mut().find(|(name, _)| name == k) {
                Some((_, value)) => *value = v.clone(),
                None => params.push((k.clone(), v.clone())),
            }
        }
        params
    }

    fn take_conn(&self) -> Option<Conn> {
        self.inner.pool.idle.lock().unwrap_or_else(|e| e.into_inner()).pop()
    }

    fn put_conn(&self, conn: Conn) {
        let mut idle = self.inner.pool.idle.lock().unwrap_or_else(|e| e.into_inner());
        if idle.len() < self.inner.pool.max {
            idle.push(conn);
        }
    }

    /// Run the script: the CGI output split into header block and body.
    fn exchange(&self, params: &[(String, String)], body: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
        let mut message = Vec::with_capacity(body.len() + 1024);
        let mut begin = Vec::with_capacity(8);
        begin.extend_from_slice(&RESPONDER.to_be_bytes());
        begin.extend_from_slice(&[KEEP_CONN, 0, 0, 0, 0, 0]);
        encode_record(&mut message, BEGIN_REQUEST, 1, &begin);
        encode_stream(&mut message, PARAMS, 1, &encode_params(params));
        encode_stream(&mut message, STDIN, 1, body);
        let cfg = &self.inner.config;
        // A pooled connection may have been closed by the server: retry once
        // on a fresh one when nothing came back.
        let mut tried_fresh = false;
        loop {
            let (mut conn, pooled) = match self.take_conn() {
                Some(c) if !tried_fresh => (c, true),
                _ => (Conn::open(&self.inner.address, cfg.connect_timeout, cfg.read_timeout).map_err(|e| format!("FastCGI server {}: {e}", cfg.address))?, false),
            };
            match self.round_trip(&mut conn, &message) {
                Ok((out, keep)) => {
                    if keep {
                        self.put_conn(conn);
                    }
                    let end = header_end(&out).ok_or_else(|| "FastCGI: the response has no header block".to_string())?;
                    let mut out = out;
                    let body = out.split_off(end);
                    return Ok((out, body));
                }
                Err((e, nothing_read)) if pooled && nothing_read => {
                    let _ = e;
                    tried_fresh = true;
                }
                Err((e, _)) => return Err(format!("FastCGI server {}: {e}", cfg.address)),
            }
        }
    }

    /// Send `message`, read STDOUT until END_REQUEST. `Err((error, nothing
    /// was read))`; `Ok((stdout, connection reusable))`.
    fn round_trip(&self, conn: &mut Conn, message: &[u8]) -> Result<(Vec<u8>, bool), (String, bool)> {
        conn.stream.write_all(message).and_then(|_| conn.stream.flush()).map_err(|e| (e.to_string(), true))?;
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut read_any = false;
        loop {
            let record = match read_record(&mut conn.stream) {
                Ok(Some(r)) => r,
                Ok(None) => return Err(("the connection closed before the response ended".to_string(), !read_any)),
                Err(e) => return Err((e.to_string(), !read_any)),
            };
            read_any = true;
            if record.request_id != 1 {
                continue;
            }
            match record.kind {
                STDOUT => stdout.extend_from_slice(&record.content),
                STDERR => stderr.extend_from_slice(&record.content),
                END_REQUEST => {
                    let text = String::from_utf8_lossy(&stderr);
                    for line in text.lines().filter(|l| !l.trim().is_empty()) {
                        eprintln!("[fastcgi] {line}");
                    }
                    // protocolStatus 0 = request complete.
                    let complete = record.content.get(4).is_none_or(|s| *s == 0);
                    if !complete {
                        return Err((format!("the server refused the request (protocol status {})", record.content[4]), false));
                    }
                    return Ok((stdout, true));
                }
                _ => {}
            }
        }
    }
}

fn simple(status: u16, headers: Vec<Header>, body: &str) -> Response {
    let mut r = Response::new();
    r.status_code = status as i16;
    r.reason_phrase = reason_for(status);
    r.headers.extend(headers);
    if !body.is_empty() {
        r.content_range_list = vec![Range::get_content_range(body.as_bytes().to_vec(), "text/plain".to_string())];
    }
    r
}

impl Application for FastCgi {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        let Some(path) = crate::url::URL::request_target_path(&request.request_uri) else {
            return Ok(simple(400, Vec::new(), "Bad Request"));
        };
        match self.resolve(path) {
            Target::NotFound => Ok(simple(404, Vec::new(), "Not Found")),
            Target::Redirect(location) => {
                let query = request.request_uri.split_once('?').map(|(_, q)| format!("?{q}")).unwrap_or_default();
                Ok(simple(301, vec![Header::new("Location", format!("{location}{query}"))], ""))
            }
            Target::Static => match &self.inner.statics {
                Some(app) => app.execute(request, connection),
                None => Ok(simple(404, Vec::new(), "Not Found")),
            },
            Target::Script { script_name, filename, path_info } => {
                let params = self.params(request, connection, &script_name, &filename, &path_info);
                let (head, body) = match self.exchange(&params, &request.body) {
                    Ok(out) => out,
                    Err(e) => {
                        eprintln!("[fastcgi] {e}");
                        return Ok(simple(502, Vec::new(), "Bad Gateway"));
                    }
                };
                let (status, reason, headers) = match parse_cgi_headers(&head) {
                    Ok(h) => h,
                    Err(e) => {
                        eprintln!("[fastcgi] {e}");
                        return Ok(simple(502, Vec::new(), "Bad Gateway"));
                    }
                };
                let mut response = Response::new();
                response.status_code = status as i16;
                response.reason_phrase = if reason.is_empty() { reason_for(status) } else { reason };
                let mut content_type = String::new();
                for (name, value) in headers {
                    if name.eq_ignore_ascii_case("content-type") {
                        content_type = value;
                    } else if !name.eq_ignore_ascii_case("content-length") && !name.eq_ignore_ascii_case("transfer-encoding") && !name.eq_ignore_ascii_case("connection") {
                        response.headers.push(Header::new(name, value));
                    }
                }
                if !body.is_empty() || !content_type.is_empty() {
                    let content_type = if content_type.is_empty() { "application/octet-stream".to_string() } else { content_type };
                    response.content_range_list = vec![Range::get_content_range(body, content_type)];
                }
                Ok(response)
            }
        }
    }
}

/// Parse the FastCGI routes' `params` table into pairs (for the config file).
pub fn params_from_map(map: &HashMap<String, String>) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    v.sort();
    v
}

#[doc(hidden)]
pub mod fuzz {
    /// Fuzz entry: records, params and a CGI header block from bytes.
    pub fn records(data: &[u8]) {
        if let Some(records) = super::parse_records(data) {
            for r in records {
                if r.kind == super::PARAMS {
                    let _ = super::decode_params(&r.content);
                }
            }
        }
        let _ = super::decode_params(data);
        if let Some(end) = super::header_end(data) {
            let _ = super::parse_cgi_headers(&data[..end]);
        }
    }
}

#[cfg(test)]
mod tests;
