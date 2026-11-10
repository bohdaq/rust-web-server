//! Cloud and server hosting networks, from a table compiled into the binary
//! (the `geo` feature): DB-IP's "IP to ASN Lite" (CC BY 4.0) reduced by
//! `ci/gen_hosting.py` to the ranges of hosting providers, in
//! [`geo::Table`](super::geo::Table)'s format with a provider code where
//! that has a country (~60 KB). A browser User-Agent from one of these is
//! a program — a link scanner, a preview, a headless crawler — not a
//! person, so the Traffic page counts it as a bot ([`Kind::Hosting`]).
//! Consumer VPN and CDN egress (Cloudflare WARP, iCloud Private Relay, VPN
//! services) is left out: people browse from it.
//!
//! [`Kind::Hosting`]: super::Kind::Hosting

use std::net::IpAddr;
use std::sync::OnceLock;

use super::geo::Table;

#[cfg(feature = "geo")]
static BUNDLED: &[u8] = include_bytes!("hosting.bin");

/// The provider codes `ci/gen_hosting.py` writes, and their names.
const PROVIDERS: [(&[u8; 2], &str); 20] = [
    (b"AW", "AWS"),
    (b"GC", "Google Cloud"),
    (b"AZ", "Azure"),
    (b"OR", "Oracle Cloud"),
    (b"AL", "Alibaba Cloud"),
    (b"TC", "Tencent Cloud"),
    (b"HW", "Huawei Cloud"),
    (b"DO", "DigitalOcean"),
    (b"LI", "Linode"),
    (b"VU", "Vultr"),
    (b"OV", "OVHcloud"),
    (b"HZ", "Hetzner"),
    (b"CB", "Contabo"),
    (b"SC", "Scaleway"),
    (b"LW", "Leaseweb"),
    (b"IO", "IONOS"),
    (b"HG", "Hostinger"),
    (b"IB", "IBM Cloud"),
    (b"RS", "Rackspace"),
    (b"DC", "other hosting"),
];

/// The table, decoded once; `None` without the `geo` feature.
pub fn table() -> Option<&'static Table> {
    static TABLE: OnceLock<Option<Table>> = OnceLock::new();
    TABLE.get_or_init(bundled).as_ref()
}

#[cfg(feature = "geo")]
fn bundled() -> Option<Table> {
    match Table::decode(BUNDLED) {
        Ok(table) => Some(table),
        Err(e) => {
            eprintln!("traffic: built-in hosting table: {e}");
            None
        }
    }
}

#[cfg(not(feature = "geo"))]
fn bundled() -> Option<Table> {
    None
}

/// The hosting provider `ip` belongs to (`"AWS"`, `"other hosting"`);
/// `None` for other networks, or without the table.
pub fn provider(ip: IpAddr) -> Option<&'static str> {
    provider_in(table()?, ip)
}

/// [`provider`] in a given table.
#[doc(hidden)]
pub fn provider_in(table: &Table, ip: IpAddr) -> Option<&'static str> {
    let code = table.country(ip)?;
    Some(PROVIDERS.iter().find(|(c, _)| **c == code).map_or("other hosting", |(_, name)| name))
}
