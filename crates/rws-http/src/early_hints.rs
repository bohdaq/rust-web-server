//! `103 Early Hints` links: what a usable `Link` value is. The middleware
//! that sends them is rust-web-server's `early_hints`.

/// A usable `Link` value: not empty, no CR, LF or NUL.
pub fn valid(link: &str) -> bool {
    !link.trim().is_empty() && !link.bytes().any(|b| matches!(b, b'\r' | b'\n' | 0))
}
