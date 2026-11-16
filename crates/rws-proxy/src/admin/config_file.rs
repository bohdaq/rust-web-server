//! The configuration file through the admin API (18.38.0): read it, check a
//! candidate, write it and reload — what the admin UI's Config page stages
//! and applies.
//!
//! Off unless `[admin] edit_config = true`: whoever can write the file can
//! add a route serving any directory rws can read, so it takes that setting
//! and the `admin` role (see [`super::access::required_role`]). The file's
//! text is not served either while it is off.
//!
//! A write goes: the caller's `sha256` still names the file on disk (nobody
//! changed it meanwhile) → the new text passes [`crate::config_check`] → the
//! previous text is kept beside it as `<file>.bak` → the file is replaced
//! (a temporary file renamed over it) → rws reloads. A reload that reports
//! errors puts the previous file back.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::config_check::ConfigReport;
use crate::core::json::escape as json_escape;

/// `[admin] edit_config`.
pub const RWS_CONFIG_ADMIN_EDIT_CONFIG: &str = "RWS_CONFIG_ADMIN_EDIT_CONFIG";

/// A configuration larger than this isn't taken.
pub(crate) const MAX_BYTES: usize = 1024 * 1024;

/// `[admin] edit_config = true`.
pub(crate) fn enabled() -> bool {
    std::env::var(RWS_CONFIG_ADMIN_EDIT_CONFIG).is_ok_and(|v| v.trim().eq_ignore_ascii_case("true"))
}

const OFF: &str = "editing the configuration file is off: set [admin] edit_config = true";

/// The file rws loaded, or why there's none to edit.
fn target() -> Result<PathBuf, String> {
    #[cfg(all(feature = "http2", not(target_arch = "wasm32")))]
    if crate::fleet::status_json().is_some() {
        return Err("[fleet] replaces this file from its source; change the configuration there".to_string());
    }
    let Some(source) = crate::entry_point::config_source() else {
        return Err("rws runs without a configuration file (defaults, environment and flags only)".to_string());
    };
    std::fs::canonicalize(&source).map_err(|e| format!("{source}: {e}"))
}

/// The file and its bytes, when rws may also write it.
fn open(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    std::fs::OpenOptions::new().write(true).open(path).map_err(|e| format!("rws cannot write {}: {e}", path.display()))?;
    Ok(bytes)
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    crate::core::sha256::sha256(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn quoted(value: &str) -> String {
    format!("\"{}\"", json_escape(value))
}

fn strings(values: &[String]) -> String {
    values.iter().map(|v| quoted(v)).collect::<Vec<_>>().join(",")
}

/// `GET /config/file`:
/// `{"enabled":…,"path":…,"editable":…,"reason":…,"content":…,"sha256":…,"modified":…}`.
/// The text only when it can be edited here; otherwise `reason` says why not.
pub(crate) fn read_json() -> String {
    let unavailable = |path: Option<String>, reason: &str| {
        format!(
            r#"{{"enabled":{},"path":{},"editable":false,"reason":{},"content":null,"sha256":null,"modified":null}}"#,
            enabled(),
            path.map_or("null".to_string(), |p| quoted(&p)),
            quoted(reason)
        )
    };
    if !enabled() {
        return unavailable(crate::entry_point::config_source(), OFF);
    }
    let path = match target() {
        Ok(path) => path,
        Err(reason) => return unavailable(crate::entry_point::config_source(), &reason),
    };
    match read_at(&path) {
        Ok(body) => body,
        Err(reason) => unavailable(Some(path.display().to_string()), &reason),
    }
}

/// [`read_json`] for the file at `path`.
fn read_at(path: &Path) -> Result<String, String> {
    let shown = path.display().to_string();
    let content = String::from_utf8(open(path)?).map_err(|_| "the file isn't UTF-8 text".to_string())?;
    let modified = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or("null".to_string(), |d| d.as_secs().to_string());
    Ok(format!(
        r#"{{"enabled":true,"path":{},"editable":true,"reason":null,"content":{},"sha256":"{}","modified":{modified}}}"#,
        quoted(&shown),
        quoted(&content),
        sha256_hex(content.as_bytes())
    ))
}

// ── What a reload can't apply ────────────────────────────────────────────────

/// Settings read when the process starts: a reload re-reads them from the
/// file, but they do nothing until a restart.
const AT_START: &[&str] = &[
    "RWS_CONFIG_IP",
    "RWS_CONFIG_PORT",
    "RWS_CONFIG_THREAD_COUNT",
    "RWS_CONFIG_TLS_CERT_FILE",
    "RWS_CONFIG_TLS_KEY_FILE",
    "RWS_CONFIG_HTTP_REDIRECT_PORT",
    "RWS_CONFIG_WATCH",
];

/// The same, by table: the admin listener, the fleet puller and the cluster
/// are set up once.
const AT_START_TABLES: &[&str] = &["RWS_CONFIG_ADMIN_", "RWS_CONFIG_FLEET_", "RWS_CONFIG_CLUSTER_"];

/// Sections that bind sockets of their own.
const AT_START_SECTIONS: &[&str] = &["tcp_proxy", "udp_proxy", "ws_proxy"];

/// `[admin]` settings read on each request, so a reload does apply them.
const PER_REQUEST: &[&str] = &[RWS_CONFIG_ADMIN_EDIT_CONFIG, "RWS_CONFIG_ADMIN_COUNT_OWN_REQUESTS"];

fn at_start(name: &str) -> bool {
    !PER_REQUEST.contains(&name) && (AT_START.contains(&name) || AT_START_TABLES.iter().any(|t| name.starts_with(t)))
}

/// `RWS_CONFIG_CORS_ALLOW_ALL` as the file can spell it: `cors_allow_all`.
fn key(name: &str) -> String {
    name.strip_prefix("RWS_CONFIG_").unwrap_or(name).to_ascii_lowercase()
}

fn settings(text: &str) -> Vec<(String, String)> {
    crate::entry_point::config_file::collect_settings(std::io::Cursor::new(text.as_bytes()), String::new()).map(|s| s.settings).unwrap_or_default()
}

/// The lines of every `[[name]]` block, without comments and blank lines.
fn section(text: &str, name: &str) -> Vec<String> {
    let header = format!("[[{name}]]");
    let mut inside = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        let line = line.split_once('#').map_or(line, |(code, _)| code).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            // `[tcp_proxy.tls]` and the like belong to the block.
            inside = line == header || (inside && line.starts_with(&format!("[{name}.")));
        }
        if inside {
            lines.push(line.to_string());
        }
    }
    lines
}

/// What differs between `old` and `new` that a reload doesn't apply: a
/// setting read at startup, a setting taken out of the file (a reload
/// leaves its last value in place), a section with listeners of its own.
pub(crate) fn needs_restart(old: &str, new: &str) -> Vec<String> {
    let (before, after) = (settings(old), settings(new));
    let value = |list: &[(String, String)], name: &str| list.iter().rev().find(|(n, _)| n == name).map(|(_, v)| v.clone());
    let mut names: Vec<&String> = before.iter().chain(after.iter()).map(|(n, _)| n).collect();
    names.sort();
    names.dedup();
    let mut out = Vec::new();
    for name in names {
        let (was, now) = (value(&before, name), value(&after, name));
        if was == now {
            continue;
        }
        if at_start(name) {
            out.push(key(name));
        } else if now.is_none() {
            out.push(format!("{} (removed: it keeps its value until a restart)", key(name)));
        }
    }
    for name in AT_START_SECTIONS {
        if section(old, name) != section(new, name) {
            out.push(format!("[[{name}]]"));
        }
    }
    out
}

/// `{"ok":…,"errors":[…],"warnings":[…],"restart":[…]}`: `rws --check` on
/// `content`, and what in it would wait for a restart.
pub(crate) fn check_json(content: &str) -> Result<String, Refusal> {
    if !enabled() {
        return Err(Refusal::Off(OFF.to_string()));
    }
    check_at(&target().map_err(Refusal::Unavailable)?, content)
}

/// [`check_json`] against the file at `path`.
fn check_at(path: &Path, content: &str) -> Result<String, Refusal> {
    if content.len() > MAX_BYTES {
        return Err(Refusal::TooLarge);
    }
    let current = std::fs::read_to_string(path).unwrap_or_default();
    let report = crate::config_check::check_str(content);
    Ok(format!(
        r#"{{"ok":{},"errors":[{}],"warnings":[{}],"restart":[{}]}}"#,
        report.is_ok(),
        strings(&report.errors),
        strings(&report.warnings),
        strings(&needs_restart(&current, content))
    ))
}

// ── Writing ──────────────────────────────────────────────────────────────────

/// How the reload after a write went.
pub(crate) enum Reload {
    Done,
    /// It reported these errors: the running configuration stayed.
    Refused(String),
    /// It didn't finish in time.
    Unknown(String),
}

/// Why a write (or a check) didn't happen.
#[derive(Debug)]
pub(crate) enum Refusal {
    /// `[admin] edit_config` is off (`403`).
    Off(String),
    /// No file to edit: none loaded, or the fleet owns it (`409`).
    Unavailable(String),
    /// The file on disk isn't the one the edit started from (`409`).
    Stale(String),
    /// Over [`MAX_BYTES`] (`413`).
    TooLarge,
    /// The text doesn't pass `rws --check` (`422`).
    Invalid(ConfigReport),
    /// Reading or writing failed, or the reload refused the file and the
    /// previous one is back (`500`).
    Failed(String),
    /// Written, but the reload didn't answer (`504`).
    Unknown(String),
}

impl Refusal {
    /// The HTTP status and the JSON body.
    pub(crate) fn answer(&self) -> (u16, String) {
        let error = |message: &str| format!(r#"{{"error":{}}}"#, quoted(message));
        match self {
            Refusal::Off(m) => (403, error(m)),
            Refusal::Unavailable(m) | Refusal::Stale(m) => (409, error(m)),
            Refusal::TooLarge => (413, error(&format!("a configuration file over {} KB isn't taken", MAX_BYTES / 1024))),
            Refusal::Invalid(report) => (
                422,
                format!(
                    r#"{{"error":"the configuration doesn't pass rws --check; nothing was written","errors":[{}],"warnings":[{}]}}"#,
                    strings(&report.errors),
                    strings(&report.warnings)
                ),
            ),
            Refusal::Failed(m) => (500, error(m)),
            Refusal::Unknown(m) => (504, error(m)),
        }
    }
}

/// A finished write.
#[derive(Debug, PartialEq)]
pub(crate) struct Written {
    pub path: String,
    pub sha256: String,
    /// `false`: the text was already the file's.
    pub changed: bool,
    /// Where the previous text is, when it could be kept.
    pub backup: Option<String>,
    pub restart: Vec<String>,
    pub warnings: Vec<String>,
}

impl Written {
    /// `{"applied":true,"changed":…,"path":…,"sha256":…,"backup":…,"restart":[…],"warnings":[…]}`.
    pub(crate) fn to_json(&self) -> String {
        format!(
            r#"{{"applied":true,"changed":{},"path":{},"sha256":"{}","backup":{},"restart":[{}],"warnings":[{}]}}"#,
            self.changed,
            quoted(&self.path),
            self.sha256,
            self.backup.as_deref().map_or("null".to_string(), quoted),
            strings(&self.restart),
            strings(&self.warnings)
        )
    }
}

/// `path` with `suffix` after its file name.
fn beside(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(suffix);
    path.with_file_name(name)
}

/// Replace `path`'s contents: a temporary file with the same mode and
/// owner renamed over it, so a reader sees the old text or the new, never
/// half of one. Where that can't be done (no room for a file beside it, or
/// the owner can't be kept) the file is written in place.
fn replace(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = beside(path, ".rws-admin.tmp");
    let staged = (|| {
        let meta = std::fs::metadata(path)?;
        // Never write through whatever a crashed write (or anyone else)
        // left under the temporary name.
        let _ = std::fs::remove_file(&tmp);
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&tmp)?;
        file.write_all(bytes)?;
        file.set_permissions(meta.permissions())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let mine = file.metadata()?;
            if (mine.uid(), mine.gid()) != (meta.uid(), meta.gid()) {
                std::os::unix::fs::fchown(&file, Some(meta.uid()), Some(meta.gid()))?;
            }
        }
        file.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if staged.is_ok() {
        return Ok(());
    }
    let _ = std::fs::remove_file(&tmp);
    let mut file = std::fs::OpenOptions::new().write(true).truncate(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Keep `previous` as `<file>.bak`, readable by whoever could read the file.
fn back_up(path: &Path, previous: &[u8]) -> std::io::Result<PathBuf> {
    let backup = beside(path, ".bak");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&backup)?;
    file.write_all(previous)?;
    if let Ok(meta) = std::fs::metadata(path) {
        file.set_permissions(meta.permissions())?;
    }
    file.sync_all()?;
    Ok(backup)
}

/// One write at a time.
static WRITING: Mutex<()> = Mutex::new(());

/// `PUT /config/file`: make `content` the configuration file and reload.
/// `based_on` is the `sha256` of the text the edit started from.
pub(crate) fn write(content: &str, based_on: &str, reload: &dyn Fn() -> Reload) -> Result<Written, Refusal> {
    if !enabled() {
        return Err(Refusal::Off(OFF.to_string()));
    }
    write_at(&target().map_err(Refusal::Unavailable)?, content, based_on, reload)
}

/// [`write`] to the file at `path`.
fn write_at(path: &Path, content: &str, based_on: &str, reload: &dyn Fn() -> Reload) -> Result<Written, Refusal> {
    if content.len() > MAX_BYTES {
        return Err(Refusal::TooLarge);
    }
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let previous = open(path).map_err(Refusal::Failed)?;
    let shown = path.display().to_string();
    if !based_on.eq_ignore_ascii_case(&sha256_hex(&previous)) {
        return Err(Refusal::Stale(format!("{shown} changed on disk since it was read; read it again and redo the change")));
    }
    let report = crate::config_check::check_str(content);
    if !report.is_ok() {
        return Err(Refusal::Invalid(report));
    }
    let restart = needs_restart(&String::from_utf8_lossy(&previous), content);
    let sha256 = sha256_hex(content.as_bytes());
    if content.as_bytes() == previous.as_slice() {
        return Ok(Written { path: shown, sha256, changed: false, backup: None, restart, warnings: report.warnings });
    }
    let backup = back_up(path, &previous).ok().map(|b| b.display().to_string());
    replace(path, content.as_bytes()).map_err(|e| Refusal::Failed(format!("writing {shown}: {e}")))?;
    match reload() {
        Reload::Done => Ok(Written { path: shown, sha256, changed: true, backup, restart, warnings: report.warnings }),
        Reload::Refused(errors) => {
            // Put the previous file back, so a restart doesn't load the
            // configuration the running server refused.
            match replace(path, &previous) {
                Ok(()) => {
                    let _ = reload();
                    Err(Refusal::Failed(format!("the reload refused it ({errors}); the previous file is back")))
                }
                Err(e) => Err(Refusal::Failed(format!("the reload refused it ({errors}), and putting the previous file back failed: {e}"))),
            }
        }
        Reload::Unknown(why) => Err(Refusal::Unknown(format!("{shown} is written, but {why}"))),
    }
}

// ── The request body ─────────────────────────────────────────────────────────

/// The top-level string `key` of a JSON object, decoded strictly: every
/// escape JSON has (`\b`, `\f`, surrogate pairs), and an error for anything
/// else — the text goes to disk, so nothing in it may be guessed.
pub(crate) fn body_text(body: &[u8], key: &str) -> Result<String, String> {
    let body = std::str::from_utf8(body).map_err(|_| "the body isn't UTF-8".to_string())?;
    let members = crate::mcp::json_rpc::object_members(body).ok_or("the body isn't a JSON object")?;
    let raw = members.into_iter().find(|(k, _)| k == key).map(|(_, v)| v).ok_or(format!("the body has no \"{key}\""))?;
    let raw = raw.trim();
    let inner = raw.strip_prefix('"').and_then(|r| r.strip_suffix('"')).ok_or(format!("\"{key}\" isn't a string"))?;
    let bad = || format!("\"{key}\" isn't a valid JSON string");
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    let hex4 = |chars: &mut std::str::Chars| -> Option<u32> {
        let digits: String = chars.take(4).collect();
        (digits.len() == 4).then(|| u32::from_str_radix(&digits, 16).ok()).flatten()
    };
    while let Some(c) = chars.next() {
        match c {
            '"' => return Err(bad()),
            '\\' => match chars.next().ok_or_else(bad)? {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                '/' => out.push('/'),
                'b' => out.push('\u{8}'),
                'f' => out.push('\u{c}'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                'u' => {
                    let first = hex4(&mut chars).ok_or_else(bad)?;
                    let code = if (0xD800..0xDC00).contains(&first) {
                        if chars.next() != Some('\\') || chars.next() != Some('u') {
                            return Err(bad());
                        }
                        let second = hex4(&mut chars).filter(|s| (0xDC00..0xE000).contains(s)).ok_or_else(bad)?;
                        0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
                    } else {
                        first
                    };
                    out.push(char::from_u32(code).ok_or_else(bad)?);
                }
                _ => return Err(bad()),
            },
            c if (c as u32) < 0x20 => return Err(bad()),
            c => out.push(c),
        }
    }
    Ok(out)
}

/// What the audit log keeps of a write: not the text (it can hold
/// secrets), its hash and size.
pub(crate) fn audit_detail(body: &[u8]) -> String {
    let based_on = body_text(body, "sha256").unwrap_or_default();
    match body_text(body, "content") {
        Ok(content) => format!(r#"{{"based_on":"{}","sha256":"{}","bytes":{}}}"#, json_escape(&based_on), sha256_hex(content.as_bytes()), content.len()),
        Err(_) => r#"{"content":"(unreadable)"}"#.to_string(),
    }
}

#[cfg(test)]
mod tests;
