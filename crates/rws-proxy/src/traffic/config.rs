//! `[traffic]` settings (`RWS_CONFIG_TRAFFIC_*`).

use std::path::PathBuf;

pub const RWS_CONFIG_TRAFFIC_ENABLED: &str = "RWS_CONFIG_TRAFFIC_ENABLED";
pub const RWS_CONFIG_TRAFFIC_RETENTION_DAYS: &str = "RWS_CONFIG_TRAFFIC_RETENTION_DAYS";
pub const RWS_CONFIG_TRAFFIC_PERSIST: &str = "RWS_CONFIG_TRAFFIC_PERSIST";
pub const RWS_CONFIG_TRAFFIC_EXCLUDE: &str = "RWS_CONFIG_TRAFFIC_EXCLUDE";
pub const RWS_CONFIG_TRAFFIC_GEO_DB: &str = "RWS_CONFIG_TRAFFIC_GEO_DB";

fn read(key: &str) -> String {
    std::env::var(key).unwrap_or_default().trim().to_string()
}

/// `enabled`: `Some(false)` turns counting off even with the admin API on;
/// `None` when unset.
pub fn enabled_setting() -> Option<bool> {
    match read(RWS_CONFIG_TRAFFIC_ENABLED).to_ascii_lowercase().as_str() {
        "" => None,
        "false" | "0" | "no" | "off" => Some(false),
        _ => Some(true),
    }
}

/// `retention_days`: 1–90, default 30.
pub fn retention_days() -> u64 {
    read(RWS_CONFIG_TRAFFIC_RETENTION_DAYS).parse::<u64>().ok().filter(|d| *d >= 1).map_or(30, |d| d.min(90))
}

/// `exclude`: exact paths not counted (comma-separated); default the
/// health, readiness and metrics endpoints.
pub fn exclude() -> Vec<String> {
    let value = std::env::var(RWS_CONFIG_TRAFFIC_EXCLUDE);
    match value {
        Ok(v) => v.split(',').map(str::trim).filter(|p| !p.is_empty()).map(str::to_string).collect(),
        Err(_) => ["/healthz", "/readyz", "/metrics"].iter().map(|p| p.to_string()).collect(),
    }
}

/// `persist`: the file the counts are saved to. Unset: `traffic.json` next
/// to the certificate (the ACME certificate's directory, else the TLS
/// certificate's) — the state directory; without either, nothing is
/// saved. `"off"`: not saved.
pub fn persist_path() -> Option<PathBuf> {
    let value = read(RWS_CONFIG_TRAFFIC_PERSIST);
    if value.eq_ignore_ascii_case("off") {
        return None;
    }
    if !value.is_empty() {
        return Some(PathBuf::from(value));
    }
    [crate::entry_point::Config::RWS_CONFIG_ACME_CERT_PATH, crate::entry_point::Config::RWS_CONFIG_TLS_CERT_FILE]
        .iter()
        .map(|key| read(key))
        .find(|path| !path.is_empty())
        .and_then(|cert| PathBuf::from(cert).parent().map(|dir| dir.join("traffic.json")))
}

/// `geo_db`: a country table (`ci/gen_geo.py`'s format) to use instead of
/// the built-in one; unset: the built-in one.
pub fn geo_db() -> Option<PathBuf> {
    let value = read(RWS_CONFIG_TRAFFIC_GEO_DB);
    (!value.is_empty()).then(|| PathBuf::from(value))
}
