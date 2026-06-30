//! IP address allowlist and denylist middleware.
//!
//! [`IpFilter`] inspects the client IP from [`crate::server::ConnectionInfo`]
//! and either allows or blocks the request based on a list of IPv4 addresses
//! or CIDR ranges. IPv6 addresses are not matched by any rule; their handling
//! depends on the filter mode — they are blocked in allow mode and passed in
//! deny mode.
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
//!     .wrap(IpFilter::deny(["1.2.3.4", "5.6.7.0/24"]));
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

struct IpRange {
    network: u32,
    mask: u32,
}

/// Middleware that filters requests by client IPv4 address.
///
/// Constructed via [`IpFilter::allow`] or [`IpFilter::deny`]. Each entry
/// may be an exact address (`"1.2.3.4"`) or a CIDR range (`"10.0.0.0/8"`).
/// Malformed entries are silently skipped.
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
        let ip_u32 = match parse_ipv4(ip) {
            Some(v) => v,
            None => return false,
        };
        self.ranges.iter().any(|r| (ip_u32 & r.mask) == r.network)
    }
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
}

impl IpRange {
    fn parse(entry: &str) -> Option<Self> {
        if let Some(slash) = entry.find('/') {
            let network_str = &entry[..slash];
            let prefix_len: u8 = entry[slash + 1..].parse().ok()?;
            if prefix_len > 32 {
                return None;
            }
            let raw = parse_ipv4(network_str)?;
            let mask = if prefix_len == 0 { 0u32 } else { !0u32 << (32 - prefix_len) };
            Some(IpRange { network: raw & mask, mask })
        } else {
            let addr = parse_ipv4(entry)?;
            Some(IpRange { network: addr, mask: !0u32 })
        }
    }
}

fn parse_ipv4(s: &str) -> Option<u32> {
    let mut parts = s.split('.');
    let a: u8 = parts.next()?.parse().ok()?;
    let b: u8 = parts.next()?.parse().ok()?;
    let c: u8 = parts.next()?.parse().ok()?;
    let d: u8 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(u32::from_be_bytes([a, b, c, d]))
}
