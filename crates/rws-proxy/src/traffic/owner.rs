//! The site's owner (18.35): their visits are counted apart, so the
//! Traffic page shows everyone else. A client is the owner when
//!
//! - it made an admin API call that succeeded (a request with the admin
//!   UI's `X-Rws-Admin` header answered 2xx — through a route that proxies
//!   the admin API, or the admin listener itself): from then on that day,
//!   its address and User-Agent are the owner's on every site;
//! - its address is in `[traffic] owner_networks`;
//! - its browser holds the owner cookie, set by opening
//!   `https://<site>/_rws/me?k=<key>` (the links are on the Traffic page,
//!   `GET /traffic/owner`): `rws_me=<key>`, a year, `HttpOnly`. The key is
//!   derived from the admin token, so it survives restarts and changes when
//!   the token does; with OIDC sign-in only, it's made at start (the cookie
//!   then works until a restart).
//!
//! Recognition starts when it happens: what a visitor did earlier that day
//! stays where it was counted.

use std::net::IpAddr;
use std::sync::{OnceLock, RwLock};

/// The owner cookie's name.
pub const COOKIE: &str = "rws_me";
/// The path that sets (and with `?off`, clears) the owner cookie.
pub const PATH: &str = "/_rws/me";

fn networks() -> &'static RwLock<Vec<String>> {
    static NETWORKS: OnceLock<RwLock<Vec<String>>> = OnceLock::new();
    NETWORKS.get_or_init(|| RwLock::new(super::config::owner_networks()))
}

/// Replace `[traffic] owner_networks` (reload, tests).
pub fn set_networks(entries: &[String]) {
    *networks().write().unwrap_or_else(|e| e.into_inner()) = entries.to_vec();
}

/// Whether `ip` is in `[traffic] owner_networks`.
pub fn in_networks(ip: IpAddr) -> bool {
    let networks = networks().read().unwrap_or_else(|e| e.into_inner());
    if networks.is_empty() {
        return false;
    }
    let ip = ip.to_canonical().to_string();
    networks.iter().any(|entry| crate::ip_filter::matches(entry, &ip))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The owner cookie's value: HMAC-SHA256 of the admin token (32 hex
/// characters), or a value made at start without one.
pub fn key() -> &'static str {
    static KEY: OnceLock<String> = OnceLock::new();
    KEY.get_or_init(|| {
        #[cfg(not(target_arch = "wasm32"))]
        if let Ok(token) = crate::admin::AdminConfig::from_env().token() {
            return hex(&crate::core::sha256::hmac_sha256(token.as_bytes(), b"rws-traffic-owner")[..16]);
        }
        use std::hash::{BuildHasher, Hasher};
        let mut out = String::new();
        for i in 0..2u8 {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u8(i);
            out.push_str(&format!("{:016x}", h.finish()));
        }
        out
    })
}

/// Whether `value` is the owner key.
pub fn key_matches(value: &str) -> bool {
    crate::core::constant_time_eq(value.trim().as_bytes(), key().as_bytes())
}

/// Whether the request's `Cookie` headers carry the owner cookie.
pub fn has_cookie(headers: &[crate::header::Header]) -> bool {
    headers.iter().filter(|h| h.name.eq_ignore_ascii_case("cookie")).any(|h| {
        h.value.split(';').any(|pair| match pair.trim().split_once('=') {
            Some((name, value)) => name.trim() == COOKIE && key_matches(value),
            None => false,
        })
    })
}

/// Whether a finished request was a successful admin API call (the admin
/// UI sends `X-Rws-Admin: 1` with each).
pub fn is_admin_call(headers: &[crate::header::Header], status: i16) -> bool {
    (200..300).contains(&status) && headers.iter().any(|h| h.name.eq_ignore_ascii_case("x-rws-admin"))
}
