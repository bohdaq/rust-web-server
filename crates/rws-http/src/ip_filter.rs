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

// ── the client's address ──────────────────────────────────────────────────────

/// The address a request comes from, for everything that judges or records
/// a client (rate limits, IP filters, the blocklist, WAF rules, logs).
///
/// From a `trusted_proxies` peer (a load balancer, a CDN) that is the
/// rightmost `X-Forwarded-For` address the trusted proxies didn't add
/// themselves — walking right to left past trusted addresses, as nginx's
/// `real_ip_recursive` does, so a client can't pick its address by sending
/// its own `X-Forwarded-For`; without that header, the last `Forwarded:
/// for=`. From anyone else, or with `client_ip = "peer"`, it is the peer.
///
/// Proxying keeps using the peer: the `X-Forwarded-For` rws sends on is the
/// incoming chain plus the peer, so the next hop can do the same walk.
pub fn client_ip<'a>(request: &crate::request::Request, peer: &'a str) -> std::borrow::Cow<'a, str> {
    let config = crate::server_config::ServerConfig::current();
    client_ip_with(request, peer, &config.trusted_proxies, config.client_ip_forwarded)
}

/// [`client_ip`] with the trust list and mode given (no config read).
/// The scheme the client used: `https` on a TLS connection; behind a
/// trusted proxy that terminates TLS (`trusted_proxies`, unless
/// `client_ip = "peer"`), what it says in `X-Forwarded-Proto` (its first
/// value) or else `Forwarded: proto=`; `http` otherwise. For URLs rws hands
/// back to the client (an agent card, an OAuth redirect URI) and the
/// `Secure` cookie flag: behind a TLS-terminating load balancer the
/// connection itself is plain HTTP.
pub fn request_scheme(request: &crate::request::Request, peer: &str, tls: bool) -> &'static str {
    let config = crate::server_config::ServerConfig::current();
    request_scheme_with(request, peer, tls, &config.trusted_proxies, config.client_ip_forwarded)
}

/// [`request_scheme`] with the trust list given (no config read).
#[doc(hidden)]
pub fn request_scheme_with(request: &crate::request::Request, peer: &str, tls: bool, trusted: &TrustedProxies, forwarded: bool) -> &'static str {
    if tls {
        return "https";
    }
    if !forwarded || !trusted.trusts(peer) {
        return "http";
    }
    let first = |name: &str| request.headers.iter().find(|h| h.name.eq_ignore_ascii_case(name)).map(|h| h.value.split(',').next().unwrap_or("").trim().to_string());
    let proto = first("x-forwarded-proto").or_else(|| {
        let element = first("forwarded")?;
        element.split(';').find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            k.trim().eq_ignore_ascii_case("proto").then(|| v.trim().trim_matches('"').to_string())
        })
    });
    match proto {
        Some(p) if p.eq_ignore_ascii_case("https") => "https",
        _ => "http",
    }
}

#[doc(hidden)]
pub fn client_ip_with<'a>(request: &crate::request::Request, peer: &'a str, trusted: &TrustedProxies, forwarded: bool) -> std::borrow::Cow<'a, str> {
    use std::borrow::Cow;
    if !forwarded || !trusted.trusts(peer) {
        return Cow::Borrowed(peer);
    }
    // Every X-Forwarded-For line, in order, as one list.
    let xff: Vec<&str> = request
        .headers
        .iter()
        .filter(|h| h.name.eq_ignore_ascii_case("x-forwarded-for"))
        .flat_map(|h| h.value.split(','))
        .collect();
    let hops: Vec<&str> = if xff.is_empty() {
        request
            .headers
            .iter()
            .filter(|h| h.name.eq_ignore_ascii_case("forwarded"))
            .flat_map(|h| h.value.split(','))
            .filter_map(|element| {
                element.split(';').find_map(|pair| {
                    let (k, v) = pair.split_once('=')?;
                    k.trim().eq_ignore_ascii_case("for").then_some(v)
                })
            })
            .collect()
    } else {
        xff
    };
    let mut chosen: Option<String> = None;
    for hop in hops.iter().rev() {
        let Some(ip) = hop_address(hop) else { break };
        let trusted_hop = trusted.trusts(&ip);
        chosen = Some(ip);
        if !trusted_hop {
            break;
        }
    }
    chosen.map_or(Cow::Borrowed(peer), Cow::Owned)
}

/// One hop as an address in its canonical text: `203.0.113.7`,
/// `203.0.113.7:4711`, `2001:db8::1`, `[2001:db8::1]:443`, `"[…]"` (RFC 7239
/// quoting). `None` for `unknown`, obfuscated `_names` and garbage.
fn hop_address(hop: &str) -> Option<String> {
    let hop = hop.trim().trim_matches('"');
    let bare = if let Some(rest) = hop.strip_prefix('[') {
        rest.split(']').next()?
    } else if hop.matches(':').count() == 1 {
        hop.split(':').next()?
    } else {
        hop
    };
    bare.parse::<std::net::IpAddr>().ok().map(|ip| ip.to_string())
}

#[cfg(test)]
mod client_ip_tests {
    use super::*;
    use crate::header::Header;
    use crate::request::Request;

    fn with(headers: &[(&str, &str)]) -> Request {
        Request {
            method: "GET".into(),
            request_uri: "/".into(),
            http_version: "HTTP/1.1".into(),
            headers: headers.iter().map(|(n, v)| Header::new(n.to_string(), v.to_string())).collect(),
            body: Default::default(),
        }
    }
    fn private() -> TrustedProxies {
        TrustedProxies::parse("").unwrap()
    }

    #[test]
    fn a_trusted_peer_gives_the_client_in_x_forwarded_for() {
        let r = with(&[("X-Forwarded-For", "88.99.218.47, 172.30.0.3")]);
        assert_eq!(client_ip_with(&r, "172.30.0.3", &private(), true), "88.99.218.47");
    }

    #[test]
    fn a_client_cant_choose_its_address() {
        // The client sent 6.6.6.6 itself; the gateway appended what it saw.
        let r = with(&[("X-Forwarded-For", "6.6.6.6,88.99.218.47, 172.30.0.3")]);
        assert_eq!(client_ip_with(&r, "172.30.0.3", &private(), true), "88.99.218.47");
        // A private address in the client's own part is skipped like a proxy's,
        // but the walk stops at the first public one.
        let r = with(&[("X-Forwarded-For", "10.9.9.9, 88.99.218.47")]);
        assert_eq!(client_ip_with(&r, "10.0.0.1", &private(), true), "88.99.218.47");
    }

    #[test]
    fn an_untrusted_peer_or_peer_mode_keeps_the_peer() {
        let r = with(&[("X-Forwarded-For", "1.2.3.4")]);
        assert_eq!(client_ip_with(&r, "88.99.218.47", &private(), true), "88.99.218.47");
        assert_eq!(client_ip_with(&r, "10.0.0.1", &private(), false), "10.0.0.1");
        assert_eq!(client_ip_with(&r, "10.0.0.1", &TrustedProxies::parse("none").unwrap(), true), "10.0.0.1");
    }

    #[test]
    fn several_lines_ports_and_ipv6() {
        let r = with(&[("X-Forwarded-For", "203.0.113.7:4711"), ("X-Forwarded-For", "10.0.0.2")]);
        assert_eq!(client_ip_with(&r, "10.0.0.1", &private(), true), "203.0.113.7");
        let r = with(&[("X-Forwarded-For", "[2001:db8::1]:443, fd00::2")]);
        assert_eq!(client_ip_with(&r, "::1", &private(), true), "2001:db8::1");
    }

    #[test]
    fn forwarded_when_there_is_no_x_forwarded_for() {
        let r = with(&[("Forwarded", "for=\"[2001:db8::7]\";proto=https, for=10.0.0.9")]);
        assert_eq!(client_ip_with(&r, "10.0.0.1", &private(), true), "2001:db8::7");
    }

    #[test]
    fn garbage_or_only_trusted_hops() {
        // An unparseable hop ends the walk at the last address read.
        let r = with(&[("X-Forwarded-For", "203.0.113.7, unknown, 10.0.0.5")]);
        assert_eq!(client_ip_with(&r, "10.0.0.1", &private(), true), "10.0.0.5");
        // Nothing usable at all: the peer.
        let r = with(&[("X-Forwarded-For", "nonsense")]);
        assert_eq!(client_ip_with(&r, "10.0.0.1", &private(), true), "10.0.0.1");
        let r = with(&[]);
        assert_eq!(client_ip_with(&r, "10.0.0.1", &private(), true), "10.0.0.1");
        // Every hop trusted: the leftmost.
        let r = with(&[("X-Forwarded-For", "10.1.1.1, 10.2.2.2")]);
        assert_eq!(client_ip_with(&r, "10.0.0.1", &private(), true), "10.1.1.1");
    }
}
