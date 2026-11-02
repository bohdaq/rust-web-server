//! Local HTTPS (18.21.0, #71): rws's own certificate authority.
//!
//! With `tls = "internal"` (`RWS_CONFIG_TLS`) rws keeps a CA (EC P-256, ten
//! years) in [`dir`] and serves a certificate it issues itself for
//! `localhost`, `127.0.0.1`, `::1`, the listen address, the
//! virtual hosts' domains and `tls_internal_names`. The certificate lives
//! [`LEAF_LIFETIME`] and is re-issued at half-life, then hot-swapped through
//! the reload path. `rws trust` puts the CA in the system trust store once,
//! and browsers trust `https://localhost` without a warning.
//!
//! Meant for development and private networks; ACME wins when both are set.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use rcgen::{BasicConstraints, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair, KeyUsagePurpose, SanType};

/// `RWS_CONFIG_TLS`: `internal` turns local HTTPS on.
pub const RWS_CONFIG_TLS: &str = "RWS_CONFIG_TLS";
/// Where the CA and the issued certificate live (default [`default_dir`]).
pub const RWS_CONFIG_TLS_INTERNAL_DIR: &str = "RWS_CONFIG_TLS_INTERNAL_DIR";
/// More names (comma-separated) the issued certificate covers.
pub const RWS_CONFIG_TLS_INTERNAL_NAMES: &str = "RWS_CONFIG_TLS_INTERNAL_NAMES";

/// How long an issued certificate is valid.
pub const LEAF_LIFETIME: Duration = Duration::from_secs(7 * 24 * 3600);
const CA_LIFETIME: Duration = Duration::from_secs(10 * 365 * 24 * 3600);

/// Whether `tls = "internal"`.
pub fn enabled() -> bool {
    std::env::var(RWS_CONFIG_TLS).is_ok_and(|v| v.trim().eq_ignore_ascii_case("internal"))
}

/// The CA directory: `RWS_CONFIG_TLS_INTERNAL_DIR`, else [`default_dir`].
pub fn dir() -> PathBuf {
    match std::env::var(RWS_CONFIG_TLS_INTERNAL_DIR) {
        Ok(d) if !d.trim().is_empty() => PathBuf::from(d.trim()),
        _ => default_dir(),
    }
}

/// The per-user data directory, so `rws trust` finds the CA from any
/// working directory: `$XDG_DATA_HOME/rws/pki` or `~/.local/share/rws/pki`
/// (Linux), `~/Library/Application Support/rws/pki` (macOS),
/// `%APPDATA%\rws\pki` (Windows); `.rws/pki` without a home.
pub fn default_dir() -> PathBuf {
    let env = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty()).map(PathBuf::from);
    let base = if cfg!(windows) {
        env("APPDATA")
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Application Support"))
    } else {
        env("XDG_DATA_HOME").or_else(|| env("HOME").map(|h| h.join(".local/share")))
    };
    base.map(|b| b.join("rws").join("pki")).unwrap_or_else(|| PathBuf::from(".rws/pki"))
}

/// rws's local certificate authority.
pub struct LocalCa {
    dir: PathBuf,
    cert_pem: String,
    issuer: Issuer<'static, KeyPair>,
}

impl LocalCa {
    /// The CA in `dir`, created on first use (`ca.pem`, `ca.key` 0600,
    /// `ca.name`: its subject, so the issuer is rebuilt without parsing).
    pub fn load_or_create(dir: &Path) -> Result<LocalCa, String> {
        let (cert, key, name) = (dir.join("ca.pem"), dir.join("ca.key"), dir.join("ca.name"));
        if cert.is_file() && key.is_file() && name.is_file() {
            let read = |p: &Path| std::fs::read_to_string(p).map_err(|e| format!("can't read {}: {e}", p.display()));
            let key_pair = KeyPair::from_pem(&read(&key)?).map_err(|e| format!("{}: {e}", key.display()))?;
            let subject = read(&name)?.trim().to_string();
            return Ok(LocalCa { dir: dir.to_path_buf(), cert_pem: read(&cert)?, issuer: Issuer::new(ca_params(&subject), key_pair) });
        }
        std::fs::create_dir_all(dir).map_err(|e| format!("can't create {}: {e}", dir.display()))?;
        let subject = format!("rws local CA {}@{} {}", user(), host(), crate::http::date::format(now_secs()).get(5..16).unwrap_or("").trim());
        let key_pair = KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).map_err(|e| e.to_string())?;
        let mut params = ca_params(&subject);
        let now = SystemTime::now();
        params.not_before = (now - Duration::from_secs(3600)).into();
        params.not_after = (now + CA_LIFETIME).into();
        let ca = params.self_signed(&key_pair).map_err(|e| e.to_string())?;
        write_private(&key, &key_pair.serialize_pem())?;
        std::fs::write(&name, &subject).map_err(|e| format!("can't write {}: {e}", name.display()))?;
        std::fs::write(&cert, ca.pem()).map_err(|e| format!("can't write {}: {e}", cert.display()))?;
        println!("local HTTPS: created a certificate authority in {} — run `rws trust` once so browsers trust it", dir.display());
        Ok(LocalCa { dir: dir.to_path_buf(), cert_pem: ca.pem(), issuer: Issuer::new(ca_params(&subject), key_pair) })
    }

    /// The CA certificate (PEM).
    pub fn cert_pem(&self) -> &str {
        &self.cert_pem
    }

    /// Path of the CA certificate.
    pub fn cert_path(&self) -> PathBuf {
        self.dir.join("ca.pem")
    }

    /// A server certificate for `names` (DNS names or IP addresses), valid
    /// for `lifetime`: (certificate chain PEM — leaf then CA, key PEM).
    pub fn issue(&self, names: &[String], lifetime: Duration) -> Result<(String, String), String> {
        let mut params = CertificateParams::default();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, names.first().cloned().unwrap_or_else(|| "localhost".to_string()));
        params.distinguished_name = dn;
        params.subject_alt_names = names
            .iter()
            .map(|n| match n.parse::<std::net::IpAddr>() {
                Ok(ip) => Ok(SanType::IpAddress(ip)),
                Err(_) if valid_dns_name(n) => n.clone().try_into().map(SanType::DnsName).map_err(|e| format!("{n:?}: {e}")),
                Err(_) => Err(format!("{n:?} is not a host name or IP address")),
            })
            .collect::<Result<_, _>>()?;
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        params.use_authority_key_identifier_extension = true;
        let now = SystemTime::now();
        params.not_before = (now - Duration::from_secs(300)).into();
        params.not_after = (now + lifetime).into();
        let key_pair = KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).map_err(|e| e.to_string())?;
        let leaf = params.signed_by(&key_pair, &self.issuer).map_err(|e| e.to_string())?;
        Ok((format!("{}{}", leaf.pem(), self.cert_pem), key_pair.serialize_pem()))
    }
}

/// A host name, optionally `*.` + one with at least two labels (browsers
/// refuse `*.localhost`-style wildcards).
fn valid_dns_name(name: &str) -> bool {
    let (wildcard, rest) = match name.strip_prefix("*.") {
        Some(rest) => (true, rest),
        None => (false, name),
    };
    let labels: Vec<&str> = rest.split('.').collect();
    !rest.is_empty()
        && rest.len() <= 253
        && (!wildcard || labels.len() >= 2)
        && labels.iter().all(|l| !l.is_empty() && l.len() <= 63 && !l.starts_with('-') && !l.ends_with('-') && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
}

fn ca_params(subject: &str) -> CertificateParams {
    let mut params = CertificateParams::default();
    let mut dn = DistinguishedName::new();
    dn.push(DnType::OrganizationName, "rws local development");
    dn.push(DnType::CommonName, subject);
    params.distinguished_name = dn;
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign, KeyUsagePurpose::DigitalSignature];
    params
}

fn write_private(path: &Path, contents: &str) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut file = options.open(&tmp).map_err(|e| format!("can't write {}: {e}", tmp.display()))?;
        std::io::Write::write_all(&mut file, contents.as_bytes()).map_err(|e| format!("can't write {}: {e}", tmp.display()))?;
    }
    std::fs::rename(&tmp, path).map_err(|e| format!("can't write {}: {e}", path.display()))
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn user() -> String {
    ["USER", "USERNAME", "LOGNAME"].iter().find_map(|v| std::env::var(v).ok().filter(|u| !u.is_empty())).unwrap_or_else(|| "user".to_string())
}

fn host() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|h| !h.is_empty())
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok().map(|h| h.trim().to_string()).filter(|h| !h.is_empty()))
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .unwrap_or_else(|| "localhost".to_string())
}

/// The names the issued certificate covers: the loopback names, the listen
/// address (unless a wildcard), the virtual hosts' domains and
/// `tls_internal_names`, without duplicates.
pub fn names() -> Vec<String> {
    let mut names: Vec<String> = ["localhost", "127.0.0.1", "::1"].iter().map(|s| s.to_string()).collect();
    let mut add = |n: &str| {
        let n = n.trim().trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase();
        if !n.is_empty() && n != "0.0.0.0" && n != "::" && !names.contains(&n) {
            names.push(n);
        }
    };
    add(&std::env::var(crate::entry_point::Config::RWS_CONFIG_IP).unwrap_or_default());
    for vhost in crate::entry_point::get_virtual_hosts() {
        add(&vhost.domain);
    }
    for n in std::env::var(RWS_CONFIG_TLS_INTERNAL_NAMES).unwrap_or_default().split(',') {
        add(n);
    }
    names
}

/// Paths of the certificate being served.
pub fn leaf_paths(dir: &Path) -> (PathBuf, PathBuf) {
    (dir.join("localhost.pem"), dir.join("localhost.key"))
}

/// Issue a certificate for [`names`] into [`leaf_paths`].
fn issue_leaf(ca: &LocalCa, dir: &Path) -> Result<(PathBuf, PathBuf), String> {
    let (cert_path, key_path) = leaf_paths(dir);
    let (chain, key) = ca.issue(&names(), LEAF_LIFETIME)?;
    // Key first: a reload between the two writes then fails on a mismatch
    // and keeps the old pair, instead of serving a key for another cert.
    write_private(&key_path, &key)?;
    let tmp = cert_path.with_extension("tmp");
    std::fs::write(&tmp, chain).and_then(|_| std::fs::rename(&tmp, &cert_path)).map_err(|e| format!("can't write {}: {e}", cert_path.display()))?;
    Ok((cert_path, key_path))
}

/// The binary's entry point: with `tls = "internal"` and no ACME domains,
/// issue a certificate, point `tls_cert_file`/`tls_key_file` at it (when
/// they are empty) and renew it at half-life on a thread. Errors are
/// logged; the server then starts without TLS as it would otherwise.
pub fn start_from_env() {
    if !enabled() {
        return;
    }
    let configured = |name: &str| std::env::var(name).is_ok_and(|v| !v.trim().is_empty());
    if configured(crate::entry_point::Config::RWS_CONFIG_ACME_DOMAINS) {
        eprintln!("local HTTPS: acme_domains is set, so ACME provides the certificate (tls = \"internal\" ignored)");
        return;
    }
    if configured(crate::entry_point::Config::RWS_CONFIG_TLS_CERT_FILE) || configured(crate::entry_point::Config::RWS_CONFIG_TLS_KEY_FILE) {
        eprintln!("local HTTPS: tls_cert_file/tls_key_file are set, so they are served (tls = \"internal\" ignored)");
        return;
    }
    let dir = dir();
    let ca = match LocalCa::load_or_create(&dir) {
        Ok(ca) => ca,
        Err(e) => return eprintln!("local HTTPS: {e}"),
    };
    let (cert, key) = match issue_leaf(&ca, &dir) {
        Ok(paths) => paths,
        Err(e) => return eprintln!("local HTTPS: {e}"),
    };
    std::env::set_var(crate::entry_point::Config::RWS_CONFIG_TLS_CERT_FILE, &cert);
    std::env::set_var(crate::entry_point::Config::RWS_CONFIG_TLS_KEY_FILE, &key);
    crate::server_config::invalidate();
    println!("local HTTPS: serving a certificate from rws's own CA ({}) for {}", ca.cert_path().display(), names().join(", "));
    let _ = std::thread::Builder::new().name("rws-tls-internal".into()).spawn(move || loop {
        std::thread::sleep(LEAF_LIFETIME / 2);
        match issue_leaf(&ca, &dir) {
            // `watch = true` notices the new files itself.
            Ok(_) if !crate::config_watch::enabled() => crate::config_reload::request_reload(),
            Ok(_) => {}
            Err(e) => eprintln!("local HTTPS: renewal failed: {e}"),
        }
    });
}

// ── rws trust / rws untrust ──────────────────────────────────────────────────

/// `rws trust [--print]` / `rws untrust`: `Some(exit code)` when `args`
/// (a full argv) asks for one, `None` otherwise.
pub fn trust_command(args: &[String]) -> Option<i32> {
    let verb = args.get(1).map(String::as_str)?;
    if verb != "trust" && verb != "untrust" {
        return None;
    }
    let rest: Vec<&str> = args[2..].iter().map(String::as_str).collect();
    let dir = dir();
    let ca = match LocalCa::load_or_create(&dir) {
        Ok(ca) => ca,
        Err(e) => {
            eprintln!("rws {verb}: {e}");
            return Some(1);
        }
    };
    if rest.contains(&"--print") {
        print!("{}", ca.cert_pem());
        return Some(0);
    }
    if let Some(unknown) = rest.iter().find(|a| **a != "--print") {
        eprintln!("rws {verb}: unknown argument {unknown}\nUsage: rws trust [--print] | rws untrust");
        return Some(2);
    }
    let steps = trust_steps(verb == "trust", &ca.cert_path());
    if steps.is_empty() {
        eprintln!("rws {verb}: no known trust store on this system; add {} by hand", ca.cert_path().display());
        return Some(1);
    }
    let mut ok = true;
    for step in steps {
        println!("{}", step.describe);
        let status = std::process::Command::new(&step.program).args(&step.args).status();
        match status {
            Ok(s) if s.success() => {}
            Ok(s) if step.optional => println!("  (skipped: exit {s})"),
            Err(e) if step.optional => println!("  (skipped: {e})"),
            Ok(s) => {
                eprintln!("  failed: {} exited with {s}", step.program);
                ok = false;
            }
            Err(e) => {
                eprintln!("  failed: can't run {}: {e}", step.program);
                ok = false;
            }
        }
    }
    if ok {
        println!("{} {}", if verb == "trust" { "trusted" } else { "removed" }, ca.cert_path().display());
    }
    Some(if ok { 0 } else { 1 })
}

/// One command `rws trust` runs.
#[derive(Debug, PartialEq)]
pub struct TrustStep {
    pub describe: String,
    pub program: String,
    pub args: Vec<String>,
    /// A failure here (a browser store that isn't set up) isn't fatal.
    pub optional: bool,
}

fn step(describe: impl Into<String>, program: &str, args: &[&str], optional: bool) -> TrustStep {
    TrustStep { describe: describe.into(), program: program.to_string(), args: args.iter().map(|a| a.to_string()).collect(), optional }
}

const CA_NICKNAME: &str = "rws local CA";

/// What `rws trust` (or `untrust` when `add` is false) runs on this system.
pub fn trust_steps(add: bool, ca: &Path) -> Vec<TrustStep> {
    let ca_s = ca.to_string_lossy().to_string();
    let mut steps = Vec::new();
    #[cfg(windows)]
    {
        if add {
            steps.push(step("adding the CA to the current user's Root store", "certutil", &["-user", "-addstore", "-f", "Root", &ca_s], false));
        } else {
            steps.push(step("removing the CA from the current user's Root store", "certutil", &["-user", "-delstore", "Root", CA_NICKNAME], false));
        }
    }
    #[cfg(target_os = "macos")]
    {
        if add {
            steps.push(step("adding the CA to the System keychain (asks for your password)", "sudo", &["security", "add-trusted-cert", "-d", "-r", "trustRoot", "-k", "/Library/Keychains/System.keychain", &ca_s], false));
        } else {
            steps.push(step("removing the CA from the System keychain", "sudo", &["security", "remove-trusted-cert", "-d", &ca_s], false));
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let root = unsafe { libc::geteuid() } == 0;
        let sudo = |describe: String, program: &str, args: &[&str]| {
            if root {
                step(describe, program, args, false)
            } else {
                let mut all = vec![program];
                all.extend_from_slice(args);
                step(format!("{describe} (asks for your password)"), "sudo", &all, false)
            }
        };
        let stores: [(&str, &str, &[&str]); 3] = [
            ("/usr/local/share/ca-certificates", "update-ca-certificates", &[]),
            ("/etc/pki/ca-trust/source/anchors", "update-ca-trust", &["extract"]),
            ("/etc/ca-certificates/trust-source/anchors", "trust", &["extract-compat"]),
        ];
        if let Some((anchor_dir, update, update_args)) = stores.iter().find(|(d, _, _)| Path::new(d).is_dir()) {
            let target = format!("{anchor_dir}/rws-local-ca.crt");
            if add {
                steps.push(sudo(format!("copying the CA to {target}"), "install", &["-m", "0644", &ca_s, &target]));
            } else {
                steps.push(sudo(format!("removing {target}"), "rm", &["-f", &target]));
            }
            steps.push(sudo(format!("running {update}"), update, update_args));
        }
        // Chrome and Firefox on Linux read the NSS database.
        if let Ok(home) = std::env::var("HOME") {
            let nssdb = format!("{home}/.pki/nssdb");
            if Path::new(&nssdb).is_dir() {
                let db = format!("sql:{nssdb}");
                if add {
                    steps.push(step("adding the CA to the browsers' NSS database", "certutil", &["-d", &db, "-A", "-t", "C,,", "-n", CA_NICKNAME, "-i", &ca_s], true));
                } else {
                    steps.push(step("removing the CA from the browsers' NSS database", "certutil", &["-d", &db, "-D", "-n", CA_NICKNAME], true));
                }
            }
        }
    }
    let _ = (add, &ca_s);
    steps
}

#[cfg(test)]
#[path = "internal_tests.rs"]
mod tests;
