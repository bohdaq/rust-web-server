//! Fleet config pull (18.23.0, #80): many instances, one source of truth.
//!
//! With `[fleet] source` set, rws fetches its config file from an HTTPS
//! URL, an S3 object or a git repository every `interval_secs`, checks its
//! Ed25519 signature (`<source>.sig`) against `public_key`, validates it
//! like `rws --check`, writes it over the local config file and reloads.
//! A config that fails any step is refused and the running one stays;
//! `report_url` gets each outcome. `{node}` in the source is the node name
//! (`[cluster] node`, else the host name), so each instance can have its
//! own file.
//!
//! The fleet config is the declared layer; runtime changes made through
//! the admin API ([`crate::cluster`]) are the live one.
//!
//! `rws fleet keygen` makes a key pair; `rws fleet sign <file> --key <key>`
//! writes `<file>.sig`.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crate::core::base64::Base64;
use crate::mcp::json_escape;

pub const RWS_CONFIG_FLEET_SOURCE: &str = "RWS_CONFIG_FLEET_SOURCE";
pub const RWS_CONFIG_FLEET_INTERVAL_SECS: &str = "RWS_CONFIG_FLEET_INTERVAL_SECS";
pub const RWS_CONFIG_FLEET_PUBLIC_KEY: &str = "RWS_CONFIG_FLEET_PUBLIC_KEY";
pub const RWS_CONFIG_FLEET_INSECURE: &str = "RWS_CONFIG_FLEET_INSECURE";
pub const RWS_CONFIG_FLEET_REPORT_URL: &str = "RWS_CONFIG_FLEET_REPORT_URL";
pub const RWS_CONFIG_FLEET_S3_ENDPOINT: &str = "RWS_CONFIG_FLEET_S3_ENDPOINT";

/// Where the config comes from.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Http(String),
    S3 { bucket: String, key: String },
    Git { repo: String, branch: String, path: String },
}

impl Source {
    /// `https://…`, `s3://bucket/key`, `git+https://host/repo.git#branch:path`
    /// (`git+ssh://`, `git+file://` too); `{node}` replaced.
    pub fn parse(text: &str, node: &str) -> Result<Source, String> {
        let text = text.trim().replace("{node}", node);
        if text.starts_with("https://") || text.starts_with("http://") {
            return Ok(Source::Http(text));
        }
        if let Some(rest) = text.strip_prefix("s3://") {
            let (bucket, key) = rest.split_once('/').ok_or_else(|| format!("[fleet] source {text:?}: expected s3://bucket/key"))?;
            if bucket.is_empty() || key.is_empty() {
                return Err(format!("[fleet] source {text:?}: expected s3://bucket/key"));
            }
            return Ok(Source::S3 { bucket: bucket.to_string(), key: key.to_string() });
        }
        if let Some(rest) = text.strip_prefix("git+") {
            let (repo, fragment) = rest.split_once('#').ok_or_else(|| format!("[fleet] source {text:?}: expected git+<url>#<branch>:<path>"))?;
            let (branch, path) = fragment.split_once(':').ok_or_else(|| format!("[fleet] source {text:?}: expected git+<url>#<branch>:<path>"))?;
            if branch.is_empty() || path.is_empty() || branch.starts_with('-') {
                return Err(format!("[fleet] source {text:?}: expected git+<url>#<branch>:<path>"));
            }
            return Ok(Source::Git { repo: repo.to_string(), branch: branch.to_string(), path: path.trim_start_matches('/').to_string() });
        }
        Err(format!("[fleet] source {text:?}: expected https://…, s3://bucket/key or git+<url>#<branch>:<path>"))
    }
}

/// `[fleet]` settings.
#[derive(Debug, Clone)]
pub struct FleetConfig {
    pub source: Source,
    pub interval: Duration,
    /// Ed25519 public key (32 bytes); `None` only with `insecure = true`.
    pub public_key: Option<Vec<u8>>,
    pub report_url: String,
    pub node: String,
    /// The local config file written and reloaded.
    pub config_path: PathBuf,
    /// Where a git source is cloned.
    pub state_dir: PathBuf,
    pub s3_endpoint: String,
}

/// A public key: base64 of the 32 raw bytes, inline or in a file.
pub fn read_public_key(value: &str) -> Result<Vec<u8>, String> {
    let value = value.trim();
    let text = if value.len() != 44 && std::path::Path::new(value).is_file() {
        std::fs::read_to_string(value).map_err(|e| format!("[fleet] public_key {value}: {e}"))?
    } else {
        value.to_string()
    };
    let key = Base64::decode(text.trim().to_string()).map_err(|e| format!("[fleet] public_key: not base64: {e}"))?;
    if key.len() != 32 {
        return Err(format!("[fleet] public_key: an Ed25519 public key is 32 bytes, this one is {}", key.len()));
    }
    Ok(key)
}

impl FleetConfig {
    /// `None` without `[fleet] source`.
    pub fn from_env() -> Option<Result<FleetConfig, String>> {
        let env = |name: &str| std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let source = env(RWS_CONFIG_FLEET_SOURCE)?;
        Some((|| {
            let node = crate::admin::access::node_name();
            let insecure = env(RWS_CONFIG_FLEET_INSECURE).is_some_and(|v| v.eq_ignore_ascii_case("true"));
            let public_key = match env(RWS_CONFIG_FLEET_PUBLIC_KEY) {
                Some(k) => Some(read_public_key(&k)?),
                None if insecure => None,
                None => return Err("[fleet] needs public_key (or insecure = true to skip signatures)".to_string()),
            };
            Ok(FleetConfig {
                source: Source::parse(&source, &node)?,
                interval: Duration::from_secs(env(RWS_CONFIG_FLEET_INTERVAL_SECS).and_then(|v| v.parse().ok()).unwrap_or(30).max(5)),
                public_key,
                report_url: env(RWS_CONFIG_FLEET_REPORT_URL).unwrap_or_default(),
                node,
                config_path: PathBuf::from(env("RWS_CONFIG_FILE").unwrap_or_else(|| "rws.config.toml".to_string())),
                state_dir: PathBuf::from(".rws/fleet"),
                s3_endpoint: env(RWS_CONFIG_FLEET_S3_ENDPOINT).unwrap_or_default(),
            })
        })())
    }
}

/// What a fetch brought.
#[derive(Debug, PartialEq)]
pub enum Fetched {
    /// The source says nothing changed (HTTP 304).
    NotModified,
    Config { body: Vec<u8>, signature: Option<Vec<u8>> },
}

/// A signature file: base64, or the 64 raw bytes.
fn signature_bytes(raw: &[u8]) -> Result<Vec<u8>, String> {
    if raw.len() == 64 {
        return Ok(raw.to_vec());
    }
    let sig = Base64::decode(String::from_utf8_lossy(raw).trim().to_string()).map_err(|e| format!("the signature isn't base64: {e}"))?;
    if sig.len() != 64 {
        return Err(format!("an Ed25519 signature is 64 bytes, this one is {}", sig.len()));
    }
    Ok(sig)
}

/// Check `body` against `signature` with `public_key`.
pub fn verify(body: &[u8], signature: &[u8], public_key: &[u8]) -> Result<(), String> {
    let sig = signature_bytes(signature)?;
    aws_lc_rs::signature::UnparsedPublicKey::new(&aws_lc_rs::signature::ED25519, public_key)
        .verify(body, &sig)
        .map_err(|_| "the signature doesn't match (wrong key, or the file was changed after signing)".to_string())
}

fn sha256_hex(bytes: &[u8]) -> String {
    crate::core::sha256::sha256(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// The puller: fetch, verify, check, write, reload, report.
pub struct Fleet {
    config: FleetConfig,
    state: Mutex<FleetState>,
}

#[derive(Debug, Default, Clone)]
struct FleetState {
    etag: Option<String>,
    applied: Option<String>,
    last_check: u64,
    last_error: Option<String>,
}

/// One round's outcome.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Unchanged,
    Applied(String),
    Refused(String),
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Fleet {
    pub fn new(config: FleetConfig) -> Fleet {
        let applied = std::fs::read(&config.config_path).ok().map(|b| sha256_hex(&b));
        Fleet { config, state: Mutex::new(FleetState { applied, ..FleetState::default() }) }
    }

    fn fetch(&self) -> Result<Fetched, String> {
        let want_sig = self.config.public_key.is_some();
        match &self.config.source {
            Source::Http(url) => {
                let etag = self.state.lock().unwrap_or_else(|e| e.into_inner()).etag.clone();
                let client = crate::http_client::Client::new().timeout_ms(15_000);
                let mut request = client.get(url);
                if let Some(tag) = &etag {
                    request = request.header("If-None-Match", tag);
                }
                let response = request.send().map_err(|e| format!("fetching {url}: {e}"))?;
                if response.status() == 304 {
                    return Ok(Fetched::NotModified);
                }
                if !response.is_success() {
                    return Err(format!("fetching {url}: HTTP {}", response.status()));
                }
                let new_etag = response.header("ETag").map(str::to_string);
                let body = response.bytes().to_vec();
                let signature = if want_sig {
                    let sig = client.get(&format!("{url}.sig")).send().map_err(|e| format!("fetching {url}.sig: {e}"))?;
                    if !sig.is_success() {
                        return Err(format!("fetching {url}.sig: HTTP {}", sig.status()));
                    }
                    Some(sig.bytes().to_vec())
                } else {
                    None
                };
                self.state.lock().unwrap_or_else(|e| e.into_inner()).etag = new_etag;
                Ok(Fetched::Config { body, signature })
            }
            #[cfg(feature = "storage-s3")]
            Source::S3 { bucket, key } => {
                use crate::storage::Storage;
                let region = std::env::var("AWS_REGION").or_else(|_| std::env::var("AWS_DEFAULT_REGION")).unwrap_or_else(|_| "us-east-1".to_string());
                let endpoint = if self.config.s3_endpoint.is_empty() { format!("https://s3.{region}.amazonaws.com") } else { self.config.s3_endpoint.clone() };
                let s3 = crate::storage::S3Storage::new(crate::storage::S3Config {
                    bucket: bucket.clone(),
                    region,
                    access_key: std::env::var("AWS_ACCESS_KEY_ID").unwrap_or_default(),
                    secret_key: std::env::var("AWS_SECRET_ACCESS_KEY").unwrap_or_default(),
                    endpoint,
                });
                let body = s3.get(key).map_err(|e| format!("s3://{bucket}/{key}: {e}"))?;
                let signature = if want_sig { Some(s3.get(&format!("{key}.sig")).map_err(|e| format!("s3://{bucket}/{key}.sig: {e}"))?) } else { None };
                Ok(Fetched::Config { body, signature })
            }
            #[cfg(not(feature = "storage-s3"))]
            Source::S3 { .. } => Err("s3:// sources need the storage-s3 feature, which this build doesn't include".to_string()),
            Source::Git { repo, branch, path } => {
                let dir = self.config.state_dir.join("git");
                let git = |args: &[&str]| -> Result<(), String> {
                    let out = std::process::Command::new("git").args(args).output().map_err(|e| format!("running git: {e} (is git installed?)"))?;
                    if out.status.success() {
                        Ok(())
                    } else {
                        Err(format!("git {}: {}", args.first().unwrap_or(&""), String::from_utf8_lossy(&out.stderr).trim()))
                    }
                };
                let d = dir.to_string_lossy().to_string();
                if !dir.join(".git").is_dir() {
                    std::fs::create_dir_all(&self.config.state_dir).map_err(|e| e.to_string())?;
                    git(&["clone", "--quiet", "--depth", "1", "--branch", branch, "--", repo, &d])?;
                } else {
                    git(&["-C", &d, "fetch", "--quiet", "--depth", "1", "origin", branch])?;
                    git(&["-C", &d, "reset", "--quiet", "--hard", "FETCH_HEAD"])?;
                }
                let file = dir.join(path);
                if !file.canonicalize().is_ok_and(|f| f.starts_with(dir.canonicalize().unwrap_or_default())) {
                    return Err(format!("{path} isn't in the repository"));
                }
                let body = std::fs::read(&file).map_err(|e| format!("{path}: {e}"))?;
                let signature = if want_sig { Some(std::fs::read(dir.join(format!("{path}.sig"))).map_err(|e| format!("{path}.sig: {e}"))?) } else { None };
                Ok(Fetched::Config { body, signature })
            }
        }
    }

    /// One round.
    pub fn tick(&self) -> Outcome {
        let outcome = match self.fetch() {
            Ok(Fetched::NotModified) => Outcome::Unchanged,
            Ok(Fetched::Config { body, signature }) => self.apply(body, signature),
            Err(e) => Outcome::Refused(e),
        };
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.last_check = unix_now();
            match &outcome {
                Outcome::Refused(e) => {
                    if state.last_error.as_deref() != Some(e) {
                        eprintln!("[fleet] {e}; the running config stays");
                    }
                    state.last_error = Some(e.clone());
                }
                Outcome::Applied(hash) => {
                    println!("[fleet] applied config {}", &hash[..12]);
                    state.last_error = None;
                }
                Outcome::Unchanged => state.last_error = None,
            }
        }
        if !matches!(outcome, Outcome::Unchanged) {
            self.report(&outcome);
        }
        outcome
    }

    fn apply(&self, body: Vec<u8>, signature: Option<Vec<u8>>) -> Outcome {
        if let Some(key) = &self.config.public_key {
            let Some(sig) = signature else { return Outcome::Refused("no signature".into()) };
            if let Err(e) = verify(&body, &sig, key) {
                return Outcome::Refused(e);
            }
        }
        let hash = sha256_hex(&body);
        if self.state.lock().unwrap_or_else(|e| e.into_inner()).applied.as_deref() == Some(hash.as_str()) {
            return Outcome::Unchanged;
        }
        let Ok(text) = String::from_utf8(body.clone()) else { return Outcome::Refused("the config isn't UTF-8".into()) };
        let report = crate::config_check::check_str(&text);
        if !report.is_ok() {
            return Outcome::Refused(format!("the fetched config doesn't pass rws --check: {}", report.errors.join("; ")));
        }
        let path = &self.config.config_path;
        let tmp = path.with_extension("fleet.tmp");
        if let Err(e) = std::fs::write(&tmp, &body).and_then(|_| std::fs::rename(&tmp, path)) {
            return Outcome::Refused(format!("writing {}: {e}", path.display()));
        }
        self.state.lock().unwrap_or_else(|e| e.into_inner()).applied = Some(hash.clone());
        if crate::metrics::SERVER_READY.load(std::sync::atomic::Ordering::SeqCst) {
            let (before, _) = crate::config_reload::last_reload();
            crate::config_reload::request_reload();
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            while std::time::Instant::now() < deadline {
                let (count, errors) = crate::config_reload::last_reload();
                if count > before {
                    if !errors.is_empty() {
                        return Outcome::Refused(format!("the reload reported: {}", errors.join("; ")));
                    }
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        Outcome::Applied(hash)
    }

    fn report(&self, outcome: &Outcome) {
        if self.config.report_url.is_empty() {
            return;
        }
        let (applied, detail) = match outcome {
            Outcome::Applied(hash) => (true, format!(r#""config":"{hash}""#)),
            Outcome::Refused(e) => (false, format!(r#""error":"{}""#, json_escape(e))),
            Outcome::Unchanged => return,
        };
        let body = format!(r#"{{"node":"{}","rws":"{}","applied":{applied},{detail},"at":{}}}"#, json_escape(&self.config.node), env!("CARGO_PKG_VERSION"), unix_now());
        if let Err(e) = crate::http_client::Client::new().timeout_ms(10_000).post(&self.config.report_url).body_json(&body).send() {
            eprintln!("[fleet] report to {}: {e}", self.config.report_url);
        }
    }

    /// `{"source":…,"node":…,"config":…,"last_check":…,"error":…}`.
    pub fn status_json(&self) -> String {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let source = match &self.config.source {
            Source::Http(u) => u.clone(),
            Source::S3 { bucket, key } => format!("s3://{bucket}/{key}"),
            Source::Git { repo, branch, path } => format!("git+{repo}#{branch}:{path}"),
        };
        let opt = |v: &Option<String>| v.as_ref().map_or("null".to_string(), |s| format!("\"{}\"", json_escape(s)));
        format!(
            r#"{{"source":"{}","node":"{}","config":{},"last_check":{},"error":{}}}"#,
            json_escape(&source),
            json_escape(&self.config.node),
            opt(&state.applied),
            state.last_check,
            opt(&state.last_error)
        )
    }
}

fn global() -> &'static OnceLock<Fleet> {
    static FLEET: OnceLock<Fleet> = OnceLock::new();
    &FLEET
}

/// The fleet status for the admin API, or `None` without `[fleet]`.
pub fn status_json() -> Option<String> {
    global().get().map(Fleet::status_json)
}

/// The binary's entry point: with `[fleet] source`, pull on a thread.
pub fn start_from_env() {
    let config = match FleetConfig::from_env() {
        None => return,
        Some(Ok(c)) => c,
        Some(Err(e)) => return eprintln!("[fleet] not started: {e}"),
    };
    let interval = config.interval;
    if global().set(Fleet::new(config)).is_err() {
        return;
    }
    println!("[fleet] pulling the config every {}s", interval.as_secs());
    let _ = std::thread::Builder::new().name("rws-fleet".into()).spawn(move || loop {
        if let Some(fleet) = global().get() {
            fleet.tick();
        }
        std::thread::sleep(interval);
    });
}

// ── rws fleet keygen / sign ──────────────────────────────────────────────────

/// `rws fleet keygen [<prefix>]` / `rws fleet sign <file> --key <file>`:
/// `Some(exit code)` when `args` (a full argv) asks for one.
pub fn command(args: &[String]) -> Option<i32> {
    if args.get(1).map(String::as_str) != Some("fleet") {
        return None;
    }
    let rest: Vec<&str> = args[2..].iter().map(String::as_str).collect();
    let usage = "Usage: rws fleet keygen [<prefix>]   (writes <prefix>.key and <prefix>.pub; default fleet)\n       rws fleet sign <file> --key <prefix>.key   (writes <file>.sig)";
    let result = match rest.as_slice() {
        ["keygen"] => keygen("fleet"),
        ["keygen", prefix] => keygen(prefix),
        ["sign", file, "--key", key] | ["sign", "--key", key, file] => sign(file, key),
        _ => {
            eprintln!("{usage}");
            return Some(2);
        }
    };
    Some(match result {
        Ok(message) => {
            println!("{message}");
            0
        }
        Err(e) => {
            eprintln!("rws fleet: {e}");
            1
        }
    })
}

fn keygen(prefix: &str) -> Result<String, String> {
    use aws_lc_rs::signature::KeyPair;
    let rng = aws_lc_rs::rand::SystemRandom::new();
    let pkcs8 = aws_lc_rs::signature::Ed25519KeyPair::generate_pkcs8(&rng).map_err(|_| "key generation failed".to_string())?;
    let pair = aws_lc_rs::signature::Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).map_err(|_| "key generation failed".to_string())?;
    let private = Base64::encode(pkcs8.as_ref())?;
    let public = Base64::encode(pair.public_key().as_ref())?;
    let key_path = format!("{prefix}.key");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&key_path).map_err(|e| format!("{key_path}: {e}"))?;
    std::io::Write::write_all(&mut file, format!("{private}\n").as_bytes()).map_err(|e| e.to_string())?;
    std::fs::write(format!("{prefix}.pub"), format!("{public}\n")).map_err(|e| format!("{prefix}.pub: {e}"))?;
    Ok(format!("wrote {key_path} (keep it secret) and {prefix}.pub\npublic_key = \"{public}\""))
}

fn sign(file: &str, key: &str) -> Result<String, String> {
    let body = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
    let pkcs8 = Base64::decode(std::fs::read_to_string(key).map_err(|e| format!("{key}: {e}"))?.trim().to_string()).map_err(|e| format!("{key}: not a key from rws fleet keygen: {e}"))?;
    let pair = aws_lc_rs::signature::Ed25519KeyPair::from_pkcs8(&pkcs8).map_err(|_| format!("{key}: not a key from rws fleet keygen"))?;
    let report = crate::config_check::check_str_offline(&String::from_utf8_lossy(&body));
    if !report.is_ok() {
        return Err(format!("{file} doesn't pass rws --check: {}", report.errors.join("; ")));
    }
    let signature = Base64::encode(pair.sign(&body).as_ref())?;
    std::fs::write(format!("{file}.sig"), format!("{signature}\n")).map_err(|e| format!("{file}.sig: {e}"))?;
    Ok(format!("wrote {file}.sig"))
}

#[cfg(test)]
mod tests;
