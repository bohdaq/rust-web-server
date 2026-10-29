/// Automatic TLS certificate management via ACME (RFC 8555) / Let's Encrypt.
///
/// # Quick start
///
/// Set environment variables and call [`AcmeManager::provision_if_needed`] at
/// startup before starting the TLS server:
///
/// ```bash
/// RWS_CONFIG_ACME_DOMAINS=example.com,www.example.com
/// RWS_CONFIG_ACME_EMAIL=admin@example.com
/// ```
///
/// ```rust,no_run
/// use rust_web_server::acme::{AcmeConfig, AcmeManager};
///
/// # async fn run() {
/// if let Some(cfg) = AcmeConfig::from_env() {
///     let mgr = AcmeManager::new(cfg);
///     mgr.provision_if_needed().await.unwrap();
///     // TLS cert is now at the paths in RWS_CONFIG_TLS_CERT_FILE / KEY_FILE.
///     // Spawn renewal in background:
///     tokio::spawn(mgr.run_renewal_loop());
/// }
/// # }
/// ```
///
/// # Environment variables
///
/// | Variable | Default | Description |
/// |---|---|---|
/// | `RWS_CONFIG_ACME_DOMAINS` | — | Comma-separated domain list (required to activate ACME) |
/// | `RWS_CONFIG_ACME_EMAIL` | — | Contact email sent to the CA |
/// | `RWS_CONFIG_ACME_STAGING` | `false` | `true` = Let's Encrypt staging (for testing) |
/// | `RWS_CONFIG_ACME_DIRECTORY` | LE production URL | Custom ACME directory URL |
/// | `RWS_CONFIG_ACME_CERT_PATH` | `RWS_CONFIG_TLS_CERT_FILE` | Where to write the certificate chain |
/// | `RWS_CONFIG_ACME_KEY_PATH` | `RWS_CONFIG_TLS_KEY_FILE` | Where to write the certificate private key |
/// | `RWS_CONFIG_ACME_CHALLENGE_PORT` | `80` | Port for the HTTP-01 challenge server |
/// | `RWS_CONFIG_ACME_RENEW_BEFORE_DAYS` | — | Renew this many days before expiry instead of at two-thirds of the lifetime |
/// | `RWS_CONFIG_ACME_PROFILE` | — | Certificate profile to order (`classic`, `tlsserver`, `shortlived`, … as the CA offers) |
/// | `RWS_CONFIG_ACME_ACCOUNT_KEY_PATH` | `acme_account.key` | Persist the ACME account key |
/// | `RWS_CONFIG_ACME_REDIS` | — | Redis `host:port` shared by replicas (see [`shared`]) |
/// | `RWS_CONFIG_ACME_REDIS_PASSWORD` | — | Its password |
/// | `RWS_CONFIG_ACME_REDIS_PREFIX` | `rws:acme` | Key prefix in that Redis |
/// | `RWS_CONFIG_ACME_ON_DEMAND` | `false` | Order certificates at the first handshake (see [`on_demand`]) |
/// | `RWS_CONFIG_ACME_ON_DEMAND_ASK` | — | URL asked `?domain=<name>` before each such order; 2xx allows it |

mod crypto;
mod client;
pub(crate) mod dns01;
pub mod dns_api;
pub mod on_demand;
pub(crate) mod shared;
pub mod tls_alpn;

#[cfg(test)]
mod tests;

use crate::entry_point::Config;
use client::AcmeHttpClient;
use crypto::AccountKey;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Parsers of CA-supplied input, for `fuzz_api`.
#[cfg(fuzzing)]
pub(crate) mod fuzzing {
    pub(crate) use super::crypto::cert_info_from_der;
    pub(crate) use super::{json_object_keys, parse_rfc3339, parse_suggested_window};
}

// ── Config ────────────────────────────────────────────────────────────────────

/// ACME provisioning configuration.
#[derive(Clone, Debug)]
pub struct AcmeConfig {
    /// DNS names to include in the certificate (first entry is CN).
    pub domains: Vec<String>,
    /// Contact email address for the CA.
    pub email: String,
    /// ACME directory URL. Defaults to Let's Encrypt production.
    pub directory_url: String,
    /// Path to write the provisioned certificate chain (PEM).
    pub cert_path: String,
    /// Path to write the certificate's private key (PEM).
    pub key_path: String,
    /// Port for the temporary HTTP-01 challenge server (default 80).
    pub challenge_port: u16,
    /// Renew this many days before expiry. `None` (the default): at
    /// two-thirds of the certificate's lifetime, or earlier when the CA's
    /// renewal information (RFC 9773) says so.
    pub renew_before_days: Option<i64>,
    /// Certificate profile to order (draft-ietf-acme-profiles), e.g.
    /// `shortlived`; `None` lets the CA choose.
    pub profile: Option<String>,
    /// Path to persist the ACME account key between restarts.
    pub account_key_path: String,
    /// How control of the domains is proven (since 18.4.0; `Http01` before).
    pub challenge: ChallengeType,
    /// DNS settings; `Some` exactly when `challenge` is `Dns01` or
    /// `DnsPersist01` (then only where to look the record up).
    pub dns: Option<DnsConfig>,
}

/// The ACME challenge rws answers (RFC 8555 §8, RFC 8737).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChallengeType {
    /// A file at `http://<domain>/.well-known/acme-challenge/` (port 80).
    Http01,
    /// A special certificate on the HTTPS port, for the `acme-tls/1` ALPN
    /// protocol (RFC 8737): no port 80 needed.
    TlsAlpn01,
    /// A TXT record at `_acme-challenge.<domain>`: the only way to get a
    /// wildcard certificate; no inbound port needed.
    Dns01,
    /// A TXT record at `_validation-persist.<domain>` naming the CA and the
    /// ACME account, published once by you (draft-ietf-acme-dns-persist):
    /// wildcards too, and no DNS credentials on the server. rws checks
    /// that it's there and prints it when it isn't.
    DnsPersist01,
}

impl ChallengeType {
    /// `http-01`, `tls-alpn-01`, `dns-01` or `dns-persist-01` (any case).
    pub fn parse(value: &str) -> Option<ChallengeType> {
        match value.trim().to_ascii_lowercase().as_str() {
            "http-01" => Some(ChallengeType::Http01),
            "tls-alpn-01" => Some(ChallengeType::TlsAlpn01),
            "dns-01" => Some(ChallengeType::Dns01),
            "dns-persist-01" => Some(ChallengeType::DnsPersist01),
            _ => None,
        }
    }

    /// The challenge's name in ACME (`"http-01"`, ...).
    pub fn as_str(&self) -> &'static str {
        match self {
            ChallengeType::Http01 => "http-01",
            ChallengeType::TlsAlpn01 => "tls-alpn-01",
            ChallengeType::Dns01 => "dns-01",
            ChallengeType::DnsPersist01 => "dns-persist-01",
        }
    }

    /// The configured challenge (`RWS_CONFIG_ACME_CHALLENGE`), `Http01` when
    /// unset or unknown (`rws --check` reports an unknown one).
    pub fn configured() -> ChallengeType {
        non_empty_env(Config::RWS_CONFIG_ACME_CHALLENGE).and_then(|v| ChallengeType::parse(&v)).unwrap_or(ChallengeType::Http01)
    }
}

/// DNS-01 settings.
#[derive(Clone, Debug)]
pub struct DnsConfig {
    /// Who adds and removes the TXT records.
    pub provider: DnsProvider,
    /// The zone holding them; `None`: found by an SOA lookup.
    pub zone: Option<String>,
    /// How long to wait for a record to be visible before telling the CA.
    pub propagation_secs: u64,
}

/// How DNS-01 records are published.
#[derive(Clone, Debug)]
pub enum DnsProvider {
    /// A DNS UPDATE (RFC 2136) to the zone's primary server, signed with a
    /// TSIG key (RFC 8945) when one is given — BIND, Knot, PowerDNS, ...
    Rfc2136 { server: String, tsig: Option<Tsig> },
    /// A program, run as `<program> present <fqdn> <value>` and
    /// `<program> cleanup <fqdn> <value>`; exit status 0 = done. `server`
    /// (`dns_server`, optional) is where to look for the record; the system
    /// resolver without it.
    Exec { program: String, server: Option<String> },
    /// Cloudflare's API with an API token; the zone by id or looked up.
    /// `server` as for `Exec`.
    Cloudflare { token: dns_api::Secret, api: String, zone_id: Option<String>, server: Option<String> },
    /// Amazon Route 53 with the standard AWS credentials; the hosted zone by
    /// id or looked up. `server` as for `Exec`.
    Route53 { credentials: dns_api::AwsCredentials, api: String, zone_id: Option<String>, server: Option<String> },
    /// Nobody: records are published by you (`dns-persist-01`). `server`
    /// is where rws looks for them; the system resolver without it.
    Manual { server: Option<String> },
}

/// A TSIG key: its name, algorithm and secret.
#[derive(Clone)]
pub struct Tsig {
    pub key_name: String,
    /// `hmac-sha256` or `hmac-sha512`.
    pub algorithm: String,
    pub secret: Vec<u8>,
}

impl std::fmt::Debug for Tsig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tsig").field("key_name", &self.key_name).field("algorithm", &self.algorithm).field("secret", &"(hidden)").finish()
    }
}

/// A base64 TSIG secret, decoded; `None` if it isn't base64 (or is empty).
pub(crate) fn base64_secret(value: &str) -> Option<Vec<u8>> {
    crypto::base64_decode_std(value.trim()).filter(|s| !s.is_empty())
}

impl DnsConfig {
    /// From `RWS_CONFIG_ACME_DNS_*`; an error names what's missing or wrong.
    pub fn from_env() -> Result<DnsConfig, String> {
        let provider = match non_empty_env(Config::RWS_CONFIG_ACME_DNS_PROVIDER).map(|p| p.to_ascii_lowercase()).as_deref() {
            Some("rfc2136") => {
                let server = non_empty_env(Config::RWS_CONFIG_ACME_DNS_SERVER)
                    .ok_or("DNS-01 with rfc2136 needs RWS_CONFIG_ACME_DNS_SERVER (the primary server, host[:port])")?;
                let tsig = match (non_empty_env(Config::RWS_CONFIG_ACME_DNS_TSIG_KEY), non_empty_env(Config::RWS_CONFIG_ACME_DNS_TSIG_SECRET)) {
                    (None, None) => None,
                    (Some(key_name), Some(secret)) => {
                        let secret = base64_secret(&secret).ok_or("RWS_CONFIG_ACME_DNS_TSIG_SECRET is not base64")?;
                        let algorithm = non_empty_env(Config::RWS_CONFIG_ACME_DNS_TSIG_ALGORITHM)
                            .unwrap_or_else(|| "hmac-sha256".to_string())
                            .to_ascii_lowercase();
                        if !matches!(algorithm.as_str(), "hmac-sha256" | "hmac-sha512") {
                            return Err(format!("RWS_CONFIG_ACME_DNS_TSIG_ALGORITHM {algorithm:?}: expected hmac-sha256 or hmac-sha512"));
                        }
                        Some(Tsig { key_name, algorithm, secret })
                    }
                    _ => return Err("RWS_CONFIG_ACME_DNS_TSIG_KEY and RWS_CONFIG_ACME_DNS_TSIG_SECRET go together".to_string()),
                };
                DnsProvider::Rfc2136 { server, tsig }
            }
            Some("exec") => DnsProvider::Exec {
                program: non_empty_env(Config::RWS_CONFIG_ACME_DNS_EXEC).ok_or("DNS-01 with exec needs RWS_CONFIG_ACME_DNS_EXEC (the program)")?,
                server: non_empty_env(Config::RWS_CONFIG_ACME_DNS_SERVER),
            },
            Some("cloudflare") => DnsProvider::Cloudflare {
                token: dns_api::Secret(
                    non_empty_env(Config::RWS_CONFIG_ACME_DNS_API_TOKEN).ok_or("DNS-01 with cloudflare needs RWS_CONFIG_ACME_DNS_API_TOKEN (an API token with DNS edit permission)")?,
                ),
                api: non_empty_env(Config::RWS_CONFIG_ACME_DNS_API_URL).unwrap_or_else(|| dns_api::CLOUDFLARE_API.to_string()).trim_end_matches('/').to_string(),
                zone_id: non_empty_env(Config::RWS_CONFIG_ACME_DNS_ZONE_ID),
                server: non_empty_env(Config::RWS_CONFIG_ACME_DNS_SERVER),
            },
            Some("route53") => DnsProvider::Route53 {
                credentials: dns_api::AwsCredentials::from_env().ok_or("DNS-01 with route53 needs AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY")?,
                api: non_empty_env(Config::RWS_CONFIG_ACME_DNS_API_URL).unwrap_or_else(|| dns_api::ROUTE53_API.to_string()).trim_end_matches('/').to_string(),
                zone_id: non_empty_env(Config::RWS_CONFIG_ACME_DNS_ZONE_ID),
                server: non_empty_env(Config::RWS_CONFIG_ACME_DNS_SERVER),
            },
            Some(other) => return Err(format!("RWS_CONFIG_ACME_DNS_PROVIDER {other:?}: expected rfc2136, cloudflare, route53 or exec")),
            None => return Err("DNS-01 needs RWS_CONFIG_ACME_DNS_PROVIDER (rfc2136, cloudflare, route53 or exec)".to_string()),
        };
        let zone = non_empty_env(Config::RWS_CONFIG_ACME_DNS_ZONE);
        let propagation_secs = non_empty_env(Config::RWS_CONFIG_ACME_DNS_PROPAGATION_SECS).and_then(|v| v.parse().ok()).unwrap_or(120);
        Ok(DnsConfig { provider, zone, propagation_secs })
    }
}

impl AcmeConfig {
    /// Let's Encrypt production ACME directory.
    pub const LETSENCRYPT: &'static str =
        "https://acme-v02.api.letsencrypt.org/directory";
    /// Let's Encrypt staging ACME directory (no real certificates; for testing).
    pub const LETSENCRYPT_STAGING: &'static str =
        "https://acme-staging-v02.api.letsencrypt.org/directory";

    /// Build config from environment variables. Returns `None` when
    /// `RWS_CONFIG_ACME_DOMAINS` is not set or empty.
    pub fn from_env() -> Option<Self> {
        let domains_str = std::env::var(Config::RWS_CONFIG_ACME_DOMAINS).ok()?;
        if domains_str.trim().is_empty() { return None; }

        let domains: Vec<String> = domains_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if domains.is_empty() { return None; }

        // Startup sets unset RWS_CONFIG_* variables to their (often empty)
        // defaults, so an empty value must mean "not configured" here.
        let email = non_empty_env(Config::RWS_CONFIG_ACME_EMAIL).unwrap_or_default();
        let staging = non_empty_env(Config::RWS_CONFIG_ACME_STAGING)
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let directory_url = if staging {
            Self::LETSENCRYPT_STAGING.to_string()
        } else {
            non_empty_env(Config::RWS_CONFIG_ACME_DIRECTORY)
                .unwrap_or_else(|| Self::LETSENCRYPT.to_string())
        };

        // Unset paths default into STATE_DIR, a dot-directory the static
        // server never serves — not the working directory (the web root),
        // where they were downloadable before 17.113.1 (#45).
        let cert_path = non_empty_env(Config::RWS_CONFIG_ACME_CERT_PATH)
            .or_else(|| non_empty_env(Config::RWS_CONFIG_TLS_CERT_FILE))
            .unwrap_or_else(|| DEFAULT_CERT_PATH.to_string());
        let key_path = non_empty_env(Config::RWS_CONFIG_ACME_KEY_PATH)
            .or_else(|| non_empty_env(Config::RWS_CONFIG_TLS_KEY_FILE))
            .unwrap_or_else(|| DEFAULT_KEY_PATH.to_string());

        let challenge_port = non_empty_env(Config::RWS_CONFIG_ACME_CHALLENGE_PORT)
            .and_then(|v| v.parse().ok())
            .unwrap_or(80u16);

        let renew_before_days = non_empty_env(Config::RWS_CONFIG_ACME_RENEW_BEFORE_DAYS)
            .and_then(|v| v.parse().ok());
        let profile = non_empty_env(Config::RWS_CONFIG_ACME_PROFILE);

        let account_key_path = non_empty_env(Config::RWS_CONFIG_ACME_ACCOUNT_KEY_PATH)
            .unwrap_or_else(|| DEFAULT_ACCOUNT_KEY_PATH.to_string());

        let challenge = ChallengeType::configured();
        let dns = match challenge {
            ChallengeType::Dns01 => match DnsConfig::from_env() {
                Ok(dns) => Some(dns),
                Err(e) => {
                    eprintln!("[ACME] {e}; certificates can't be issued until it's fixed.");
                    return None;
                }
            },
            // You publish the record; rws only looks for it.
            ChallengeType::DnsPersist01 => Some(DnsConfig {
                provider: DnsProvider::Manual { server: non_empty_env(Config::RWS_CONFIG_ACME_DNS_SERVER) },
                zone: None,
                propagation_secs: 0,
            }),
            _ => None,
        };

        Some(AcmeConfig {
            domains,
            email,
            directory_url,
            cert_path,
            key_path,
            challenge_port,
            renew_before_days,
            profile,
            account_key_path,
            challenge,
            dns,
        })
    }

    /// Point the TLS listener at this config's certificate: sets
    /// `RWS_CONFIG_TLS_CERT_FILE` / `RWS_CONFIG_TLS_KEY_FILE` to `cert_path` /
    /// `key_path` when they're empty, so setting only `RWS_CONFIG_ACME_DOMAINS`
    /// is enough for `Server::run_tls` to serve the issued certificate. An
    /// explicitly configured TLS path is left alone.
    pub fn install_as_tls_certificate(&self) {
        if non_empty_env(Config::RWS_CONFIG_TLS_CERT_FILE).is_none() {
            std::env::set_var(Config::RWS_CONFIG_TLS_CERT_FILE, &self.cert_path);
        }
        if non_empty_env(Config::RWS_CONFIG_TLS_KEY_FILE).is_none() {
            std::env::set_var(Config::RWS_CONFIG_TLS_KEY_FILE, &self.key_path);
        }
    }
}

pub(crate) fn renew_check_interval_secs() -> u64 {
    non_empty_env(Config::RWS_CONFIG_ACME_RENEW_CHECK_INTERVAL_SECS)
        .and_then(|v| v.parse().ok())
        .filter(|secs| *secs > 0)
        .unwrap_or(12 * 3600)
}

/// Where ACME keeps its files when no path is configured (relative to the
/// working directory, but a dot-directory: never served).
pub const STATE_DIR: &str = ".rws/acme";
const DEFAULT_CERT_PATH: &str = ".rws/acme/cert.pem";
const DEFAULT_KEY_PATH: &str = ".rws/acme/key.pem";
const DEFAULT_ACCOUNT_KEY_PATH: &str = ".rws/acme/account.key";
/// The defaults before 17.113.1 — in the web root.
const LEGACY_DEFAULTS: [(&str, &str); 3] = [
    ("cert.pem", DEFAULT_CERT_PATH),
    ("key.pem", DEFAULT_KEY_PATH),
    ("acme_account.key", DEFAULT_ACCOUNT_KEY_PATH),
];

/// Move files left at the old default paths (the web root) into
/// [`STATE_DIR`] when the new default is in use and has no file yet, so an
/// upgrade keeps its certificate and account — and stops serving its keys.
fn adopt_legacy_files(config: &AcmeConfig) {
    for (legacy, current) in LEGACY_DEFAULTS {
        let in_use = [&config.cert_path, &config.key_path, &config.account_key_path].iter().any(|p| p.as_str() == current);
        if !in_use || std::path::Path::new(current).exists() || !std::path::Path::new(legacy).is_file() {
            continue;
        }
        match move_private(legacy, current) {
            Ok(()) => println!("[ACME] Moved '{legacy}' (in the web root) to '{current}'."),
            Err(e) => eprintln!("[ACME] Could not move '{legacy}' to '{current}': {e}"),
        }
    }
}

/// Move `from` to `to` (creating `to`'s directory), leaving it `0600`.
fn move_private(from: &str, to: &str) -> std::io::Result<()> {
    ensure_state_dir(to)?;
    std::fs::rename(from, to).or_else(|_| std::fs::copy(from, to).and_then(|_| std::fs::remove_file(from)))?;
    restrict_permissions(to);
    Ok(())
}

/// Create the directory holding `path`, `0700` on Unix.
fn ensure_state_dir(path: &str) -> std::io::Result<()> {
    let Some(dir) = std::path::Path::new(path).parent().filter(|d| !d.as_os_str().is_empty()) else {
        return Ok(());
    };
    if dir.is_dir() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(dir)
    }
}

/// Write a private key: `0600` on Unix (the file is chmod-ed too if it
/// already existed with broader permissions).
fn write_private(path: &str, contents: &[u8]) -> std::io::Result<()> {
    ensure_state_dir(path)?;
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)?;
        file.write_all(contents)?;
        restrict_permissions(path);
        Ok(())
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, contents)
    }
}

fn restrict_permissions(path: &str) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = path;
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

// ── HTTP-01 challenge store ───────────────────────────────────────────────────
//
// Pending challenges live in a process-wide map rather than inside one
// temporary server, so whichever listener owns the challenge port can answer
// them: the standalone server below at startup, or `Server::run_redirect`'s
// HTTP→HTTPS listener once it holds port 80 — renewal would otherwise fail to
// bind the port the redirect listener already has.

pub(crate) fn challenges() -> &'static std::sync::Mutex<std::collections::HashMap<String, String>> {
    static CHALLENGES: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, String>>> =
        std::sync::OnceLock::new();
    CHALLENGES.get_or_init(Default::default)
}

/// Port the HTTP→HTTPS redirect listener is bound to, `0` when not running.
static REDIRECT_LISTENER_PORT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);

/// Called by `Server::run_redirect` once it has bound `port`, so challenges
/// on that port are served by it instead of a standalone server.
pub(crate) fn note_redirect_listener(port: u16) {
    REDIRECT_LISTENER_PORT.store(port, std::sync::atomic::Ordering::SeqCst);
}

const CHALLENGE_PATH_PREFIX: &str = "/.well-known/acme-challenge/";

/// The full HTTP/1.1 response for a request to `uri` if it's a pending
/// HTTP-01 challenge (`200` with the key authorization), else `None`.
pub(crate) fn challenge_http_response(uri: &str) -> Option<String> {
    let token = uri.strip_prefix(CHALLENGE_PATH_PREFIX)?;
    let key_auth = challenges().lock().unwrap().get(token)?.clone();
    Some(challenge_response(&key_auth))
}

/// [`challenge_http_response`], also answering a challenge another replica
/// presented (`[acme] redis`).
pub(crate) async fn challenge_http_response_shared(uri: &str) -> Option<String> {
    if let Some(response) = challenge_http_response(uri) {
        return Some(response);
    }
    let store = shared::shared()?;
    let token = uri.strip_prefix(CHALLENGE_PATH_PREFIX)?.to_string();
    if token.is_empty() || token.len() > 256 {
        return None;
    }
    let key_auth = tokio::task::spawn_blocking(move || store.challenge(&token)).await.ok()??;
    Some(challenge_response(&key_auth))
}

fn challenge_response(key_auth: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        key_auth.len(),
        key_auth
    )
}

// ── Manager ───────────────────────────────────────────────────────────────────

/// Manages the ACME lifecycle: initial provisioning and automatic renewal.
pub struct AcmeManager {
    config: AcmeConfig,
}

impl AcmeManager {
    pub fn new(config: AcmeConfig) -> Self {
        AcmeManager { config }
    }

    /// Provision a certificate if there is none, or renew it when it's due
    /// (see [`check`](Self::check)).
    pub async fn provision_if_needed(&self) -> Result<(), String> {
        self.check().await.map(|_| ())
    }

    /// Renew when due, else say when to look again. Due is the two-thirds
    /// point of the certificate's lifetime (`renew_before_days` set: that
    /// many days before expiry) — or earlier, when the CA's ACME Renewal
    /// Information (RFC 9773) suggests a window that starts sooner, e.g.
    /// after a revocation. A fixed "30 days before expiry" would renew a
    /// 6-day certificate at once and leave a 45-day one too little margin.
    /// Renewal information that can't be fetched is not an error: the
    /// two-thirds point still applies.
    pub(crate) async fn check(&self) -> Result<RenewalCheck, String> {
        // Another replica renewed it: install theirs (the caller hot-swaps).
        if self.adopt_shared().await {
            return Ok(RenewalCheck::Renewed);
        }
        let Some(info) = crypto::read_cert_info(&self.config.cert_path) else {
            println!("[ACME] No valid certificate found at '{}', provisioning...", self.config.cert_path);
            self.provision().await?;
            return Ok(RenewalCheck::Renewed);
        };
        // A name added to `domains` since the certificate was issued: order
        // one that covers them all. Not as a renewal (`replaces`): the CA
        // only accepts that for the same identifiers.
        let missing = missing_names(&self.config.domains, crypto::read_cert_dns_names(&self.config.cert_path).as_deref());
        if !missing.is_empty() {
            println!("[ACME] Certificate at '{}' doesn't cover {:?}; ordering one for {:?}.",
                self.config.cert_path, missing, self.config.domains);
            self.provision_coordinated(None).await?;
            return Ok(RenewalCheck::Renewed);
        }
        let (window, retry_after) = match &info.cert_id {
            Some(cert_id) => match self.renewal_info(cert_id).await {
                Ok(found) => found,
                Err(e) => {
                    eprintln!("[ACME] No renewal information ({e}); renewing at two-thirds of the lifetime.");
                    (None, None)
                }
            },
            None => (None, None),
        };
        let due = renewal_time(&info, window, self.config.renew_before_days);
        let now = unix_now();
        if now >= due {
            println!("[ACME] Certificate at '{}' is due for renewal (expires {}), renewing...",
                self.config.cert_path, crate::http::date::format(info.not_after.max(0) as u64));
            self.provision_coordinated(info.cert_id.as_deref()).await?;
            return Ok(RenewalCheck::Renewed);
        }
        println!("[ACME] Certificate at '{}' is current; renewal due {}.",
            self.config.cert_path, crate::http::date::format(due.max(0) as u64));
        let mut recheck = renew_check_interval_secs().min((due - now).max(60) as u64);
        if info.cert_id.is_some() {
            // The CA's answer can change (revocation): ask again when it
            // says, else every 6 hours.
            recheck = recheck.min(retry_after.unwrap_or(RENEWAL_INFO_RECHECK_SECS).max(60));
        }
        Ok(RenewalCheck::Current { recheck_in: recheck })
    }

    /// The CA's suggested renewal window for `cert_id` and its `Retry-After`,
    /// from the directory's `renewalInfo` endpoint. `(None, None)` when the
    /// CA has no such endpoint.
    async fn renewal_info(&self, cert_id: &str) -> Result<(Option<(i64, i64)>, Option<u64>), String> {
        let http = AcmeHttpClient::new()?;
        let dir = fetch_directory(&http, &self.config.directory_url).await?;
        let Some(base) = dir.renewal_info_url else { return Ok((None, None)) };
        let resp = http.get(&format!("{}/{}", base.trim_end_matches('/'), cert_id)).await?;
        if resp.status != 200 {
            return Err(format!("renewalInfo: HTTP {}", resp.status));
        }
        let retry_after = resp.header("retry-after").and_then(|v| {
            v.trim().parse::<u64>().ok().or_else(|| {
                crate::http::date::parse(v).map(|at| at.saturating_sub(unix_now().max(0) as u64))
            })
        });
        Ok((parse_suggested_window(&resp.body), retry_after))
    }

    /// Provision a new certificate unconditionally.
    pub async fn provision(&self) -> Result<(), String> {
        self.provision_coordinated(None).await
    }

    /// [`provision_replacing`](Self::provision_replacing), coordinated with
    /// other replicas when `[acme] redis` is set: the one that takes the lock
    /// orders and stores the result, the others wait for it.
    async fn provision_coordinated(&self, replaces: Option<&str>) -> Result<(), String> {
        let Some(store) = shared::shared() else {
            return self.provision_replacing(replaces).await;
        };
        let domains = self.config.domains.clone();
        let locked = tokio::task::spawn_blocking(move || store.lock(&domains)).await.unwrap_or(None);
        if locked == Some(false) {
            println!("[ACME] Another replica is ordering the certificate for {:?}; waiting for it.", self.config.domains);
            let deadline = tokio::time::Instant::now() + shared::WAIT_FOR_OTHER;
            while tokio::time::Instant::now() < deadline {
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                if self.adopt_shared().await {
                    return Ok(());
                }
            }
            return Err("another replica held the ordering lock, but no certificate appeared".to_string());
        }
        let result = self.provision_replacing(replaces).await;
        let (cert_path, key_path, domains) = (self.config.cert_path.clone(), self.config.key_path.clone(), self.config.domains.clone());
        let stored = result.is_ok();
        let _ = tokio::task::spawn_blocking(move || {
            if stored {
                if let (Ok(cert), Ok(key)) = (std::fs::read(&cert_path), std::fs::read(&key_path)) {
                    store.store_certificate(&domains, &cert, &key);
                }
            }
            if locked == Some(true) {
                store.unlock(&domains);
            }
        })
        .await;
        result
    }

    /// Install the certificate another replica stored in Redis when it is
    /// newer than the local one. `true`: the local files changed.
    async fn adopt_shared(&self) -> bool {
        let Some(store) = shared::shared() else { return false };
        let domains = self.config.domains.clone();
        let Ok(Some((cert, key))) = tokio::task::spawn_blocking(move || store.certificate(&domains)).await else {
            return false;
        };
        let Some(theirs) = crypto::cert_info_from_pem(&cert) else { return false };
        if crypto::read_cert_info(&self.config.cert_path).is_some_and(|ours| ours.not_after >= theirs.not_after) {
            return false;
        }
        let written = ensure_state_dir(&self.config.cert_path)
            .and_then(|_| std::fs::write(&self.config.cert_path, &cert))
            .and_then(|_| write_private(&self.config.key_path, &key));
        match written {
            Ok(()) => {
                println!("[ACME] Installed the certificate another replica stored (expires {}).", crate::http::date::format(theirs.not_after.max(0) as u64));
                true
            }
            Err(e) => {
                eprintln!("[ACME] Couldn't install the shared certificate: {e}");
                false
            }
        }
    }

    /// [`provision`](Self::provision), telling the CA which certificate the
    /// new one replaces (`replaces`, RFC 9773 §5) so it can exempt the order
    /// from rate limits and track the renewal.
    async fn provision_replacing(&self, replaces: Option<&str>) -> Result<(), String> {
        println!("[ACME] Provisioning certificate for: {:?}", self.config.domains);

        let http = AcmeHttpClient::new()?;

        // 1. Load or generate account key.
        let account_key = self.load_or_create_account_key()?;

        // 2. Fetch ACME directory.
        let dir = fetch_directory(&http, &self.config.directory_url).await?;
        if let Some(profile) = &self.config.profile {
            if !dir.profiles.is_empty() && !dir.profiles.contains(profile) {
                return Err(format!(
                    "the CA doesn't offer the certificate profile {profile:?} (RWS_CONFIG_ACME_PROFILE); it offers: {}",
                    dir.profiles.join(", ")
                ));
            }
        }

        // 3. Get initial nonce.
        let nonce = get_nonce(&http, &dir.new_nonce_url).await?;

        // 4. Create/find account; get account URL and fresh nonce.
        let (account_url, nonce) =
            create_account(&http, &account_key, &dir.new_account_url, &self.config.email, &nonce).await?;
        println!("[ACME] Account: {account_url}");

        // 5. Submit order. A CA may refuse `replaces` (the certificate was
        // already replaced by an order whose certificate never got
        // installed): order again without it rather than not at all.
        let profile = self.config.profile.as_deref();
        let ordered = new_order(&http, &account_key, &account_url, &dir.new_order_url, &self.config.domains, replaces, profile, &nonce).await;
        let (order, order_url, nonce) = match (ordered, replaces) {
            (Ok(order), _) => order,
            (Err(e), Some(_)) => {
                eprintln!("[ACME] Order replacing the current certificate refused ({e}); ordering without 'replaces'.");
                let nonce = get_nonce(&http, &dir.new_nonce_url).await?;
                new_order(&http, &account_key, &account_url, &dir.new_order_url, &self.config.domains, None, profile, &nonce).await?
            }
            (Err(e), None) => return Err(e),
        };
        println!("[ACME] Order: {order_url}{}", replaces.map(|id| format!(" (replacing {id})")).unwrap_or_default());

        // 6. Complete HTTP-01 challenge for each authorization.
        let mut nonce = nonce;
        for authz_url in &order.authorizations {
            nonce = self.complete_authorization(&http, &account_key, &account_url, authz_url, &nonce).await?;
        }

        // 7. Generate certificate key + CSR.
        let (cert_key, csr_der) = generate_csr(&self.config.domains)?;
        let csr_b64 = crypto::base64url(&csr_der);

        // 8. Finalize order with CSR.
        let nonce = finalize_order(&http, &account_key, &account_url, &order.finalize_url, &csr_b64, &nonce).await?;

        // 9. Poll order until certificate URL is available.
        let (cert_url, nonce) = poll_order(&http, &account_key, &account_url, &order_url, &nonce).await?;
        println!("[ACME] Certificate URL: {cert_url}");

        // 10. Download certificate chain.
        let cert_pem = download_cert(&http, &account_key, &account_url, &cert_url, &nonce).await?;

        // 11. Write cert + key.
        ensure_state_dir(&self.config.cert_path)
            .and_then(|_| std::fs::write(&self.config.cert_path, &cert_pem))
            .map_err(|e| format!("failed to write cert to '{}': {e}", self.config.cert_path))?;
        write_private(&self.config.key_path, cert_key.serialize_pem().as_bytes())
            .map_err(|e| format!("failed to write key to '{}': {e}", self.config.key_path))?;

        println!(
            "[ACME] Certificate written to '{}' and '{}'.",
            self.config.cert_path, self.config.key_path
        );
        Ok(())
    }

    /// Startup provisioning that doesn't give up: retries
    /// [`provision_if_needed`](Self::provision_if_needed) with exponential
    /// backoff (30s doubling to 15 min) until a certificate is in place —
    /// e.g. while DNS for a new domain is still propagating. Without a
    /// certificate the TLS listener has nothing to serve.
    pub async fn provision_until_ready(&self) {
        adopt_legacy_files(&self.config);
        let mut delay = 30u64;
        loop {
            match self.provision_if_needed().await {
                Ok(()) => return,
                Err(e) => {
                    eprintln!("[ACME] Provisioning failed: {e}");
                    eprintln!("[ACME] Retrying in {delay}s...");
                    tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                    delay = (delay * 2).min(15 * 60);
                }
            }
        }
    }

    /// Background renewal loop: [`check`](Self::check)s at most every
    /// `RWS_CONFIG_ACME_RENEW_CHECK_INTERVAL_SECS` (default 12 hours), sooner
    /// when renewal is due earlier or the CA's renewal information asks to be
    /// fetched again; after a failure it retries within an hour. After a
    /// renewal it sends SIGHUP to itself (Unix) so the running server reloads
    /// the TLS certificate — only then (it used to on every check).
    pub async fn run_renewal_loop(self) {
        renewal().running.store(true, std::sync::atomic::Ordering::SeqCst);
        let interval = renew_check_interval_secs();
        let mut next = crypto::read_cert_info(&self.config.cert_path)
            .map(|info| ((renewal_time(&info, None, self.config.renew_before_days) - unix_now()).max(60) as u64).min(interval))
            .unwrap_or(interval);
        loop {
            let forced = tokio::select! {
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(next)) => false,
                _ = renewal().now.notified() => true,
            };
            let result = if forced {
                println!("[ACME] Renewal of '{}' requested through the admin API, renewing...", self.config.cert_path);
                let cert_id = crypto::read_cert_info(&self.config.cert_path).and_then(|info| info.cert_id);
                self.provision_coordinated(cert_id.as_deref()).await.map(|_| RenewalCheck::Renewed)
            } else {
                self.check().await
            };
            renewal().record(&result);
            match result {
                Ok(RenewalCheck::Current { recheck_in }) => next = recheck_in,
                Ok(RenewalCheck::Renewed) => {
                    // Look at the new certificate's renewal information soon.
                    next = 60;
                    #[cfg(unix)]
                    {
                        // Signal the process to reload TLS cert (run_tls reloads on SIGHUP).
                        unsafe { libc::kill(libc::getpid(), libc::SIGHUP); }
                    }
                }
                Err(e) => {
                    next = interval.min(3600);
                    eprintln!("[ACME] Renewal failed: {e} — retrying in {next}s");
                }
            }
        }
    }

    // ── private helpers ───────────────────────────────────────────────────────

    fn load_or_create_account_key(&self) -> Result<AccountKey, String> {
        let path = &self.config.account_key_path;
        if let Ok(der) = std::fs::read(path) {
            let key = AccountKey::from_pkcs8(&der)
                .map_err(|e| format!("failed to load account key from '{path}': {e}"))?;
            println!("[ACME] Loaded account key from '{path}'.");
            return Ok(key);
        }
        let (key, der) = AccountKey::generate()
            .map_err(|e| format!("account key generation failed: {e}"))?;
        write_private(path, &der)
            .map_err(|e| format!("failed to save account key to '{path}': {e}"))?;
        println!("[ACME] Generated new account key at '{path}'.");
        Ok(key)
    }

    async fn complete_authorization(
        &self,
        http: &AcmeHttpClient,
        key: &AccountKey,
        account_url: &str,
        authz_url: &str,
        nonce: &str,
    ) -> Result<String, String> {
        let (authz, nonce) = get_authorization(http, key, account_url, authz_url, nonce).await?;

        if authz.status == "valid" {
            return Ok(nonce);
        }

        let kind = self.config.challenge;
        let challenge = authz.challenges.iter()
            .find(|c| c.challenge_type == kind.as_str())
            .ok_or_else(|| format!("no {} challenge in authorization {authz_url} (the CA offers: {})", kind.as_str(),
                authz.challenges.iter().map(|c| c.challenge_type.as_str()).collect::<Vec<_>>().join(", ")))?;

        let key_auth = format!(
            "{}.{}",
            challenge.token,
            crypto::key_thumbprint(key)?
        );
        // The authorization's name: a wildcard's base name, without "*.".
        let domain = authz.identifier_value.clone();

        let presented = if kind == ChallengeType::DnsPersist01 {
            self.check_persist_record(&domain, &challenge.issuer_domain_names, account_url).await?
        } else {
            self.present(kind, &challenge.token, &domain, &key_auth).await?
        };

        // Signal challenge ready, then poll for validation (up to 60 seconds).
        let poll_result = match signal_challenge(http, key, account_url, &challenge.url, &nonce).await {
            Ok(nonce) => poll_authorization_valid(http, key, account_url, authz_url, &nonce).await,
            Err(e) => Err(e),
        };

        // Stop serving the challenge regardless of the outcome.
        presented.withdraw().await;

        let nonce = poll_result?;

        println!("[ACME] Authorization validated for '{}'.", authz.identifier_value);
        Ok(nonce)
    }
}

impl AcmeManager {
    /// Put the answer to a `kind` challenge where the CA will look.
    async fn present(&self, kind: ChallengeType, token: &str, domain: &str, key_auth: &str) -> Result<Presented, String> {
        match kind {
            ChallengeType::Http01 => {
                challenges().lock().unwrap().insert(token.to_string(), key_auth.to_string());
                if let Some(store) = shared::shared() {
                    let (token, key_auth) = (token.to_string(), key_auth.to_string());
                    let _ = tokio::task::spawn_blocking(move || store.put_challenge(&token, &key_auth)).await;
                }
                // The redirect listener answers challenges itself once it owns the
                // port (renewals); otherwise (startup) run a temporary server.
                let port = self.config.challenge_port;
                let standalone = if REDIRECT_LISTENER_PORT.load(std::sync::atomic::Ordering::SeqCst) == port {
                    println!("[ACME] Answering HTTP-01 challenge via the redirect listener on port {port}.");
                    None
                } else {
                    println!("[ACME] Starting HTTP-01 challenge server on port {port}...");
                    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
                    let handle = tokio::spawn(async move {
                        if let Err(e) = run_challenge_server(port, shutdown_rx).await {
                            eprintln!("[ACME] Challenge server error: {e}");
                        }
                    });
                    // Small delay to let the server bind before signalling ACME.
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                    Some((shutdown_tx, handle))
                };
                Ok(Presented::Http { token: token.to_string(), standalone })
            }
            ChallengeType::TlsAlpn01 => {
                tls_alpn::add(domain, key_auth)?;
                if let Some(store) = shared::shared() {
                    let (domain, key_auth) = (domain.to_string(), key_auth.to_string());
                    let _ = tokio::task::spawn_blocking(move || store.put_tls_alpn(&domain, &key_auth)).await;
                }
                let standalone = if tls_alpn::tls_listener_up() {
                    println!("[ACME] Answering TLS-ALPN-01 challenge for {domain} via the TLS listener.");
                    None
                } else {
                    let (ip, port, _) = crate::entry_point::get_ip_port_thread_count();
                    let addr = if ip.contains(':') { format!("[{ip}]:{port}") } else { format!("{ip}:{port}") };
                    println!("[ACME] Answering TLS-ALPN-01 challenge for {domain} on {addr}...");
                    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
                    let handle = tokio::spawn(async move {
                        if let Err(e) = tls_alpn::serve(addr, shutdown_rx).await {
                            eprintln!("[ACME] TLS-ALPN-01 listener error: {e}");
                        }
                    });
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                    Some((shutdown_tx, handle))
                };
                Ok(Presented::TlsAlpn { domain: domain.to_string(), standalone })
            }
            ChallengeType::Dns01 => {
                let dns = self.config.dns.clone().ok_or("DNS-01 isn't configured (RWS_CONFIG_ACME_DNS_PROVIDER)")?;
                let name = dns01::record_name(domain);
                let value = dns01::record_value(key_auth);
                println!("[ACME] Publishing the DNS-01 record {name} TXT {value}...");
                dns01::present(&dns, &name, &value).await?;
                if dns01::wait_visible(&dns, &name, &value).await {
                    println!("[ACME] {name} is visible.");
                } else {
                    eprintln!("[ACME] {name} isn't visible after {}s; asking the CA to check anyway.", dns.propagation_secs);
                }
                Ok(Presented::Dns { config: dns, name, value })
            }
            ChallengeType::DnsPersist01 => Err("dns-persist-01 records are published by you, not presented".to_string()),
        }
    }

    /// DNS-PERSIST-01: make sure the record you published for `domain` is
    /// visible before asking the CA (a failed validation counts against its
    /// limits); when it isn't, say exactly what to publish.
    async fn check_persist_record(&self, domain: &str, issuers: &[String], account_url: &str) -> Result<Presented, String> {
        let dns = self.config.dns.clone().ok_or("dns-persist-01 isn't configured")?;
        let issuer = issuers.first().ok_or("the CA's dns-persist-01 challenge names no issuer-domain-names")?;
        // One record covers the name and, with policy=wildcard, `*.name`.
        let wildcard = self.config.domains.iter().any(|d| d.strip_prefix("*.").is_some_and(|base| base.eq_ignore_ascii_case(domain)));
        let name = dns01::persist_record_name(domain);
        if dns01::persist_record_published(&dns, &name, issuers, account_url, wildcard).await? {
            println!("[ACME] Found the DNS-PERSIST-01 record at {name}.");
            return Ok(Presented::Persistent);
        }
        let value = dns01::persist_record_value(issuer, account_url, wildcard);
        Err(format!("dns-persist-01: publish this TXT record once (it stays; rws retries until it's visible): {name}. TXT \"{value}\""))
    }
}

/// A challenge answer in place, withdrawn after validation.
enum Presented {
    Http { token: String, standalone: Option<(tokio::sync::oneshot::Sender<()>, tokio::task::JoinHandle<()>)> },
    TlsAlpn { domain: String, standalone: Option<(tokio::sync::oneshot::Sender<()>, tokio::task::JoinHandle<()>)> },
    Dns { config: DnsConfig, name: String, value: String },
    /// A DNS-PERSIST-01 record: yours, left in place.
    Persistent,
}

impl Presented {
    async fn withdraw(self) {
        let standalone = match self {
            Presented::Http { token, standalone } => {
                challenges().lock().unwrap().remove(&token);
                if let Some(store) = shared::shared() {
                    let _ = tokio::task::spawn_blocking(move || store.remove_challenge(&token)).await;
                }
                standalone
            }
            Presented::TlsAlpn { domain, standalone } => {
                tls_alpn::remove(&domain);
                if let Some(store) = shared::shared() {
                    let _ = tokio::task::spawn_blocking(move || store.remove_tls_alpn(&domain)).await;
                }
                standalone
            }
            Presented::Dns { config, name, value } => {
                if let Err(e) = dns01::cleanup(&config, &name, &value).await {
                    eprintln!("[ACME] Couldn't remove the DNS-01 record {name}: {e}");
                }
                None
            }
            Presented::Persistent => None,
        };
        if let Some((shutdown_tx, handle)) = standalone {
            let _ = shutdown_tx.send(());
            let _ = handle.await;
        }
    }
}

// ── ACME protocol functions ───────────────────────────────────────────────────

struct Directory {
    new_nonce_url: String,
    new_account_url: String,
    new_order_url: String,
    /// ACME Renewal Information base URL (RFC 9773), if the CA has one.
    renewal_info_url: Option<String>,
    /// Certificate profiles the CA offers (`meta.profiles`); empty when it
    /// doesn't list any.
    profiles: Vec<String>,
}

struct Order {
    authorizations: Vec<String>,
    finalize_url: String,
}

struct Authorization {
    status: String,
    identifier_value: String,
    challenges: Vec<Challenge>,
}

struct Challenge {
    challenge_type: String,
    url: String,
    /// Empty for `dns-persist-01`, which has none.
    token: String,
    /// `dns-persist-01`: the CA names its record may carry.
    issuer_domain_names: Vec<String>,
}

async fn fetch_directory(http: &AcmeHttpClient, url: &str) -> Result<Directory, String> {
    let resp = http.get(url).await?;
    if resp.status != 200 {
        return Err(format!("directory fetch failed: HTTP {}: {}", resp.status, resp.body));
    }
    let new_nonce_url = json_str(&resp.body, "newNonce")
        .ok_or("directory missing newNonce")?;
    let new_account_url = json_str(&resp.body, "newAccount")
        .ok_or("directory missing newAccount")?;
    let new_order_url = json_str(&resp.body, "newOrder")
        .ok_or("directory missing newOrder")?;
    let renewal_info_url = json_str(&resp.body, "renewalInfo");
    let profiles = json_object_keys(&resp.body, "profiles");
    Ok(Directory { new_nonce_url, new_account_url, new_order_url, renewal_info_url, profiles })
}

async fn get_nonce(http: &AcmeHttpClient, url: &str) -> Result<String, String> {
    let resp = http.head(url).await?;
    resp.nonce().ok_or_else(|| "Replay-Nonce missing from newNonce response".to_string())
}

async fn create_account(
    http: &AcmeHttpClient,
    key: &AccountKey,
    url: &str,
    email: &str,
    nonce: &str,
) -> Result<(String, String), String> {
    let payload = if email.is_empty() {
        r#"{"termsOfServiceAgreed":true}"#.to_string()
    } else {
        format!(r#"{{"termsOfServiceAgreed":true,"contact":["mailto:{email}"]}}"#)
    };
    let body = crypto::build_jws(key, nonce, url, None, Some(&payload))?;
    let resp = http.post_jws(url, &body).await?;
    if resp.status != 200 && resp.status != 201 {
        return Err(format!("newAccount failed: HTTP {}: {}", resp.status, resp.body));
    }
    let account_url = resp.location()
        .ok_or("newAccount response missing Location header")?;
    let nonce = resp.nonce()
        .ok_or("newAccount response missing Replay-Nonce")?;
    Ok((account_url, nonce))
}

#[allow(clippy::too_many_arguments)]
async fn new_order(
    http: &AcmeHttpClient,
    key: &AccountKey,
    account_url: &str,
    url: &str,
    domains: &[String],
    replaces: Option<&str>,
    profile: Option<&str>,
    nonce: &str,
) -> Result<(Order, String, String), String> {
    let payload = new_order_payload(domains, replaces, profile);
    let body = crypto::build_jws(key, nonce, url, Some(account_url), Some(&payload))?;
    let resp = http.post_jws(url, &body).await?;
    if resp.status != 201 {
        return Err(format!("newOrder failed: HTTP {}: {}", resp.status, resp.body));
    }
    let order_url = resp.location()
        .ok_or("newOrder response missing Location header")?;
    let nonce = resp.nonce()
        .ok_or("newOrder response missing Replay-Nonce")?;
    let authorizations = json_array_strings(&resp.body, "authorizations");
    let finalize_url = json_str(&resp.body, "finalize")
        .ok_or("newOrder response missing finalize URL")?;
    Ok((Order { authorizations, finalize_url }, order_url, nonce))
}

async fn get_authorization(
    http: &AcmeHttpClient,
    key: &AccountKey,
    account_url: &str,
    authz_url: &str,
    nonce: &str,
) -> Result<(Authorization, String), String> {
    // POST-as-GET (empty payload)
    let body = crypto::build_jws(key, nonce, authz_url, Some(account_url), None)?;
    let resp = http.post_jws(authz_url, &body).await?;
    if resp.status != 200 {
        return Err(format!("authorization GET failed: HTTP {}: {}", resp.status, resp.body));
    }
    let nonce = resp.nonce().unwrap_or_default();
    let status = json_str(&resp.body, "status").unwrap_or_default();
    let identifier_value = json_str(&resp.body, "value").unwrap_or_default();
    let challenges = parse_challenges(&resp.body);
    Ok((Authorization { status, identifier_value, challenges }, nonce))
}

async fn signal_challenge(
    http: &AcmeHttpClient,
    key: &AccountKey,
    account_url: &str,
    challenge_url: &str,
    nonce: &str,
) -> Result<String, String> {
    let body = crypto::build_jws(key, nonce, challenge_url, Some(account_url), Some("{}"))?;
    let resp = http.post_jws(challenge_url, &body).await?;
    if resp.status != 200 {
        return Err(format!("challenge signal failed: HTTP {}: {}", resp.status, resp.body));
    }
    Ok(resp.nonce().unwrap_or_default())
}

async fn poll_authorization_valid(
    http: &AcmeHttpClient,
    key: &AccountKey,
    account_url: &str,
    authz_url: &str,
    start_nonce: &str,
) -> Result<String, String> {
    let mut nonce = start_nonce.to_string();
    for attempt in 0..20 {
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        // If nonce is empty, fetch a fresh one from the authz URL
        let body = crypto::build_jws(key, &nonce, authz_url, Some(account_url), None)?;
        let resp = http.post_jws(authz_url, &body).await?;
        if let Some(n) = resp.nonce() { nonce = n; }
        let status = json_str(&resp.body, "status").unwrap_or_default();
        match status.as_str() {
            "valid" => return Ok(nonce),
            "invalid" => {
                let err = json_str(&resp.body, "detail").unwrap_or_else(|| resp.body.clone());
                return Err(format!("authorization became invalid: {err}"));
            }
            _ => {
                if attempt == 19 {
                    return Err(format!("authorization timed out with status '{status}'"));
                }
            }
        }
    }
    Err("authorization polling timed out".to_string())
}

async fn finalize_order(
    http: &AcmeHttpClient,
    key: &AccountKey,
    account_url: &str,
    finalize_url: &str,
    csr_b64: &str,
    nonce: &str,
) -> Result<String, String> {
    let payload = format!(r#"{{"csr":"{csr_b64}"}}"#);
    let body = crypto::build_jws(key, nonce, finalize_url, Some(account_url), Some(&payload))?;
    let resp = http.post_jws(finalize_url, &body).await?;
    if resp.status != 200 {
        return Err(format!("finalize failed: HTTP {}: {}", resp.status, resp.body));
    }
    Ok(resp.nonce().unwrap_or_default())
}

async fn poll_order(
    http: &AcmeHttpClient,
    key: &AccountKey,
    account_url: &str,
    order_url: &str,
    start_nonce: &str,
) -> Result<(String, String), String> {
    let mut nonce = start_nonce.to_string();
    for attempt in 0..20 {
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        let body = crypto::build_jws(key, &nonce, order_url, Some(account_url), None)?;
        let resp = http.post_jws(order_url, &body).await?;
        if let Some(n) = resp.nonce() { nonce = n; }
        let status = json_str(&resp.body, "status").unwrap_or_default();
        match status.as_str() {
            "valid" => {
                let cert_url = json_str(&resp.body, "certificate")
                    .ok_or("order is valid but missing certificate URL")?;
                return Ok((cert_url, nonce));
            }
            "invalid" => {
                return Err(format!("order became invalid: {}", resp.body));
            }
            _ => {
                if attempt == 19 {
                    return Err(format!("order timed out with status '{status}'"));
                }
            }
        }
    }
    Err("order polling timed out".to_string())
}

async fn download_cert(
    http: &AcmeHttpClient,
    key: &AccountKey,
    account_url: &str,
    cert_url: &str,
    nonce: &str,
) -> Result<String, String> {
    let body = crypto::build_jws(key, nonce, cert_url, Some(account_url), None)?;
    let resp = http.post_jws(cert_url, &body).await?;
    if resp.status != 200 {
        return Err(format!("cert download failed: HTTP {}: {}", resp.status, resp.body));
    }
    Ok(resp.body)
}

// ── HTTP-01 challenge server ──────────────────────────────────────────────────

async fn run_challenge_server(
    port: u16,
    shutdown: tokio::sync::oneshot::Receiver<()>,
) -> Result<(), String> {
    let listeners = crate::server::bind_all_families("0.0.0.0", port)
        .await
        .map_err(|e| format!("challenge server: bind port {port} failed: {e}"))?;

    let tasks: Vec<_> = listeners.into_iter().map(|l| tokio::spawn(serve_challenges(l))).collect();
    let _ = shutdown.await;
    for task in tasks {
        task.abort();
    }
    Ok(())
}

async fn serve_challenges(listener: tokio::net::TcpListener) {
    loop {
        let Ok((mut stream, _)) = listener.accept().await else { break };
        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            let n = stream.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let uri = req.lines().next().and_then(|l| l.split_whitespace().nth(1)).unwrap_or("");
            let response = challenge_http_response_shared(uri).await.unwrap_or_else(|| {
                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
            });
            let _ = stream.write_all(response.as_bytes()).await;
        });
    }
}

// ── minimal JSON field extractor ──────────────────────────────────────────────

/// Generate a fresh ECDSA P-256 certificate key and a DER-encoded CSR for
/// `domains` (all listed as subjectAltName dNSNames, empty subject — Let's
/// Encrypt ignores the subject and sets validity itself).
fn generate_csr(domains: &[String]) -> Result<(rcgen::KeyPair, Vec<u8>), String> {
    let cert_key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)
        .map_err(|e| format!("cert key generation failed: {e}"))?;
    let mut params = rcgen::CertificateParams::new(domains.to_vec())
        .map_err(|e| format!("CSR params error: {e}"))?;
    params.distinguished_name = rcgen::DistinguishedName::new();
    let csr = params.serialize_request(&cert_key)
        .map_err(|e| format!("CSR serialize error: {e}"))?;
    Ok((cert_key, csr.der().to_vec()))
}

/// The `newOrder` payload: identifiers, plus `replaces` (RFC 9773 §5) and
/// `profile` (draft-ietf-acme-profiles) when given.
pub(crate) fn new_order_payload(domains: &[String], replaces: Option<&str>, profile: Option<&str>) -> String {
    let ids: Vec<String> = domains.iter()
        .map(|d| format!(r#"{{"type":"dns","value":"{}"}}"#, json_escape(d)))
        .collect();
    let mut payload = format!(r#"{{"identifiers":[{}]"#, ids.join(","));
    if let Some(replaces) = replaces {
        payload.push_str(&format!(r#","replaces":"{}""#, json_escape(replaces)));
    }
    if let Some(profile) = profile {
        payload.push_str(&format!(r#","profile":"{}""#, json_escape(profile)));
    }
    payload.push('}');
    payload
}

fn json_escape(value: &str) -> String {
    value.chars().flat_map(|c| match c {
        '"' => vec!['\\', '"'],
        '\\' => vec!['\\', '\\'],
        c if (c as u32) < 0x20 => format!("\\u{:04x}", c as u32).chars().collect(),
        c => vec![c],
    }).collect()
}

/// The renewal loop's state, for the admin API.
pub(crate) struct Renewal {
    running: std::sync::atomic::AtomicBool,
    now: tokio::sync::Notify,
    /// The last check's outcome: (Unix time, `renewed` / `current` / the error).
    last: std::sync::Mutex<Option<(u64, String)>>,
}

impl Renewal {
    fn record(&self, result: &Result<RenewalCheck, String>) {
        let outcome = match result {
            Ok(RenewalCheck::Renewed) => "renewed".to_string(),
            Ok(RenewalCheck::Current { .. }) => "current".to_string(),
            Err(e) => e.clone(),
        };
        *self.last.lock().unwrap_or_else(|e| e.into_inner()) = Some((unix_now().max(0) as u64, outcome));
    }
}

pub(crate) fn renewal() -> &'static Renewal {
    static RENEWAL: std::sync::OnceLock<Renewal> = std::sync::OnceLock::new();
    RENEWAL.get_or_init(|| Renewal {
        running: std::sync::atomic::AtomicBool::new(false),
        now: tokio::sync::Notify::new(),
        last: std::sync::Mutex::new(None),
    })
}

/// Ask the running renewal loop to order a new certificate now; `false`
/// when no loop runs (ACME isn't configured).
pub fn request_renewal() -> bool {
    let renewal = renewal();
    if !renewal.running.load(std::sync::atomic::Ordering::SeqCst) {
        return false;
    }
    renewal.now.notify_one();
    true
}

/// Whether the renewal loop runs, and its last outcome `(time, outcome)`.
pub fn renewal_status() -> (bool, Option<(u64, String)>) {
    let renewal = renewal();
    (renewal.running.load(std::sync::atomic::Ordering::SeqCst), renewal.last.lock().unwrap_or_else(|e| e.into_inner()).clone())
}

/// Outcome of [`AcmeManager::check`].
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RenewalCheck {
    /// A certificate was issued; the server should reload it.
    Renewed,
    /// Nothing to do yet; check again in this many seconds.
    Current { recheck_in: u64 },
}

/// How often to ask again for renewal information without a `Retry-After`.
const RENEWAL_INFO_RECHECK_SECS: u64 = 6 * 3600;

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// When to renew `info` (Unix seconds). The fallback is the two-thirds point
/// of its lifetime — Let's Encrypt's advice, and right for 6-day as for
/// 90-day certificates — or `renew_before_days` before expiry when that is
/// set. The CA's suggested window (`(start, end)`) can only bring renewal
/// earlier: a point in it, spread per certificate so a fleet doesn't renew
/// at the same second, wins when it's sooner. A window that has passed
/// means now.
/// The configured domains a certificate with the DNS names `have` doesn't
/// cover (DNS names compare case-insensitively; a wildcard only covers the
/// same wildcard, as ordered). Unreadable names (`None`) cover nothing.
pub(crate) fn missing_names(domains: &[String], have: Option<&[String]>) -> Vec<String> {
    let have = have.unwrap_or(&[]);
    domains
        .iter()
        .filter(|d| !have.iter().any(|h| h.eq_ignore_ascii_case(d.trim_end_matches('.'))))
        .cloned()
        .collect()
}

pub(crate) fn renewal_time(info: &crypto::CertInfo, window: Option<(i64, i64)>, renew_before_days: Option<i64>) -> i64 {
    let lifetime = (info.not_after - info.not_before).max(0);
    let fallback = match renew_before_days {
        Some(days) => info.not_after - days.saturating_mul(86_400),
        None => info.not_before + lifetime / 3 * 2,
    };
    match window {
        Some((start, end)) => {
            let span = (end - start).max(0);
            let point = start + (span as f64 * window_fraction(info)) as i64;
            point.min(fallback)
        }
        None => fallback,
    }
}

/// A fraction in `[0, 1)`, fixed per certificate (FNV-1a of its identity):
/// the same point in the window at every check, a different one per
/// certificate.
fn window_fraction(info: &crypto::CertInfo) -> f64 {
    let identity = info.cert_id.clone().unwrap_or_else(|| info.not_after.to_string());
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in identity.bytes() {
        hash = (hash ^ byte as u64).wrapping_mul(0x0100_0000_01b3);
    }
    (hash >> 11) as f64 / (1u64 << 53) as f64
}

/// `suggestedWindow.start` / `.end` of a renewalInfo response, as Unix
/// seconds; `None` when missing or malformed.
pub(crate) fn parse_suggested_window(body: &str) -> Option<(i64, i64)> {
    let window = &body[body.find("\"suggestedWindow\"")?..];
    let start = parse_rfc3339(&json_str(window, "start")?)?;
    let end = parse_rfc3339(&json_str(window, "end")?)?;
    (start <= end).then_some((start, end))
}

/// An RFC 3339 timestamp (`2026-10-26T12:00:00Z`, fractional seconds and
/// `±hh:mm` offsets allowed) as Unix seconds.
pub(crate) fn parse_rfc3339(value: &str) -> Option<i64> {
    let b = value.trim().as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || !matches!(b[10], b'T' | b't') || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let num = |range: std::ops::Range<usize>| -> Option<i64> {
        let digits = b.get(range)?;
        digits.iter().all(u8::is_ascii_digit).then(|| digits.iter().fold(0i64, |n, d| n * 10 + (d - b'0') as i64))
    };
    let (year, month, day) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (hour, minute, second) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let mut rest = &b[19..];
    if rest.first() == Some(&b'.') {
        let digits = rest[1..].iter().take_while(|c| c.is_ascii_digit()).count();
        if digits == 0 {
            return None;
        }
        rest = &rest[1 + digits..];
    }
    let offset = match rest {
        [b'Z' | b'z'] => 0,
        [sign @ (b'+' | b'-'), h1, h2, b':', m1, m2] => {
            let digits = [*h1, *h2, *m1, *m2];
            if !digits.iter().all(u8::is_ascii_digit) {
                return None;
            }
            let minutes = ((h1 - b'0') as i64 * 10 + (h2 - b'0') as i64) * 60 + (m1 - b'0') as i64 * 10 + (m2 - b'0') as i64;
            if *sign == b'+' { minutes * 60 } else { -minutes * 60 }
        }
        _ => return None,
    };
    let days = crypto::days_since_epoch(year, month, day)?;
    Some(days * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

/// The keys of the JSON object at `field` (e.g. a directory's
/// `meta.profiles`), skipping nested values; empty when absent.
pub(crate) fn json_object_keys(json: &str, field: &str) -> Vec<String> {
    let key = format!("\"{}\"", field);
    let Some(at) = json.find(&key) else { return vec![] };
    let rest = json[at + key.len()..].trim_start_matches([' ', ':', '\n', '\r', '\t']);
    if !rest.starts_with('{') {
        return vec![];
    }
    let mut keys = Vec::new();
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    let mut current = String::new();
    let mut expecting_key = true;
    for c in rest.chars() {
        if in_string {
            if escaped {
                escaped = false;
                current.push(c);
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            } else {
                current.push(c);
            }
            continue;
        }
        match c {
            '"' => { in_string = true; current.clear(); }
            ':' if depth == 1 && expecting_key => { keys.push(std::mem::take(&mut current)); expecting_key = false; }
            ',' if depth == 1 => expecting_key = true,
            '{' | '[' => depth += 1,
            '}' | ']' => {
                depth -= 1;
                if depth == 0 { break; }
            }
            _ => {}
        }
    }
    keys
}

pub(crate) fn json_str(json: &str, field: &str) -> Option<String> {
    let key = format!("\"{}\"", field);
    let start = json.find(&key)? + key.len();
    let rest = json[start..].trim_start_matches([' ', ':'].as_ref()).trim_start();
    let inner = rest.strip_prefix('"')?;
    // Read up to the closing quote, unescaping \" and \\ — CA error details
    // (e.g. Pebble's `Get \"http://...\": connection refused`) contain them.
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

fn json_array_strings(json: &str, field: &str) -> Vec<String> {
    let key = format!("\"{}\"", field);
    let start = match json.find(&key) {
        Some(s) => s + key.len(),
        None => return vec![],
    };
    let rest = json[start..].trim_start_matches([' ', ':'].as_ref()).trim_start();
    if !rest.starts_with('[') { return vec![]; }
    let arr_end = rest.find(']').unwrap_or(rest.len());
    let arr = &rest[1..arr_end];
    let mut result = Vec::new();
    let mut pos = 0;
    while let Some(q) = arr[pos..].find('"') {
        let abs = pos + q + 1;
        if let Some(end) = arr[abs..].find('"') {
            result.push(arr[abs..abs + end].to_string());
            pos = abs + end + 1;
        } else {
            break;
        }
    }
    result
}

fn parse_challenges(json: &str) -> Vec<Challenge> {
    json_objects(json, "challenges")
        .into_iter()
        .filter_map(|obj| {
            Some(Challenge {
                challenge_type: json_str(obj, "type")?,
                url: json_str(obj, "url")?,
                token: json_str(obj, "token").unwrap_or_default(),
                issuer_domain_names: json_array_strings(obj, "issuer-domain-names"),
            })
        })
        .collect()
}

/// The objects in the array `field` (the first one in `json`), as slices.
pub(crate) fn json_objects<'a>(json: &'a str, field: &str) -> Vec<&'a str> {
    let marker = format!("\"{field}\"");
    let start = match json.find(&marker) {
        Some(s) => s + marker.len(),
        None => return vec![],
    };
    let rest = json[start..].trim_start_matches([' ', ':'].as_ref()).trim_start();
    if !rest.starts_with('[') { return vec![]; }

    let mut result = Vec::new();
    let mut pos = 1usize; // skip '['
    while pos < rest.len() {
        // The next object, unless the array ends first.
        let ahead = rest[pos..].trim_start_matches([' ', '\n', '\r', '\t', ',']);
        if !ahead.starts_with('{') { break; }
        let abs = rest.len() - ahead.len();
        // Find the matching closing brace.
        let mut depth = 0usize;
        let mut obj_end = abs;
        for (i, ch) in rest[abs..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 { obj_end = abs + i + 1; break; }
                }
                _ => {}
            }
        }
        if obj_end <= abs { break; }
        result.push(&rest[abs..obj_end]);
        pos = obj_end;
    }
    result
}
