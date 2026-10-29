//! TLS metrics: what each handshake negotiated, and when the served
//! certificates expire.
//!
//! - `rws_tls_handshakes_total{tls_protocol_version,tls_cipher,tls_group}` —
//!   completed handshakes on the TLS-over-TCP listener (HTTP/1.1 and HTTP/2),
//!   by protocol version, cipher suite and key-exchange group (a post-quantum
//!   hybrid such as `X25519MLKEM768`, or a classical group). HTTP/3 handshakes
//!   aren't counted: quinn doesn't expose what its TLS session negotiated.
//! - `rws_tls_handshake_failures_total` — handshakes that failed (timeouts not
//!   included).
//! - `rws_tls_certificate_expiry_timestamp_seconds{name}` — the leaf
//!   certificate's notAfter, Unix seconds, per configured name (`default` for
//!   the default certificate, the virtual host's domain, an on-demand name);
//!   updated whenever certificates are (re)loaded.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

type Negotiated = (&'static str, &'static str, &'static str);

fn handshakes() -> &'static Mutex<HashMap<Negotiated, u64>> {
    static HANDSHAKES: OnceLock<Mutex<HashMap<Negotiated, u64>>> = OnceLock::new();
    HANDSHAKES.get_or_init(|| Mutex::new(HashMap::new()))
}

static FAILURES: AtomicU64 = AtomicU64::new(0);

/// Configured certificates (replaced as a whole on every load) and on-demand
/// ones (added as they're issued).
#[derive(Default)]
struct Expiries {
    configured: BTreeMap<String, i64>,
    on_demand: BTreeMap<String, i64>,
}

fn expiries() -> &'static Mutex<Expiries> {
    static EXPIRIES: OnceLock<Mutex<Expiries>> = OnceLock::new();
    EXPIRIES.get_or_init(|| Mutex::new(Expiries::default()))
}

/// Count a completed handshake by what it negotiated.
pub fn record_handshake(protocol: &'static str, cipher: &'static str, group: &'static str) {
    let mut map = handshakes().lock().unwrap_or_else(|e| e.into_inner());
    *map.entry((protocol, cipher, group)).or_insert(0) += 1;
}

/// Count a failed handshake.
pub fn record_handshake_failure() {
    FAILURES.fetch_add(1, Ordering::Relaxed);
}

/// Record what a finished rustls server handshake negotiated.
#[cfg(feature = "http2")]
pub(crate) fn record_rustls(conn: &rustls::ServerConnection) {
    let protocol = match conn.protocol_version() {
        Some(rustls::ProtocolVersion::TLSv1_3) => "1.3",
        Some(rustls::ProtocolVersion::TLSv1_2) => "1.2",
        Some(other) => other.as_str().unwrap_or("unknown"),
        None => "unknown",
    };
    let cipher = conn.negotiated_cipher_suite().and_then(|s| s.suite().as_str()).unwrap_or("unknown");
    let group = conn.negotiated_key_exchange_group().and_then(|g| g.name().as_str()).unwrap_or("unknown");
    record_handshake(protocol, cipher, group);
}

/// Replace the configured certificates' expiry times: `(name, leaf DER)`.
/// A certificate that can't be parsed is left out.
pub fn set_configured_certificates<'a>(certificates: impl IntoIterator<Item = (String, &'a [u8])>) {
    let configured = certificates
        .into_iter()
        .filter_map(|(name, der)| Some((name, crate::core::x509::cert_info_from_der(der)?.not_after)))
        .collect();
    expiries().lock().unwrap_or_else(|e| e.into_inner()).configured = configured;
}

/// Record an on-demand certificate's expiry time.
pub fn set_on_demand_certificate(name: &str, der: &[u8]) {
    if let Some(info) = crate::core::x509::cert_info_from_der(der) {
        expiries().lock().unwrap_or_else(|e| e.into_inner()).on_demand.insert(name.to_string(), info.not_after);
    }
}

/// Every certificate rws serves: `(name, notAfter as Unix seconds, source)`,
/// source `"configured"` (files, vhosts, ACME) or `"on_demand"`.
pub(crate) fn certificates() -> Vec<(String, i64, &'static str)> {
    let expiries = expiries().lock().unwrap_or_else(|e| e.into_inner());
    let configured = expiries.configured.iter().map(|(n, t)| (n.clone(), *t, "configured"));
    configured.chain(expiries.on_demand.iter().map(|(n, t)| (n.clone(), *t, "on_demand"))).collect()
}

pub(crate) fn prometheus_text() -> String {
    let mut out = String::new();
    let mut map: Vec<(Negotiated, u64)> = handshakes().lock().unwrap_or_else(|e| e.into_inner()).iter().map(|(k, v)| (*k, *v)).collect();
    let failures = FAILURES.load(Ordering::Relaxed);
    if !map.is_empty() || failures > 0 {
        map.sort();
        out.push_str("# HELP rws_tls_handshakes_total Completed TLS handshakes by protocol version, cipher suite and key-exchange group\n");
        out.push_str("# TYPE rws_tls_handshakes_total counter\n");
        for ((protocol, cipher, group), n) in map {
            out.push_str(&format!(
                "rws_tls_handshakes_total{{tls_protocol_version=\"{protocol}\",tls_cipher=\"{cipher}\",tls_group=\"{group}\"}} {n}\n"
            ));
        }
        out.push_str("# HELP rws_tls_handshake_failures_total TLS handshakes that failed\n");
        out.push_str("# TYPE rws_tls_handshake_failures_total counter\n");
        out.push_str(&format!("rws_tls_handshake_failures_total {failures}\n"));
    }
    let expiries = expiries().lock().unwrap_or_else(|e| e.into_inner());
    let all: BTreeMap<&String, &i64> = expiries.on_demand.iter().chain(expiries.configured.iter()).collect();
    if !all.is_empty() {
        out.push_str("# HELP rws_tls_certificate_expiry_timestamp_seconds When each served certificate expires (notAfter), Unix seconds\n");
        out.push_str("# TYPE rws_tls_certificate_expiry_timestamp_seconds gauge\n");
        for (name, not_after) in all {
            out.push_str(&format!(
                "rws_tls_certificate_expiry_timestamp_seconds{{name=\"{}\"}} {not_after}\n",
                name.replace('\\', "\\\\").replace('"', "\\\"")
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests;
