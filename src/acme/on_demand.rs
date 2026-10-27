//! On-demand TLS (`[acme] on_demand = true`): a certificate is ordered at the
//! first TLS handshake for a name nobody configured — a customer's own domain
//! pointed at a SaaS — once the `on_demand_ask` endpoint allows it
//! (`GET <ask>?domain=<name>` answering 2xx).
//!
//! The TLS listener reads the ClientHello first ([`ensure`], before the
//! handshake goes on), so the new certificate is in the store when the
//! resolver ([`certificate`]) looks. Certificates live in
//! `<state dir>/on-demand/<name>/{cert,key}.pem` (and, with `[acme] redis`,
//! in Redis like any other), are loaded at startup and renewed by one
//! background loop. Bounds: the ask endpoint decides which names; a refused
//! name is not asked about again for a minute, a failed order not retried
//! for ten; at most [`MAX_CONCURRENT_ORDERS`] orders run at once and a
//! handshake waits at most [`HANDSHAKE_WAIT`] for its certificate.
//! HTTP-01 and TLS-ALPN-01 only: DNS-01 would need the customers' zones.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};

use rustls::sign::CertifiedKey;

use super::{AcmeConfig, AcmeManager, RenewalCheck};
use crate::entry_point::Config;

/// Orders running at once.
pub const MAX_CONCURRENT_ORDERS: usize = 4;
/// How long a handshake waits for its certificate; the order goes on after.
pub const HANDSHAKE_WAIT: Duration = Duration::from_secs(90);
/// A name the ask endpoint refused isn't asked about again for this long.
const REFUSED_FOR: Duration = Duration::from_secs(60);
/// A name whose order failed isn't ordered again for this long.
const FAILED_FOR: Duration = Duration::from_secs(10 * 60);
const ASK_TIMEOUT_MS: u64 = 5_000;

struct OnDemand {
    base: AcmeConfig,
    ask: String,
    dir: PathBuf,
    /// Names with a configured certificate: never ordered on demand.
    configured: HashSet<String>,
    certificates: RwLock<HashMap<String, Arc<CertifiedKey>>>,
    /// Names refused or failed, until when.
    held_off: Mutex<HashMap<String, Instant>>,
    /// One order per name at a time.
    ordering: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    orders: tokio::sync::Semaphore,
}

static ON_DEMAND: OnceLock<OnDemand> = OnceLock::new();

/// Whether `[acme] on_demand = true` is set (with domains: the listener
/// needs a certificate of its own).
pub fn configured() -> bool {
    super::non_empty_env(Config::RWS_CONFIG_ACME_ON_DEMAND).is_some_and(|v| v.eq_ignore_ascii_case("true"))
        && super::non_empty_env(Config::RWS_CONFIG_ACME_DOMAINS).is_some()
}

/// Whether on-demand TLS is running ([`start`] succeeded).
pub fn enabled() -> bool {
    ON_DEMAND.get().is_some()
}

/// Start on-demand TLS from `base` (the `[acme]` settings): load the
/// certificates ordered before and spawn their renewal loop. Refuses (and
/// says why) without an ask endpoint or with DNS-01.
pub fn start(base: &AcmeConfig) -> Result<(), String> {
    let ask = super::non_empty_env(Config::RWS_CONFIG_ACME_ON_DEMAND_ASK)
        .ok_or("[acme] on_demand needs on_demand_ask: without it anyone could make rws order certificates for any name")?;
    if matches!(base.challenge, super::ChallengeType::Dns01 | super::ChallengeType::DnsPersist01) {
        return Err(format!("[acme] on_demand works with http-01 and tls-alpn-01, not {}", base.challenge.as_str()));
    }
    let dir = Path::new(&base.cert_path).parent().unwrap_or(Path::new(".")).join("on-demand");
    let mut configured: HashSet<String> = base.domains.iter().map(|d| d.to_ascii_lowercase()).collect();
    configured.extend(crate::entry_point::get_virtual_hosts().into_iter().map(|vh| vh.domain.to_ascii_lowercase()));
    let state = OnDemand {
        base: base.clone(),
        ask,
        dir,
        configured,
        certificates: RwLock::default(),
        held_off: Mutex::default(),
        ordering: Mutex::default(),
        orders: tokio::sync::Semaphore::new(MAX_CONCURRENT_ORDERS),
    };
    let loaded = state.load_all();
    if ON_DEMAND.set(state).is_err() {
        return Ok(());
    }
    println!("[ACME] On-demand TLS: certificates for names {} allows ({loaded} loaded).", ON_DEMAND.get().map_or("", |s| s.ask.as_str()));
    tokio::spawn(renewal_loop());
    Ok(())
}

/// The certificate ordered on demand for `server_name`, if any.
pub fn certificate(server_name: &str) -> Option<Arc<CertifiedKey>> {
    let state = ON_DEMAND.get()?;
    state.certificates.read().unwrap_or_else(|e| e.into_inner()).get(&server_name.to_ascii_lowercase()).cloned()
}

/// Make sure `server_name` has a certificate if it should get one: asks the
/// ask endpoint and orders it, waiting up to [`HANDSHAKE_WAIT`] (the order
/// carries on in the background after that). Returns at once for configured
/// names, names already served, names held off and invalid names.
pub async fn ensure(server_name: &str) {
    let Some(state) = ON_DEMAND.get() else { return };
    let name = server_name.to_ascii_lowercase();
    if !state.wants(&name) {
        return;
    }
    let order = tokio::spawn(obtain(name));
    let _ = tokio::time::timeout(HANDSHAKE_WAIT, order).await;
}

/// Whether `name` may be a DNS name a CA would issue for: letters, digits
/// and hyphens, at least two labels, no wildcard, not an IP address.
pub fn valid_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 253 || !name.contains('.') {
        return false;
    }
    let labels: Vec<&str> = name.split('.').collect();
    let label_ok = |l: &&str| {
        !l.is_empty() && l.len() <= 63 && !l.starts_with('-') && !l.ends_with('-')
            && l.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    };
    labels.iter().all(label_ok) && !labels.last().is_some_and(|tld| tld.bytes().all(|b| b.is_ascii_digit()))
}

impl OnDemand {
    fn wants(&self, name: &str) -> bool {
        if self.configured.contains(name) || !valid_name(name) {
            return false;
        }
        if self.certificates.read().unwrap_or_else(|e| e.into_inner()).contains_key(name) {
            return false;
        }
        let mut held_off = self.held_off.lock().unwrap_or_else(|e| e.into_inner());
        match held_off.get(name) {
            Some(until) if *until > Instant::now() => false,
            Some(_) => {
                held_off.remove(name);
                true
            }
            None => true,
        }
    }

    fn hold_off(&self, name: &str, for_: Duration) {
        let mut held_off = self.held_off.lock().unwrap_or_else(|e| e.into_inner());
        // Bounded: expired entries go when it grows.
        if held_off.len() >= 10_000 {
            let now = Instant::now();
            held_off.retain(|_, until| *until > now);
        }
        held_off.insert(name.to_string(), Instant::now() + for_);
    }

    fn config_for(&self, name: &str) -> AcmeConfig {
        let dir = self.dir.join(name);
        AcmeConfig {
            domains: vec![name.to_string()],
            cert_path: dir.join("cert.pem").to_string_lossy().into_owned(),
            key_path: dir.join("key.pem").to_string_lossy().into_owned(),
            ..self.base.clone()
        }
    }

    /// Put `name`'s certificate files in the store; `false` if unloadable.
    fn install(&self, name: &str) -> bool {
        let config = self.config_for(name);
        match crate::tls::load_certified_key(&config.cert_path, &config.key_path) {
            Ok(key) => {
                self.certificates.write().unwrap_or_else(|e| e.into_inner()).insert(name.to_string(), Arc::new(key));
                true
            }
            Err(e) => {
                eprintln!("[ACME] On-demand certificate for '{name}' couldn't be loaded: {e}");
                false
            }
        }
    }

    /// Load every certificate ordered before; returns how many.
    fn load_all(&self) -> usize {
        let Ok(entries) = std::fs::read_dir(&self.dir) else { return 0 };
        entries
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| valid_name(name) && !self.configured.contains(name))
            .filter(|name| self.install(name))
            .count()
    }

    async fn allowed(&self, name: &str) -> bool {
        let separator = if self.ask.contains('?') { '&' } else { '?' };
        let url = format!("{}{separator}domain={name}", self.ask);
        match crate::http_client::AsyncClient::new().max_redirects(0).get(&url).timeout_ms(ASK_TIMEOUT_MS).send().await {
            Ok(response) if response.is_success() => true,
            Ok(response) => {
                println!("[ACME] On-demand: the ask endpoint refused '{name}' ({}).", response.status());
                false
            }
            Err(e) => {
                eprintln!("[ACME] On-demand: the ask endpoint failed for '{name}': {e}");
                false
            }
        }
    }
}

/// Ask about `name` and order its certificate (one order per name at a time).
async fn obtain(name: String) {
    let Some(state) = ON_DEMAND.get() else { return };
    let lock = {
        let mut ordering = state.ordering.lock().unwrap_or_else(|e| e.into_inner());
        Arc::clone(ordering.entry(name.clone()).or_default())
    };
    let _one = lock.lock().await;
    let result = obtain_locked(state, &name).await;
    state.ordering.lock().unwrap_or_else(|e| e.into_inner()).remove(&name);
    if let Err(e) = result {
        eprintln!("[ACME] On-demand certificate for '{name}' failed: {e} — not retried for {} minutes.", FAILED_FOR.as_secs() / 60);
        state.hold_off(&name, FAILED_FOR);
    }
}

async fn obtain_locked(state: &OnDemand, name: &str) -> Result<(), String> {
    // Another handshake got it while this one waited for the lock.
    if !state.wants(name) {
        return Ok(());
    }
    if !state.allowed(name).await {
        state.hold_off(name, REFUSED_FOR);
        return Ok(());
    }
    let _permit = state.orders.acquire().await.map_err(|e| e.to_string())?;
    println!("[ACME] On-demand: getting a certificate for '{name}'.");
    // `check` installs a certificate another replica stored (`[acme]
    // redis`), keeps a current one from disk, or orders a new one.
    AcmeManager::new(state.config_for(name)).check().await?;
    if state.install(name) {
        Ok(())
    } else {
        Err("the new certificate couldn't be loaded".to_string())
    }
}

/// Renew on-demand certificates: every name is checked each
/// `RWS_CONFIG_ACME_RENEW_CHECK_INTERVAL_SECS` (default 12 hours; an hour
/// at most while one is failing), and reinstalled when renewed — the
/// resolver reads the store at every handshake, so nothing is reloaded.
async fn renewal_loop() {
    let Some(state) = ON_DEMAND.get() else { return };
    let interval = super::renew_check_interval_secs();
    loop {
        let mut next = interval;
        let names: Vec<String> = state.certificates.read().unwrap_or_else(|e| e.into_inner()).keys().cloned().collect();
        for name in names {
            match AcmeManager::new(state.config_for(&name)).check().await {
                Ok(RenewalCheck::Current { recheck_in }) => next = next.min(recheck_in),
                Ok(RenewalCheck::Renewed) => {
                    state.install(&name);
                    next = next.min(60);
                }
                Err(e) => {
                    eprintln!("[ACME] On-demand renewal of '{name}' failed: {e}");
                    next = next.min(3600);
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(next.max(60))).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_names_a_ca_would_issue_for_are_valid() {
        for name in ["example.com", "shop.customer.example", "a-b.example.co", "xn--bcher-kva.example"] {
            assert!(valid_name(name), "{name}");
        }
        for name in ["", "localhost", "*.example.com", "-a.example.com", "a-.example.com", "a..example.com",
            "1.2.3.4", "Example.com", "a_b.example.com", "example.com.", &format!("{}.com", "a".repeat(64))] {
            assert!(!valid_name(name), "{name}");
        }
    }

    fn state(dir: &Path) -> OnDemand {
        let base = AcmeConfig {
            domains: vec!["saas.example".to_string()],
            email: String::new(),
            directory_url: String::new(),
            cert_path: dir.join("cert.pem").to_string_lossy().into_owned(),
            key_path: dir.join("key.pem").to_string_lossy().into_owned(),
            challenge_port: 80,
            renew_before_days: None,
            profile: None,
            account_key_path: String::new(),
            challenge: super::super::ChallengeType::Http01,
            dns: None,
        };
        OnDemand {
            ask: "http://127.0.0.1:1/ask".to_string(),
            dir: dir.join("on-demand"),
            configured: base.domains.iter().cloned().collect(),
            base,
            certificates: RwLock::default(),
            held_off: Mutex::default(),
            ordering: Mutex::default(),
            orders: tokio::sync::Semaphore::new(MAX_CONCURRENT_ORDERS),
        }
    }

    #[test]
    fn configured_held_off_and_served_names_are_not_ordered() {
        let dir = crate::test_env::TempPath::new("rws-on-demand-wants");
        let state = state(&dir);
        assert!(state.wants("shop.customer.example"));
        assert!(!state.wants("saas.example"));
        state.hold_off("shop.customer.example", Duration::from_secs(60));
        assert!(!state.wants("shop.customer.example"));
        state.hold_off("other.customer.example", Duration::ZERO);
        assert!(state.wants("other.customer.example"));
    }

    #[test]
    fn each_name_gets_its_own_files_under_the_state_dir() {
        let dir = crate::test_env::TempPath::new("rws-on-demand-paths");
        let state = state(&dir);
        let config = state.config_for("shop.customer.example");
        assert_eq!(vec!["shop.customer.example".to_string()], config.domains);
        assert!(config.cert_path.ends_with("on-demand/shop.customer.example/cert.pem"), "{}", config.cert_path);
        assert!(config.key_path.ends_with("on-demand/shop.customer.example/key.pem"));
    }

    #[test]
    fn certificates_ordered_before_are_loaded() {
        let dir = crate::test_env::TempPath::new("rws-on-demand-load");
        let state = state(&dir);
        let name = "shop.customer.example";
        let issued = rcgen::generate_simple_self_signed(vec![name.to_string()]).unwrap();
        let files = state.config_for(name);
        std::fs::create_dir_all(Path::new(&files.cert_path).parent().unwrap()).unwrap();
        std::fs::write(&files.cert_path, issued.cert.pem()).unwrap();
        std::fs::write(&files.key_path, issued.signing_key.serialize_pem()).unwrap();
        std::fs::create_dir_all(state.dir.join("not a name")).unwrap();
        assert_eq!(1, state.load_all());
        assert!(!state.wants(name));
    }

    #[tokio::test]
    async fn an_unreachable_ask_endpoint_refuses() {
        let dir = crate::test_env::TempPath::new("rws-on-demand-ask");
        let state = state(&dir);
        assert!(!state.allowed("shop.customer.example").await);
    }
}
