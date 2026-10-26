//! Reload when the config file or a certificate changes (`watch = true`, #10).
//!
//! A background thread polls the size and modification time of
//! `rws.config.toml` (or `RWS_CONFIG_FILE`) and of every configured TLS
//! certificate and key once a second. A change that looks the same on two
//! polls in a row — an editor or `certbot` finished writing — triggers the
//! same validated reload as `SIGHUP` ([`crate::config_reload::request_reload`]):
//! a broken config is rejected and the running one keeps serving. The set of
//! files is re-read on every poll, so a reload that points at new
//! certificates starts watching them. Off by default, like Caddy's `--watch`.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// `RWS_CONFIG_WATCH`: `true` to reload when a watched file changes.
pub const RWS_CONFIG_WATCH: &str = "RWS_CONFIG_WATCH";

/// How often the files are checked.
pub const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Whether `RWS_CONFIG_WATCH` is on.
pub fn enabled() -> bool {
    std::env::var(RWS_CONFIG_WATCH).is_ok_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on"))
}

/// The files whose change means a reload: the config file and every
/// configured certificate and key (the default pair and each virtual host's).
pub fn watched_paths() -> Vec<PathBuf> {
    let var = |key: &str| std::env::var(key).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    let mut paths = vec![PathBuf::from(var("RWS_CONFIG_FILE").unwrap_or_else(|| "rws.config.toml".to_string()))];
    for key in [crate::entry_point::Config::RWS_CONFIG_TLS_CERT_FILE, crate::entry_point::Config::RWS_CONFIG_TLS_KEY_FILE] {
        paths.extend(var(key).map(PathBuf::from));
    }
    for i in 0.. {
        if var(&format!("RWS_CONFIG_VIRTUAL_HOST_{i}_DOMAIN")).is_none() {
            break;
        }
        for suffix in ["CERT_FILE", "KEY_FILE"] {
            paths.extend(var(&format!("RWS_CONFIG_VIRTUAL_HOST_{i}_{suffix}")).map(PathBuf::from));
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

/// Size and modification time of each file (`None` while it doesn't exist).
type Snapshot = Vec<(PathBuf, Option<(u64, SystemTime)>)>;

fn snapshot(paths: Vec<PathBuf>) -> Snapshot {
    paths
        .into_iter()
        .map(|path| {
            let state = std::fs::metadata(&path).ok().and_then(|md| Some((md.len(), md.modified().ok()?)));
            (path, state)
        })
        .collect()
}

/// Change detection, one [`Watcher::poll`] per interval.
pub struct Watcher<F: FnMut() -> Vec<PathBuf>> {
    paths: F,
    applied: Snapshot,
    pending: Option<Snapshot>,
}

impl<F: FnMut() -> Vec<PathBuf>> Watcher<F> {
    /// A watcher of the files `paths` lists (asked again on every poll),
    /// taking their current state as the applied one.
    pub fn new(mut paths: F) -> Self {
        let applied = snapshot(paths());
        Watcher { paths, applied, pending: None }
    }

    /// `true` when the files changed and have stayed the same since the
    /// previous poll: time to reload.
    pub fn poll(&mut self) -> bool {
        let now = snapshot((self.paths)());
        if now == self.applied {
            self.pending = None;
            return false;
        }
        if self.pending.as_ref() == Some(&now) {
            self.applied = now;
            self.pending = None;
            return true;
        }
        self.pending = Some(now);
        false
    }
}

/// Start the watcher thread when `RWS_CONFIG_WATCH` is on; the `rws` binary
/// calls it after reading its configuration.
pub fn start() {
    if !enabled() {
        return;
    }
    let spawned = std::thread::Builder::new().name("rws-config-watch".to_string()).spawn(|| {
        let mut watcher = Watcher::new(watched_paths);
        loop {
            std::thread::sleep(POLL_INTERVAL);
            if watcher.poll() {
                println!("config watch: a watched file changed; reloading");
                crate::config_reload::request_reload();
            }
        }
    });
    if let Err(e) = spawned {
        eprintln!("config watch: could not start: {e}");
    }
}

#[cfg(test)]
mod tests;
