//! Addresses and CIDR ranges: [`TrustedProxies`] and the range parsing the
//! `IpFilter` middleware (in rust-web-server's `ip_filter`) is built on.

/// An address range in IPv6 space; IPv4 rules are stored IPv4-mapped
/// (`::ffff:0:0/96`), so one comparison covers both families.
#[derive(Debug, Clone, PartialEq)]
#[doc(hidden)]
pub struct IpRange {
    pub network: u128,
    pub mask: u128,
}

/// Peers whose `X-Forwarded-*` / `Forwarded` headers rws believes
/// (`trusted_proxies`, 18.24.0). The default is loopback and the private
/// ranges load balancers usually sit in; `*` trusts everyone (the behaviour
/// before 18.24), `none` no one.
#[derive(Debug, Clone, PartialEq)]
pub enum TrustedProxies {
    All,
    Ranges(Vec<IpRangeEntry>),
}

/// One parsed range of [`TrustedProxies`].
#[derive(Debug, Clone, PartialEq)]
pub struct IpRangeEntry(IpRange);

/// The default `trusted_proxies`.
pub const DEFAULT_TRUSTED_PROXIES: &str = "127.0.0.0/8, ::1, 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 100.64.0.0/10, fc00::/7";

impl TrustedProxies {
    /// A comma-separated list; empty means the default. `Err` names the
    /// first entry that isn't an address or range.
    pub fn parse(list: &str) -> Result<TrustedProxies, String> {
        let list = if list.trim().is_empty() { DEFAULT_TRUSTED_PROXIES } else { list.trim() };
        if list == "*" {
            return Ok(TrustedProxies::All);
        }
        if list.eq_ignore_ascii_case("none") {
            return Ok(TrustedProxies::Ranges(Vec::new()));
        }
        list.split(',')
            .map(str::trim)
            .filter(|e| !e.is_empty())
            .map(|e| IpRange::parse(e).map(IpRangeEntry).ok_or_else(|| format!("trusted_proxies: {e:?} is not an address or CIDR range")))
            .collect::<Result<Vec<_>, _>>()
            .map(TrustedProxies::Ranges)
    }

    /// Whether a peer at `ip` may set the forwarding headers.
    pub fn trusts(&self, ip: &str) -> bool {
        match self {
            TrustedProxies::All => true,
            TrustedProxies::Ranges(ranges) => parse_ip(ip).is_some_and(|a| ranges.iter().any(|r| (a & r.0.mask) == r.0.network)),
        }
    }
}

/// `true` if `entry` is an address or CIDR range `IpFilter` understands.
pub fn is_valid_entry(entry: &str) -> bool {
    IpRange::parse(entry).is_some()
}

impl IpRange {
    #[doc(hidden)]
    pub fn parse(entry: &str) -> Option<Self> {
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
#[doc(hidden)]
pub fn parse_ip(s: &str) -> Option<u128> {
    let s = s.trim().trim_start_matches('[').trim_end_matches(']');
    let s = s.split('%').next()?;
    match s.parse::<std::net::IpAddr>().ok()? {
        std::net::IpAddr::V4(v4) => Some(u128::from(v4.to_ipv6_mapped())),
        std::net::IpAddr::V6(v6) => Some(u128::from(v6)),
    }
}
