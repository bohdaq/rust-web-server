//! What the access log may say about a client (18.33.0, #102): IP
//! anonymisation, header redaction and query-string redaction.
//!
//! All off by default, and then the access log is byte for byte what it was:
//! [`LogPrivacy::is_off`] is checked first and the old code runs.
//!
//! - **`RWS_CONFIG_ACCESS_LOG_ANONYMIZE_IP`** — `off` (default), `truncate`
//!   (IPv4 to its /24, IPv6 to its /48: `203.0.113.0`, `2001:db8:1::`, as
//!   nginx's `$remote_addr` maps and Google Analytics do) or `hash` (a
//!   16-hex-digit HMAC-SHA256 of the address under
//!   `RWS_CONFIG_ACCESS_LOG_IP_HASH_SECRET`: the same client gets the same
//!   pseudonym, which can't be turned back into the address without the
//!   key; with no key `hash` truncates instead).
//! - **`RWS_CONFIG_LOG_REDACT_HEADERS`** — header names whose values are
//!   written as `REDACTED` wherever a header value is logged. `Authorization`,
//!   `Proxy-Authorization`, `Cookie` and `Set-Cookie` always are.
//! - **`RWS_CONFIG_LOG_REDACT_QUERY`** — query parameter names (`token,api_key`)
//!   whose values are written as `REDACTED` in logged request targets and
//!   `Referer`s.

use std::fmt;
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// `RWS_CONFIG_ACCESS_LOG_ANONYMIZE_IP`: `off`, `truncate` or `hash`.
pub const RWS_CONFIG_ACCESS_LOG_ANONYMIZE_IP: &str = "RWS_CONFIG_ACCESS_LOG_ANONYMIZE_IP";
/// `RWS_CONFIG_ACCESS_LOG_IP_HASH_SECRET`: the HMAC key for `hash`.
pub const RWS_CONFIG_ACCESS_LOG_IP_HASH_SECRET: &str = "RWS_CONFIG_ACCESS_LOG_IP_HASH_SECRET";
/// `RWS_CONFIG_LOG_REDACT_HEADERS`: more header names to redact.
pub const RWS_CONFIG_LOG_REDACT_HEADERS: &str = "RWS_CONFIG_LOG_REDACT_HEADERS";
/// `RWS_CONFIG_LOG_REDACT_QUERY`: query parameter names to redact.
pub const RWS_CONFIG_LOG_REDACT_QUERY: &str = "RWS_CONFIG_LOG_REDACT_QUERY";

/// What a redacted value is written as.
pub const REDACTED: &str = "REDACTED";

/// Headers whose values are never logged, whatever the settings.
pub const ALWAYS_REDACTED_HEADERS: [&str; 4] = ["authorization", "proxy-authorization", "cookie", "set-cookie"];

/// Set when the last configuration built had anything on: code that would
/// otherwise not look at the configuration per request (`LogLayer`) checks
/// this first.
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Whether the last [`LogPrivacy::from_env`] turned anything on.
#[doc(hidden)]
pub fn maybe_active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// How client addresses are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AnonymizeIp {
    /// As they are.
    #[default]
    Off,
    /// IPv4 /24, IPv6 /48; the rest zeroed.
    Truncate,
    /// A keyed hash: 16 hex digits of HMAC-SHA256(key, address).
    Hash,
}

impl AnonymizeIp {
    /// `off`/`false`/`none`/empty, `truncate`, `hash`; `None` for anything else.
    pub fn parse(value: &str) -> Option<AnonymizeIp> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "off" | "false" | "none" => Some(AnonymizeIp::Off),
            "truncate" | "true" | "on" => Some(AnonymizeIp::Truncate),
            "hash" => Some(AnonymizeIp::Hash),
            _ => None,
        }
    }
}

/// The access log's privacy settings (see the module docs). The default
/// is everything off.
#[derive(Clone, Default, PartialEq)]
pub struct LogPrivacy {
    /// How client addresses are written.
    pub anonymize_ip: AnonymizeIp,
    /// The HMAC key for [`AnonymizeIp::Hash`] (empty: truncate instead).
    pub ip_hash_key: Arc<[u8]>,
    /// Lower-case header names redacted besides [`ALWAYS_REDACTED_HEADERS`].
    pub redact_headers: Vec<String>,
    /// Query parameter names whose values are redacted (matched ignoring
    /// ASCII case).
    pub redact_query: Vec<String>,
}

/// The key is never printed.
impl fmt::Debug for LogPrivacy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LogPrivacy")
            .field("anonymize_ip", &self.anonymize_ip)
            .field("ip_hash_key", &if self.ip_hash_key.is_empty() { "(none)" } else { "(set)" })
            .field("redact_headers", &self.redact_headers)
            .field("redact_query", &self.redact_query)
            .finish()
    }
}

fn names(value: &str) -> Vec<String> {
    value.split(',').map(|n| n.trim().to_ascii_lowercase()).filter(|n| !n.is_empty()).collect()
}

impl LogPrivacy {
    /// From the `RWS_CONFIG_*` variables above; an unknown
    /// `ACCESS_LOG_ANONYMIZE_IP` value truncates (fails towards privacy —
    /// `rws --check` reports it).
    pub fn from_env() -> LogPrivacy {
        let read = |key: &str| std::env::var(key).unwrap_or_default();
        let privacy = LogPrivacy {
            anonymize_ip: AnonymizeIp::parse(&read(RWS_CONFIG_ACCESS_LOG_ANONYMIZE_IP)).unwrap_or(AnonymizeIp::Truncate),
            ip_hash_key: read(RWS_CONFIG_ACCESS_LOG_IP_HASH_SECRET).trim().as_bytes().into(),
            redact_headers: names(&read(RWS_CONFIG_LOG_REDACT_HEADERS)),
            redact_query: names(&read(RWS_CONFIG_LOG_REDACT_QUERY)),
        };
        ACTIVE.store(!privacy.is_off(), Ordering::Relaxed);
        privacy
    }

    /// Nothing on: logs are written exactly as before.
    pub fn is_off(&self) -> bool {
        self.anonymize_ip == AnonymizeIp::Off && self.redact_headers.is_empty() && self.redact_query.is_empty()
    }

    /// Write `ip` as configured.
    pub fn write_ip(&self, out: &mut Vec<u8>, ip: IpAddr) {
        use std::io::Write;
        match self.anonymize_ip {
            AnonymizeIp::Off => {
                let _ = write!(out, "{ip}");
            }
            AnonymizeIp::Hash if !self.ip_hash_key.is_empty() => {
                let text = ip.to_canonical().to_string();
                let mac = crate::core::sha256::hmac_sha256(&self.ip_hash_key, text.as_bytes());
                const HEX: &[u8; 16] = b"0123456789abcdef";
                for b in &mac[..8] {
                    out.push(HEX[(b >> 4) as usize]);
                    out.push(HEX[(b & 15) as usize]);
                }
            }
            AnonymizeIp::Truncate | AnonymizeIp::Hash => {
                let _ = write!(out, "{}", truncate_ip(ip));
            }
        }
    }

    /// `ip` as configured, as a `String`.
    pub fn ip_string(&self, ip: IpAddr) -> String {
        let mut out = Vec::with_capacity(40);
        self.write_ip(&mut out, ip);
        String::from_utf8(out).unwrap_or_default()
    }

    /// Whether the value of header `name` is redacted.
    pub fn redacts_header(&self, name: &str) -> bool {
        ALWAYS_REDACTED_HEADERS.iter().any(|h| h.eq_ignore_ascii_case(name))
            || self.redact_headers.iter().any(|h| h.eq_ignore_ascii_case(name))
    }

    /// `target` (a request target or URL) with the values of the
    /// [`redact_query`](Self::redact_query) parameters replaced by
    /// [`REDACTED`], passed to `push` piece by piece (so a caller can escape
    /// as it writes). Without such parameters `target` goes through whole.
    pub fn redact_target<'a>(&self, target: &'a str, mut push: impl FnMut(&'a str)) {
        let Some(q) = target.find('?').filter(|_| !self.redact_query.is_empty()) else {
            push(target);
            return;
        };
        let (head, rest) = target.split_at(q + 1);
        let (query, fragment) = match rest.find('#') {
            Some(h) => rest.split_at(h),
            None => (rest, ""),
        };
        push(head);
        for (i, pair) in query.split('&').enumerate() {
            if i > 0 {
                push("&");
            }
            match pair.split_once('=') {
                Some((name, _)) if self.redact_query.iter().any(|n| n.eq_ignore_ascii_case(name)) => {
                    push(name);
                    push("=");
                    push(REDACTED);
                }
                _ => push(pair),
            }
        }
        push(fragment);
    }

    /// [`redact_target`](Self::redact_target) as a `String`.
    pub fn redacted_target(&self, target: &str) -> String {
        let mut out = String::with_capacity(target.len());
        self.redact_target(target, |piece| out.push_str(piece));
        out
    }
}

/// IPv4 to its /24, IPv6 to its /48 (an IPv4-mapped IPv6 address as the
/// IPv4 address it maps).
pub fn truncate_ip(ip: IpAddr) -> IpAddr {
    match ip.to_canonical() {
        IpAddr::V4(v4) => {
            let [a, b, c, _] = v4.octets();
            IpAddr::from([a, b, c, 0])
        }
        IpAddr::V6(v6) => {
            let s = v6.segments();
            IpAddr::from([s[0], s[1], s[2], 0, 0, 0, 0, 0])
        }
    }
}

#[cfg(test)]
mod tests;
