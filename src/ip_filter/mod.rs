//! IP address allowlist and denylist middleware.
//!
//! [`IpFilter`] inspects the client IP from [`crate::server::ConnectionInfo`]
//! and either allows or blocks the request based on a list of IPv4 or IPv6
//! addresses and CIDR ranges. An IPv4 rule also matches the same client
//! connecting over a dual-stack socket as an IPv4-mapped IPv6 address
//! (`::ffff:1.2.3.4`).
//!
//! - **Allow mode** — only listed addresses/ranges pass; all others get `403 Forbidden`.
//! - **Deny mode** — listed addresses/ranges get `403 Forbidden`; all others pass.
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::ip_filter::IpFilter;
//!
//! // Restrict to internal networks only.
//! let app = App::new()
//!     .wrap(IpFilter::allow(["10.0.0.0/8", "192.168.0.0/16", "127.0.0.1"]));
//!
//! // Block a known-bad address range.
//! let app = App::new()
//!     .wrap(IpFilter::deny(["1.2.3.4", "5.6.7.0/24", "2001:db8::/32"]));
//! ```

#[cfg(test)]
mod tests;

use crate::application::Application;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

enum FilterMode {
    Allow,
    Deny,
}

/// An address range in IPv6 space; IPv4 rules are stored IPv4-mapped
/// (`::ffff:0:0/96`), so one comparison covers both families.
struct IpRange {
    network: u128,
    mask: u128,
}

/// Middleware that filters requests by client IP address.
///
/// Constructed via [`IpFilter::allow`] or [`IpFilter::deny`]. Each entry
/// may be an exact address (`"1.2.3.4"`, `"2001:db8::1"`) or a CIDR range
/// (`"10.0.0.0/8"`, `"2001:db8::/32"`). Malformed entries are skipped; check
/// them with [`is_valid_entry`] (`rws --check` does).
pub struct IpFilter {
    mode: FilterMode,
    ranges: Vec<IpRange>,
}

impl IpFilter {
    /// Create an allowlist filter. Only IPs matching one of `entries` pass;
    /// all others receive `403 Forbidden`.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use rust_web_server::ip_filter::IpFilter;
    ///
    /// let filter = IpFilter::allow(["127.0.0.1", "10.0.0.0/8"]);
    /// ```
    pub fn allow(entries: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        Self::from_entries(FilterMode::Allow, entries)
    }

    /// Create a denylist filter. IPs matching one of `entries` receive
    /// `403 Forbidden`; all others pass.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use rust_web_server::ip_filter::IpFilter;
    ///
    /// let filter = IpFilter::deny(["1.2.3.4", "192.0.2.0/24"]);
    /// ```
    pub fn deny(entries: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        Self::from_entries(FilterMode::Deny, entries)
    }

    fn from_entries(mode: FilterMode, entries: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        let ranges = entries.into_iter().filter_map(|e| IpRange::parse(e.as_ref())).collect();
        IpFilter { mode, ranges }
    }

    fn matches(&self, ip: &str) -> bool {
        match parse_ip(ip) {
            Some(addr) => self.ranges.iter().any(|r| (addr & r.mask) == r.network),
            None => false,
        }
    }
}

/// `true` if `entry` is an address or CIDR range [`IpFilter`] understands.
pub fn is_valid_entry(entry: &str) -> bool {
    IpRange::parse(entry).is_some()
}

impl Middleware for IpFilter {
    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        use crate::error::{AppError, IntoResponse};
        let matched = self.matches(&connection.client.ip);
        match self.mode {
            FilterMode::Allow if !matched => Ok(AppError::Forbidden.into_response()),
            FilterMode::Deny if matched => Ok(AppError::Forbidden.into_response()),
            _ => next.execute(request, connection),
        }
    }

    /// In-memory only: safe on the event loop.
    fn runs_inline(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }
}

impl IpRange {
    fn parse(entry: &str) -> Option<Self> {
        let entry = entry.trim();
        let (addr, prefix_len) = match entry.split_once('/') {
            Some((addr, len)) => {
                let len: u32 = len.parse().ok()?;
                let is_v4 = addr.parse::<std::net::Ipv4Addr>().is_ok();
                let max = if is_v4 { 32 } else { 128 };
                if len > max {
                    return None;
                }
                (addr, if is_v4 { len + 96 } else { len })
            }
            None => (entry, 128),
        };
        let network = parse_ip(addr)?;
        let mask = if prefix_len == 0 { 0 } else { !0u128 << (128 - prefix_len) };
        Some(IpRange { network: network & mask, mask })
    }
}

/// An IPv4 or IPv6 address as a number in IPv6 space (IPv4 → `::ffff:a.b.c.d`).
/// A zone suffix (`fe80::1%eth0`) is ignored.
fn parse_ip(s: &str) -> Option<u128> {
    let s = s.trim().trim_start_matches('[').trim_end_matches(']');
    let s = s.split('%').next()?;
    match s.parse::<std::net::IpAddr>().ok()? {
        std::net::IpAddr::V4(v4) => Some(u128::from(v4.to_ipv6_mapped())),
        std::net::IpAddr::V6(v6) => Some(u128::from(v6)),
    }
}
