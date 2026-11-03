//! `Proxy-Status` (RFC 9209, 18.17.0): on every proxied response, this
//! hop's name and either the status it received (`received-status=200`) or
//! why there was no response (`error=connection_refused`).
//!
//! `[server] proxy_status = "always"` (default), `"errors"` or `"off"`;
//! `proxy_name` (default `rws`) names this hop; `proxy_status_next_hop =
//! true` adds the backend (`next-hop`) and the failure's text (`details`),
//! which reveal internal addresses, so they're off by default.

use crate::header::Header;
use crate::response::Response;

/// RFC 9209 §2.3 error types rws reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyError {
    DnsError,
    DestinationUnavailable,
    DestinationIpUnroutable,
    ConnectionRefused,
    ConnectionTerminated,
    ConnectionTimeout,
    TlsProtocolError,
    TlsCertificateError,
    HttpResponseTimeout,
    HttpResponseIncomplete,
    HttpProtocolError,
    ProxyConfigurationError,
    ProxyInternalError,
}

impl ProxyError {
    /// The RFC 9209 name.
    pub fn as_str(self) -> &'static str {
        match self {
            ProxyError::DnsError => "dns_error",
            ProxyError::DestinationUnavailable => "destination_unavailable",
            ProxyError::DestinationIpUnroutable => "destination_ip_unroutable",
            ProxyError::ConnectionRefused => "connection_refused",
            ProxyError::ConnectionTerminated => "connection_terminated",
            ProxyError::ConnectionTimeout => "connection_timeout",
            ProxyError::TlsProtocolError => "tls_protocol_error",
            ProxyError::TlsCertificateError => "tls_certificate_error",
            ProxyError::HttpResponseTimeout => "http_response_timeout",
            ProxyError::HttpResponseIncomplete => "http_response_incomplete",
            ProxyError::HttpProtocolError => "http_protocol_error",
            ProxyError::ProxyConfigurationError => "proxy_configuration_error",
            ProxyError::ProxyInternalError => "proxy_internal_error",
        }
    }

    /// The error type of a forwarder's failure text.
    pub fn classify(failure: &str) -> ProxyError {
        let f = failure.to_ascii_lowercase();
        let has = |s: &str| f.contains(s);
        if has("dns lookup") || has("no address resolved") || has("failed to lookup address") {
            ProxyError::DnsError
        } else if has("invalid upstream") || has("invalid tls server name") || has("cannot read") || has("cannot parse") || has("need the http2") {
            ProxyError::ProxyConfigurationError
        } else if has("tls") || has("handshake") || has("certificate") {
            if has("certificate") || has("unknownissuer") || has("notvalidforname") || has("expired") {
                ProxyError::TlsCertificateError
            } else if has("timed out") {
                ProxyError::ConnectionTimeout
            } else {
                ProxyError::TlsProtocolError
            }
        } else if has("connect to") || has("connect(") {
            if has("timed out") {
                ProxyError::ConnectionTimeout
            } else if has("unreachable") {
                ProxyError::DestinationIpUnroutable
            } else {
                ProxyError::ConnectionRefused
            }
        } else if has("mid-body") || has("premature eof") {
            ProxyError::HttpResponseIncomplete
        } else if has("timed out") || has("would block") || has("temporarily unavailable") {
            ProxyError::HttpResponseTimeout
        } else if has("closed connection") || has("reset") || has("broken pipe") || has("write to backend") || has("aborted") {
            ProxyError::ConnectionTerminated
        } else if has("interim") || has("parse") || has("invalid") || has("malformed") || has("chunk") || has("status line") {
            ProxyError::HttpProtocolError
        } else {
            ProxyError::ProxyInternalError
        }
    }
}

/// When to send the header.
fn mode() -> (bool, bool) {
    let config = crate::server_config::ServerConfig::current();
    match config.proxy_status.as_str() {
        "off" => (false, false),
        "errors" => (false, true),
        _ => (true, true),
    }
}

/// A successful hop: `received-status`.
pub fn received(response: &mut Response, next_hop: Option<&str>) {
    if mode().0 {
        let status = response.status_code;
        add(response, &format!("received-status={status}"), next_hop, None);
    }
}

/// A hop that got no usable response: `error=…`. A `502` for a backend that
/// didn't answer in time becomes `504 Gateway Timeout`, as nginx answers
/// (since 18.27.0).
pub fn failed(response: &mut Response, error: ProxyError, next_hop: Option<&str>, details: Option<&str>) {
    if response.status_code == 502 && matches!(error, ProxyError::ConnectionTimeout | ProxyError::HttpResponseTimeout) {
        let mut timeout = crate::error::problem(crate::response::STATUS_CODE_REASON_PHRASE.n504_gateway_timeout, "");
        for header in response.headers.drain(..) {
            if !timeout.headers.iter().any(|h| h.name.eq_ignore_ascii_case(&header.name)) {
                timeout.headers.push(header);
            }
        }
        *response = timeout;
    }
    if mode().1 {
        add(response, &format!("error={}", error.as_str()), next_hop, details);
    }
}

/// A failure from its text (what the forwarders return).
pub fn failed_with(response: &mut Response, failure: &str, next_hop: Option<&str>) {
    failed(response, ProxyError::classify(failure), next_hop, Some(failure));
}

fn add(response: &mut Response, parameter: &str, next_hop: Option<&str>, details: Option<&str>) {
    let config = crate::server_config::ServerConfig::current();
    let mut member = format!("{}; {parameter}", crate::header::sf::item(&config.proxy_name));
    if config.proxy_status_next_hop {
        if let Some(hop) = next_hop.filter(|h| !h.is_empty()) {
            member.push_str(&format!("; next-hop={}", crate::header::sf::string(hop)));
        }
        if let Some(details) = details.filter(|d| !d.is_empty()) {
            member.push_str(&format!("; details={}", crate::header::sf::string(details)));
        }
    }
    // RFC 9209 §2: the last member is the hop closest to the client.
    match response.headers.iter_mut().find(|h| h.name.eq_ignore_ascii_case("proxy-status")) {
        Some(existing) => existing.value = format!("{}, {member}", existing.value).into(),
        None => response.headers.push(Header::new("Proxy-Status", member)),
    }
}

#[cfg(test)]
mod tests;
