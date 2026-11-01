//! `rws ctl` (18.20.0, #70): the admin API from the command line.
//!
//! ```text
//! rws ctl status                         # version, uptime, listeners, disks
//! rws ctl reload                         # waits; prints the errors and exits 1 on a bad config
//! rws ctl drain <upstream> <backend> [--undo]
//! rws ctl maintenance [on|off]
//! rws ctl block <ip> | unblock <ip> | blocklist
//! rws ctl purge <cache> [--path /p | --prefix /p/]
//! rws ctl caches | upstreams
//! rws ctl certs [renew]
//! rws ctl logs [-n 100] [-f]
//! rws ctl traffic [--week | --month | --hours N] [--host example.com]
//! rws ctl whoami                         # the token's user and role (18.22.0)
//! rws ctl audit [-n 50]                  # recent changes: who, what, when (18.22.0)
//! ```
//!
//! The binary run as `rwsctl` (the packages install that symlink) acts as
//! `rws ctl`. Where it connects: `--admin http://127.0.0.1:9901`, else
//! `[admin] listen` of `RWS_CONFIG_FILE` / `rws.config.toml`, else
//! `127.0.0.1:9901`. The token: `--token-file`, else the variable
//! `[admin] token_env` names, else `RWS_ADMIN_TOKEN` — never an argument, so
//! it can't show in `ps`. Output is for people; `--json` prints the API's
//! JSON. Exit codes: 0 done, 1 refused or failed, 2 usage error, admin API
//! unreachable or token refused.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use crate::admin::{API_PREFIX, DEFAULT_TOKEN_ENV};
use crate::entry_point::Config;
use crate::mcp::json_rpc;

/// Done.
pub const EXIT_OK: i32 = 0;
/// The server refused or the operation failed.
pub const EXIT_FAILED: i32 = 1;
/// Bad arguments, the admin API unreachable, or the token refused.
pub const EXIT_UNUSABLE: i32 = 2;

const DEFAULT_ADMIN: &str = "127.0.0.1:9901";

const USAGE: &str = "\
Usage: rws ctl [--admin <url>] [--token-file <path>] [--json] <command>

Commands:
  status                               version, uptime, listeners, disks
  reload                               reload the configuration and wait for it
  drain <upstream> <backend> [--undo]  stop sending new requests to a backend
  maintenance [on|off]                 show or switch maintenance mode
  block <ip> | unblock <ip>            change the IP blocklist
  blocklist                            list blocked IPs
  purge <cache> [--path P | --prefix P]  purge a cache (all of it by default)
  caches | upstreams                   list caches / upstream backends
  certs [renew]                        certificates and ACME state; renew orders now
  logs [-n N] [-f]                     recent access-log lines; -f follows
  traffic [--week|--month|--hours N] [--host H]  who visited
  whoami                               the token's user and role
  audit [-n N]                         recent changes: who did what, when

The admin address comes from --admin, else [admin] listen in RWS_CONFIG_FILE or
rws.config.toml, else 127.0.0.1:9901. The token from --token-file, else the
variable [admin] token_env names, else RWS_ADMIN_TOKEN.
Exit codes: 0 done, 1 refused or failed, 2 usage, unreachable or unauthorized.
";

/// What went wrong, and the exit code it means.
#[derive(Debug)]
struct Failure {
    code: i32,
    message: String,
}

fn usage(message: impl Into<String>) -> Failure {
    Failure { code: EXIT_UNUSABLE, message: format!("{}\n\n{USAGE}", message.into()) }
}

fn failed(message: impl Into<String>) -> Failure {
    Failure { code: EXIT_FAILED, message: message.into() }
}

fn unusable(message: impl Into<String>) -> Failure {
    Failure { code: EXIT_UNUSABLE, message: message.into() }
}

/// Whether the program was started as `rwsctl` (its file name, any extension).
pub fn invoked_as_rwsctl(argv0: &str) -> bool {
    let name = argv0.rsplit(['/', '\\']).next().unwrap_or(argv0);
    name == "rwsctl" || name.strip_suffix(".exe").is_some_and(|n| n.eq_ignore_ascii_case("rwsctl"))
}

/// The `rws ctl` arguments in `args` (a full `argv`): after `ctl`, or all
/// of them when run as `rwsctl`. `None` for a normal server start.
pub fn requested(args: &[String]) -> Option<&[String]> {
    if args.first().is_some_and(|a| invoked_as_rwsctl(a)) {
        return Some(&args[1..]);
    }
    (args.get(1).map(String::as_str) == Some("ctl")).then(|| &args[2..])
}

/// Run `rws ctl <args>`, writing to `out` and `err`; returns the exit code.
pub fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    match parse(args).and_then(|cmd| execute(&cmd, out)) {
        Ok(()) => EXIT_OK,
        Err(f) => {
            let _ = writeln!(err, "{}", f.message.trim_end());
            f.code
        }
    }
}

/// Parsed command line.
#[derive(Debug, Default)]
struct Command {
    admin: Option<String>,
    token_file: Option<String>,
    json: bool,
    words: Vec<String>,
    undo: bool,
    follow: bool,
    lines: Option<usize>,
    path: Option<String>,
    prefix: Option<String>,
    hours: Option<u64>,
    host: Option<String>,
}

fn parse(args: &[String]) -> Result<Command, Failure> {
    let mut cmd = Command::default();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f, Some(v.to_string())),
            _ => (arg.as_str(), None),
        };
        let mut value = |name: &str| inline.clone().or_else(|| args.next().cloned()).ok_or_else(|| usage(format!("{name} needs a value")));
        match flag {
            "--admin" => cmd.admin = Some(value("--admin")?),
            "--token-file" => cmd.token_file = Some(value("--token-file")?),
            "--json" => cmd.json = true,
            "--undo" => cmd.undo = true,
            "-f" | "--follow" => cmd.follow = true,
            "-n" | "--lines" => cmd.lines = Some(value("-n")?.parse().map_err(|_| usage("-n takes a number"))?),
            "--path" => cmd.path = Some(value("--path")?),
            "--prefix" => cmd.prefix = Some(value("--prefix")?),
            "--hours" => cmd.hours = Some(value("--hours")?.parse().map_err(|_| usage("--hours takes a number"))?),
            "--week" => cmd.hours = Some(24 * 7),
            "--month" => cmd.hours = Some(24 * 30),
            "--host" => cmd.host = Some(value("--host")?),
            "--token" => return Err(usage("the token is read from --token-file or RWS_ADMIN_TOKEN, never the command line (it would show in ps)")),
            "-h" | "--help" | "help" if cmd.words.is_empty() => cmd.words.push("help".to_string()),
            f if f.starts_with('-') && f.len() > 1 && f.parse::<f64>().is_err() => return Err(usage(format!("unknown option {f}"))),
            _ => cmd.words.push(arg.clone()),
        }
    }
    Ok(cmd)
}

/// Where the admin API is: `http://host:port`, no trailing slash.
fn admin_base(cmd: &Command) -> String {
    let configured = cmd.admin.clone().or_else(|| config_value(Config::RWS_CONFIG_ADMIN_LISTEN)).filter(|a| !a.trim().is_empty());
    let address = configured.unwrap_or_else(|| DEFAULT_ADMIN.to_string());
    let address = address.trim().trim_end_matches('/');
    if address.starts_with("https://") {
        return address.to_string(); // refused when connecting
    }
    let address = address.strip_prefix("http://").unwrap_or(address);
    // A wildcard listen address is reached over loopback.
    let address = match address.rsplit_once(':') {
        Some(("0.0.0.0", port)) => format!("127.0.0.1:{port}"),
        Some(("[::]", port)) => format!("[::1]:{port}"),
        _ => address.to_string(),
    };
    format!("http://{address}")
}

/// A setting from the environment or the config file (`RWS_CONFIG_FILE`,
/// else `rws.config.toml` here), without changing the environment.
fn config_value(name: &str) -> Option<String> {
    if let Ok(v) = std::env::var(name) {
        if !v.trim().is_empty() {
            return Some(v.trim().to_string());
        }
    }
    let file = std::env::var("RWS_CONFIG_FILE").ok().filter(|f| !f.trim().is_empty()).unwrap_or_else(|| "rws.config.toml".to_string());
    let text = std::fs::read(file).ok()?;
    let settings = crate::entry_point::config_file::collect_settings(std::io::Cursor::new(&text[..]), String::new()).ok()?;
    settings.settings.into_iter().rev().find(|(k, _)| k == name).map(|(_, v)| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn token(cmd: &Command) -> Result<String, Failure> {
    if let Some(path) = &cmd.token_file {
        let token = std::fs::read_to_string(path).map_err(|e| unusable(format!("can't read the token file {path}: {e}")))?;
        return Some(token.trim().to_string()).filter(|t| !t.is_empty()).ok_or_else(|| unusable(format!("the token file {path} is empty")));
    }
    let mut names = Vec::new();
    if let Some(name) = config_value(Config::RWS_CONFIG_ADMIN_TOKEN_ENV) {
        names.push(name);
    }
    names.push(DEFAULT_TOKEN_ENV.to_string());
    for name in &names {
        if let Ok(t) = std::env::var(name) {
            if !t.trim().is_empty() {
                return Ok(t.trim().to_string());
            }
        }
    }
    Err(unusable(format!("no admin token: set {} or pass --token-file", names.join(" or "))))
}

/// The admin API's answer.
struct Reply {
    status: u16,
    body: String,
}

/// One request to the admin API (`path` under [`API_PREFIX`]).
struct Api {
    base: String,
    token: String,
}

impl Api {
    fn connect(&self, method: &str, path: &str, body: Option<&str>) -> Result<BufReader<TcpStream>, Failure> {
        let host = self.base.trim_start_matches("http://");
        if self.base.starts_with("https://") {
            return Err(unusable("the admin API speaks plain HTTP: use http:// (an SSH tunnel for remote servers)"));
        }
        let addrs: Vec<std::net::SocketAddr> =
            std::net::ToSocketAddrs::to_socket_addrs(host).map_err(|e| unusable(format!("admin API at {}: {e}", self.base)))?.collect();
        let mut last = None;
        let stream = addrs.iter().find_map(|a| match TcpStream::connect_timeout(a, Duration::from_secs(5)) {
            Ok(s) => Some(s),
            Err(e) => {
                last = Some(e);
                None
            }
        });
        let mut stream = stream.ok_or_else(|| {
            unusable(format!("can't reach the admin API at {}: {}", self.base, last.map_or("no address".to_string(), |e| e.to_string())))
        })?;
        let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
        let body = body.unwrap_or("");
        let content = if body.is_empty() { String::new() } else { format!("Content-Type: application/json\r\nContent-Length: {}\r\n", body.len()) };
        let head = format!(
            "{method} {API_PREFIX}{path} HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer {}\r\nUser-Agent: rws-ctl/{}\r\nAccept: application/json, text/event-stream\r\n{content}Connection: close\r\n\r\n",
            self.token,
            env!("CARGO_PKG_VERSION"),
        );
        stream.write_all(head.as_bytes()).and_then(|_| stream.write_all(body.as_bytes())).map_err(|e| unusable(format!("admin API at {}: {e}", self.base)))?;
        Ok(BufReader::new(stream))
    }

    /// Send, read the whole answer; 401/403 are [`EXIT_UNUSABLE`].
    fn call(&self, method: &str, path: &str, body: Option<&str>) -> Result<Reply, Failure> {
        let mut reader = self.connect(method, path, body)?;
        let (status, chunked) = read_head(&mut reader).map_err(|e| unusable(format!("admin API at {}: {e}", self.base)))?;
        let mut bytes = Vec::new();
        if chunked {
            Dechunk::new(reader).read_to_end(&mut bytes)
        } else {
            reader.read_to_end(&mut bytes)
        }
        .map_err(|e| unusable(format!("admin API at {}: {e}", self.base)))?;
        let reply = Reply { status, body: String::from_utf8_lossy(&bytes).into_owned() };
        match status {
            401 | 403 => Err(unusable(format!("the admin API refused the token ({status})"))),
            200..=299 => Ok(reply),
            _ => Err(failed(api_error(&reply))),
        }
    }
}

/// `{"error":"…"}` of a refusal, else the status and body.
fn api_error(reply: &Reply) -> String {
    json_rpc::extract_str(&reply.body, "error").unwrap_or_else(|| format!("admin API answered {}: {}", reply.status, reply.body.trim()))
}

/// The status line and headers; whether the body is chunked.
fn read_head(reader: &mut impl BufRead) -> std::io::Result<(u16, bool)> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let status = line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).ok_or_else(|| std::io::Error::other(format!("not an HTTP response: {:?}", line.trim())))?;
    let mut chunked = false;
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("transfer-encoding") && value.to_ascii_lowercase().contains("chunked") {
                chunked = true;
            }
        }
    }
    Ok((status, chunked))
}

/// A chunked body as plain bytes.
struct Dechunk<R> {
    inner: R,
    left: usize,
    done: bool,
}

impl<R: BufRead> Dechunk<R> {
    fn new(inner: R) -> Self {
        Dechunk { inner, left: 0, done: false }
    }
}

impl<R: BufRead> Read for Dechunk<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        while self.left == 0 {
            if self.done {
                return Ok(0);
            }
            let mut line = String::new();
            if self.inner.read_line(&mut line)? == 0 {
                self.done = true;
                return Ok(0);
            }
            let line = line.trim();
            if line.is_empty() {
                continue; // the CRLF after a chunk
            }
            let size = line.split(';').next().unwrap_or("");
            self.left = usize::from_str_radix(size.trim(), 16).map_err(|_| std::io::Error::other("bad chunk size"))?;
            if self.left == 0 {
                self.done = true;
                return Ok(0);
            }
        }
        let want = buf.len().min(self.left);
        let n = self.inner.read(&mut buf[..want])?;
        if n == 0 {
            self.done = true;
        }
        self.left -= n;
        Ok(n)
    }
}

// ── JSON helpers (the API's JSON is rws's own, so small helpers do) ──────────

fn members(json: &str) -> Vec<(String, String)> {
    json_rpc::object_members(json).unwrap_or_default()
}

fn member(json: &str, key: &str) -> String {
    members(json).into_iter().find(|(k, _)| k == key).map(|(_, v)| v).unwrap_or_default()
}

/// A raw JSON value as text: strings decoded, others as written.
fn text(raw: &str) -> String {
    let raw = raw.trim();
    if raw.starts_with('"') {
        json_rpc::extract_str(&format!(r#"{{"v":{raw}}}"#), "v").unwrap_or_default()
    } else {
        raw.to_string()
    }
}

fn field(json: &str, key: &str) -> String {
    text(&member(json, key))
}

fn elements(json: &str) -> Vec<String> {
    json_rpc::split_array_elements(json.trim())
}

fn quoted(s: &str) -> String {
    format!("\"{}\"", crate::mcp::json_escape(s))
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Rows as aligned columns.
fn table(out: &mut dyn Write, header: &[&str], rows: &[Vec<String>]) -> std::io::Result<()> {
    let mut widths: Vec<usize> = header.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let line = |cells: Vec<String>| {
        let last = cells.len() - 1;
        cells.iter().enumerate().map(|(i, c)| if i == last { c.clone() } else { format!("{c:<w$}", w = widths[i]) }).collect::<Vec<_>>().join("  ")
    };
    writeln!(out, "{}", line(header.iter().map(|h| h.to_string()).collect()))?;
    for row in rows {
        writeln!(out, "{}", line(row.clone()))?;
    }
    Ok(())
}

fn duration(secs: u64) -> String {
    match secs {
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m {}s", s / 60, s % 60),
        s if s < 86_400 => format!("{}h {}m", s / 3600, s % 3600 / 60),
        s => format!("{}d {}h", s / 86_400, s % 86_400 / 3600),
    }
}

fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = n as f64;
    let mut unit = 0;
    while v >= 1024.0 && unit < UNITS.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    if unit == 0 { format!("{n} B") } else { format!("{v:.1} {}", UNITS[unit]) }
}

// ── Commands ─────────────────────────────────────────────────────────────────

fn execute(cmd: &Command, out: &mut dyn Write) -> Result<(), Failure> {
    let words: Vec<&str> = cmd.words.iter().map(String::as_str).collect();
    if matches!(words.as_slice(), [] | ["help"]) {
        let _ = write!(out, "{USAGE}");
        return if words.is_empty() { Err(Failure { code: EXIT_UNUSABLE, message: String::new() }) } else { Ok(()) };
    }
    let api = Api { base: admin_base(cmd), token: token(cmd)? };
    let io = |e: std::io::Error| failed(e.to_string());
    let json_out = |out: &mut dyn Write, reply: &Reply| writeln!(out, "{}", reply.body.trim()).map_err(io);
    match words.as_slice() {
        ["status"] => {
            let r = api.call("GET", "/status", None)?;
            if cmd.json {
                return json_out(out, &r);
            }
            print_status(out, &r.body).map_err(io)
        }
        ["reload"] => {
            let r = api.call("POST", "/reload", None)?;
            if cmd.json {
                json_out(out, &r)?;
            }
            let errors: Vec<String> = elements(&member(&r.body, "errors")).iter().map(|e| text(e)).collect();
            if member(&r.body, "applied") == "true" {
                if !cmd.json {
                    writeln!(out, "reloaded").map_err(io)?;
                }
                Ok(())
            } else {
                Err(failed(format!("reload refused, the old configuration stays:\n  {}", errors.join("\n  "))))
            }
        }
        ["drain", upstream, backend] => {
            let body = format!(r#"{{"backend":{},"drained":{}}}"#, quoted(backend), !cmd.undo);
            let r = api.call("PUT", &format!("/upstreams/{}/drain", percent_encode(upstream)), Some(&body))?;
            if cmd.json {
                return json_out(out, &r);
            }
            writeln!(out, "{} {backend} of {upstream}", if cmd.undo { "undrained" } else { "drained" }).map_err(io)
        }
        ["maintenance"] => {
            let r = api.call("GET", "/maintenance", None)?;
            if cmd.json {
                return json_out(out, &r);
            }
            writeln!(out, "maintenance {}", if member(&r.body, "active") == "true" { "on" } else { "off" }).map_err(io)
        }
        ["maintenance", state @ ("on" | "off")] => {
            let r = api.call("PUT", "/maintenance", Some(&format!(r#"{{"active":{}}}"#, *state == "on")))?;
            if cmd.json {
                return json_out(out, &r);
            }
            writeln!(out, "maintenance {}", if member(&r.body, "active") == "true" { "on" } else { "off" }).map_err(io)
        }
        ["block", ip] => {
            let r = api.call("POST", "/blocklist", Some(&format!(r#"{{"ip":{}}}"#, quoted(ip))))?;
            if cmd.json {
                return json_out(out, &r);
            }
            writeln!(out, "blocked {}", field(&r.body, "blocked")).map_err(io)
        }
        ["unblock", ip] => {
            let r = api.call("DELETE", &format!("/blocklist/{}", percent_encode(ip)), None)?;
            if cmd.json {
                return json_out(out, &r);
            }
            writeln!(out, "unblocked {}", field(&r.body, "unblocked")).map_err(io)
        }
        ["blocklist"] => {
            let r = api.call("GET", "/blocklist", None)?;
            if cmd.json {
                return json_out(out, &r);
            }
            let ips = elements(&r.body);
            if ips.is_empty() {
                return writeln!(out, "no blocked IPs").map_err(io);
            }
            ips.iter().try_for_each(|ip| writeln!(out, "{}", text(ip))).map_err(io)
        }
        ["purge", cache] => {
            let mut path = format!("/caches/{}", percent_encode(cache));
            if let Some(p) = &cmd.path {
                path.push_str(&format!("?path={}", percent_encode(p)));
            } else if let Some(p) = &cmd.prefix {
                path.push_str(&format!("?prefix={}", percent_encode(p)));
            }
            let r = api.call("DELETE", &path, None)?;
            if cmd.json {
                return json_out(out, &r);
            }
            writeln!(out, "purged {} entries from {cache}", field(&r.body, "purged")).map_err(io)
        }
        ["caches"] => {
            let r = api.call("GET", "/caches", None)?;
            if cmd.json {
                return json_out(out, &r);
            }
            let rows: Vec<Vec<String>> = elements(&r.body)
                .iter()
                .map(|c| {
                    let size = field(c, "bytes").parse::<u64>().map(bytes).unwrap_or_else(|_| "-".to_string());
                    vec![field(c, "name"), field(c, "kind"), field(c, "entries"), size, field(c, "hits"), field(c, "misses")]
                })
                .collect();
            if rows.is_empty() {
                return writeln!(out, "no caches").map_err(io);
            }
            table(out, &["NAME", "KIND", "ENTRIES", "SIZE", "HITS", "MISSES"], &rows).map_err(io)
        }
        ["upstreams"] => {
            let r = api.call("GET", "/upstreams", None)?;
            if cmd.json {
                return json_out(out, &r);
            }
            let mut rows = Vec::new();
            for u in elements(&r.body) {
                for b in elements(&member(&u, "backends")) {
                    let breaker = field(&b, "breaker");
                    rows.push(vec![
                        field(&u, "name"),
                        field(&b, "url"),
                        field(&b, "weight"),
                        if field(&b, "healthy") == "true" { "yes" } else { "no" }.to_string(),
                        if breaker == "null" { "-".to_string() } else { breaker },
                        if field(&b, "drained") == "true" { "yes" } else { "no" }.to_string(),
                    ]);
                }
            }
            if rows.is_empty() {
                return writeln!(out, "no upstreams").map_err(io);
            }
            table(out, &["UPSTREAM", "BACKEND", "WEIGHT", "HEALTHY", "BREAKER", "DRAINED"], &rows).map_err(io)
        }
        ["certs"] => {
            let r = api.call("GET", "/certificates", None)?;
            if cmd.json {
                return json_out(out, &r);
            }
            print_certificates(out, &r.body).map_err(io)
        }
        ["certs", "renew"] => {
            let r = api.call("POST", "/certificates/renew", None)?;
            if cmd.json {
                return json_out(out, &r);
            }
            writeln!(out, "renewal requested; `rws ctl certs` shows the outcome").map_err(io)
        }
        ["logs"] if cmd.follow => follow_logs(&api, out),
        ["logs"] => {
            let r = api.call("GET", &format!("/logs?n={}", cmd.lines.unwrap_or(100)), None)?;
            if cmd.json {
                return json_out(out, &r);
            }
            elements(&r.body).iter().try_for_each(|l| writeln!(out, "{}", text(l))).map_err(io)
        }
        ["traffic"] => {
            let mut path = format!("/traffic?hours={}", cmd.hours.unwrap_or(24));
            if let Some(h) = &cmd.host {
                path.push_str(&format!("&host={}", percent_encode(h)));
            }
            let r = api.call("GET", &path, None)?;
            if cmd.json {
                return json_out(out, &r);
            }
            print_traffic(out, &r.body).map_err(io)
        }
        ["whoami"] => {
            let r = api.call("GET", "/whoami", None)?;
            if cmd.json {
                return json_out(out, &r);
            }
            writeln!(out, "{} ({}, via {})", field(&r.body, "user"), field(&r.body, "role"), field(&r.body, "via")).map_err(io)
        }
        ["audit"] => {
            let r = api.call("GET", &format!("/audit?n={}", cmd.lines.unwrap_or(50)), None)?;
            if cmd.json {
                return json_out(out, &r);
            }
            let rows: Vec<Vec<String>> = elements(&r.body)
                .iter()
                .map(|e| vec![field(e, "time"), format!("{} ({})", field(e, "user"), field(e, "role")), format!("{} {}", field(e, "method"), field(e, "path").trim_start_matches(API_PREFIX)), field(e, "status"), field(e, "address")])
                .collect();
            if rows.is_empty() {
                return writeln!(out, "no changes recorded").map_err(io);
            }
            table(out, &["TIME", "USER", "CHANGE", "STATUS", "FROM"], &rows).map_err(io)
        }
        _ => Err(usage(format!("unknown command: {}", words.join(" ")))),
    }
}

fn print_status(out: &mut dyn Write, body: &str) -> std::io::Result<()> {
    let uptime = field(body, "uptime_secs").parse().map(duration).unwrap_or_default();
    let ready = if field(body, "ready") == "true" { "ready" } else { "not ready" };
    writeln!(out, "rws {}  pid {}  up {uptime}  {ready}", field(body, "version"), field(body, "pid"))?;
    let listeners = member(body, "listeners");
    let main = member(&listeners, "main");
    writeln!(out, "main      {}{}", field(&main, "address"), if field(&main, "tls") == "true" { " (TLS)" } else { "" })?;
    let redirect = field(&listeners, "redirect");
    if redirect != "null" && !redirect.is_empty() {
        writeln!(out, "redirect  :{redirect}")?;
    }
    writeln!(out, "admin     {}", field(&listeners, "admin"))?;
    let features: Vec<String> = elements(&member(body, "features")).iter().map(|f| text(f)).collect();
    writeln!(out, "features  {}", features.join(" "))?;
    let cluster = member(body, "cluster");
    if !cluster.is_empty() && cluster.trim() != "null" {
        let nodes: Vec<String> = elements(&member(&cluster, "nodes")).iter().map(|n| text(n)).collect();
        let pending = field(&cluster, "pending");
        let pending = if pending == "0" || pending.is_empty() { String::new() } else { format!(", {pending} change(s) waiting") };
        writeln!(out, "cluster   node {} · Redis {} · {} node(s): {}{pending}", field(&cluster, "node"), field(&cluster, "redis"), nodes.len(), nodes.join(", "))?;
    }
    let fleet = member(body, "fleet");
    if !fleet.is_empty() && fleet.trim() != "null" {
        let config = field(&fleet, "config");
        let error = field(&fleet, "error");
        let state = if error != "null" && !error.is_empty() { format!("refused: {error}") } else { format!("config {}", config.get(..12).unwrap_or(&config)) };
        writeln!(out, "fleet     {} · {state}", field(&fleet, "source"))?;
    }
    for d in elements(&member(body, "disk")) {
        let free = field(&d, "free_bytes").parse().map(bytes).unwrap_or_default();
        let total = field(&d, "total_bytes").parse().map(bytes).unwrap_or_default();
        let low = if field(&d, "low") == "true" { "  LOW" } else { "" };
        writeln!(out, "disk      {}  {free} free of {total}{low}", field(&d, "dir"))?;
    }
    Ok(())
}

fn print_certificates(out: &mut dyn Write, body: &str) -> std::io::Result<()> {
    let rows: Vec<Vec<String>> = elements(&member(body, "certificates"))
        .iter()
        .map(|c| vec![field(c, "name"), field(c, "expires"), field(c, "days_left"), field(c, "source")])
        .collect();
    if rows.is_empty() {
        writeln!(out, "no certificates")?;
    } else {
        table(out, &["NAME", "EXPIRES", "DAYS LEFT", "SOURCE"], &rows)?;
    }
    let acme = member(body, "acme");
    if field(&acme, "running") == "true" {
        let last = member(&acme, "last_check");
        if last.trim() == "null" || last.is_empty() {
            writeln!(out, "ACME renewal: running, not checked yet")?;
        } else {
            writeln!(out, "ACME renewal: running, last check: {}", field(&last, "outcome"))?;
        }
    } else {
        writeln!(out, "ACME renewal: off")?;
    }
    Ok(())
}

fn print_traffic(out: &mut dyn Write, body: &str) -> std::io::Result<()> {
    let hours = field(body, "hours");
    let host = field(body, "host");
    let scope = if host == "null" || host.is_empty() { String::new() } else { format!(" for {host}") };
    writeln!(out, "last {hours} h{scope}")?;
    let totals = member(body, "totals");
    let rows: Vec<Vec<String>> = [("people", "people"), ("ai", "AI agents"), ("bots", "bots")]
        .iter()
        .map(|(key, label)| {
            let g = member(&totals, key);
            vec![label.to_string(), field(&g, "requests"), field(&g, "pages"), field(&g, "visitors")]
        })
        .collect();
    table(out, &["", "REQUESTS", "PAGES", "VISITORS"], &rows)?;
    let pages = elements(&member(body, "people_pages"));
    if !pages.is_empty() {
        writeln!(out, "\ntop pages")?;
        for pair in pages.iter().take(10) {
            let pair = elements(pair);
            if let [page, n] = pair.as_slice() {
                writeln!(out, "  {:>8}  {}", n.trim(), text(page))?;
            }
        }
    }
    Ok(())
}

/// `logs -f`: the SSE stream's `data:` lines until the server goes away.
fn follow_logs(api: &Api, out: &mut dyn Write) -> Result<(), Failure> {
    let mut reader = api.connect("GET", "/logs/stream", None)?;
    let (status, chunked) = read_head(&mut reader).map_err(|e| unusable(e.to_string()))?;
    if status == 401 || status == 403 {
        return Err(unusable(format!("the admin API refused the token ({status})")));
    }
    // Keep-alive comments arrive every 15 s; wait for them, not 30 s.
    let _ = reader.get_ref().set_read_timeout(Some(Duration::from_secs(60)));
    let body: Box<dyn BufRead> = if chunked { Box::new(BufReader::new(Dechunk::new(reader))) } else { Box::new(reader) };
    if status != 200 {
        let mut text = String::new();
        let mut body = body;
        let _ = body.read_to_string(&mut text);
        return Err(failed(api_error(&Reply { status, body: text })));
    }
    for line in body.lines() {
        let line = line.map_err(|e| failed(format!("log stream ended: {e}")))?;
        // Pings are events with empty data; comments start with ':'.
        if let Some(data) = line.strip_prefix("data:") {
            let data = data.strip_prefix(' ').unwrap_or(data);
            if !data.is_empty() {
                writeln!(out, "{data}").map_err(|e| failed(e.to_string()))?;
                let _ = out.flush();
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
