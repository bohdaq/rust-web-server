#[cfg(test)]
mod tests;
#[cfg(test)]
mod example;
pub mod path;

use std::collections::HashMap;
use url_build_parse::{build_url, UrlAuthority, UrlComponents, UrlUserInfo};
use url_search_params::{build_url_search_params, encode_uri_component, parse_url_search_params};
use url_search_params::decode_uri_component;

/// URL parsing, building, and percent-encoding utilities.
pub struct URL;

impl URL {
    pub fn percent_encode(component: &str) -> String {
        encode_uri_component(component)
    }

    pub fn percent_decode(component: &str) -> String {
        decode_uri_component(component)
    }

    pub fn build_query(params: HashMap<String, String>) -> String {
        build_url_search_params(params)
    }

    pub fn parse_query(component: &str) -> HashMap<String, String> {
        parse_url_search_params(component)
    }

    pub fn build(components: UrlComponents) -> Result<String, String> {
        build_url(components)
    }

    pub fn parse(url: &str) -> Result<UrlComponents, String> {
        parse_url(url)
    }

    /// Parse an origin-form request target (`/path?query`) — what
    /// `Request::request_uri` holds. Anything not starting with `/` is an
    /// error rather than being glued onto a placeholder authority, where a
    /// target like `:x` would become the port of `http://localhost:x`.
    pub fn parse_request_target(target: &str) -> Result<UrlComponents, String> {
        if !target.starts_with('/') {
            return Err(format!("not an origin-form request target: {:?}", target));
        }
        parse_url(&format!("http://localhost{}", target))
    }

    /// The path of an origin-form request target — everything before the
    /// first `?` or `#` — borrowed, without parsing or allocating. Same value
    /// as [`URL::parse_request_target`]`(target)?.path`; `None` for a target
    /// that isn't origin-form. For per-request matching on hot paths.
    pub(crate) fn request_target_path(target: &str) -> Option<&str> {
        if !target.starts_with('/') {
            return None;
        }
        Some(&target[..target.find(['?', '#']).unwrap_or(target.len())])
    }
}

/// RFC 3986 parse of `scheme:[//authority]path[?query][#fragment]` into
/// url-build-parse's `UrlComponents`, without panicking on malformed input.
/// (url-build-parse's own `parse_url` unwraps on input-derived values — e.g.
/// a non-numeric port — so a hostile URL could panic the caller.)
fn parse_url(url: &str) -> Result<UrlComponents, String> {
    let (scheme, rest) = url.split_once(':').ok_or("unable to identify scheme")?;
    if rest.is_empty() {
        return Err("error: remaining url is empty".to_string());
    }
    let mut components = UrlComponents::new();
    components.scheme = scheme.to_string();

    // Authority: after "//", up to the first '/', '?' or '#'.
    let mut remaining = rest;
    if let Some(after) = rest.strip_prefix("//") {
        let end = after.find(['/', '?', '#']).unwrap_or(after.len());
        components.authority = Some(parse_authority(&after[..end])?);
        remaining = &after[end..];
    }

    // Path: up to the first '?' or '#'.
    let path_end = remaining.find(['?', '#']).unwrap_or(remaining.len());
    components.path = remaining[..path_end].to_string();
    remaining = &remaining[path_end..];

    if let Some(after) = remaining.strip_prefix('?') {
        let query_end = after.find('#').unwrap_or(after.len());
        components.query = Some(parse_url_search_params(&after[..query_end]));
        remaining = &after[query_end..];
    }
    if let Some(fragment) = remaining.strip_prefix('#') {
        components.fragment = Some(fragment.to_string());
    }
    Ok(components)
}

/// `[userinfo@]host[:port]`, with `[...]` IPv6 hosts.
fn parse_authority(authority: &str) -> Result<UrlAuthority, String> {
    let (user_info, host_port) = match authority.split_once('@') {
        Some((user_info, host_port)) => {
            let user_info = match user_info.split_once(':') {
                Some((username, password)) => UrlUserInfo {
                    username: username.to_string(),
                    password: Some(password.to_string()),
                },
                None => UrlUserInfo { username: user_info.to_string(), password: None },
            };
            (Some(user_info), host_port)
        }
        None => (None, authority),
    };

    let (host, port_str) = if host_port.starts_with('[') {
        let close = host_port.find(']').ok_or("unterminated IPv6 host")?;
        let after = &host_port[close + 1..];
        let port = match after.strip_prefix(':') {
            Some(port) => Some(port),
            None if after.is_empty() => None,
            None => return Err(format!("unexpected characters after IPv6 host: {:?}", after)),
        };
        (&host_port[..=close], port)
    } else {
        match host_port.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (host_port, None),
        }
    };

    let port = match port_str {
        Some(port) => Some(port.parse::<usize>().map_err(|_| format!("invalid port: {:?}", port))?),
        None => None,
    };
    Ok(UrlAuthority { user_info, host: host.to_string(), port })
}

