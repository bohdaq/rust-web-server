//! Consul HTTP API discovery — queries a Consul agent's
//! `/v1/health/service/:name` endpoint, which already applies health-check
//! filtering server-side and returns each instance's node/service address and
//! port. Uses `crate::http_client::Client`, which (unlike TLS-capable async
//! clients elsewhere in this crate) needs no feature flag — `service_discovery`
//! itself is always compiled, so it can only depend on always-available pieces.

#[cfg(test)]
mod tests;

use super::json_lite::{self, JsonValue};
use crate::http_client::Client;

/// Queries `GET http://{addr}/v1/health/service/{service}?passing=true` and
/// converts the response into a `"host:port"` list. `Service.Address` is
/// preferred (the address the service itself registered); if that's empty —
/// common when a service registers without an explicit address — falls back
/// to `Node.Address` (the agent's own node address), matching Consul's own
/// documented resolution order for consumers of this endpoint.
pub(super) fn discover(addr: &str, service: &str) -> Vec<String> {
    let url = format!(
        "http://{}/v1/health/service/{}?passing=true",
        addr.trim_end_matches('/'),
        service
    );

    let response = match Client::new().get(&url).send() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("service_discovery: Consul query to {} failed: {}", url, e);
            return Vec::new();
        }
    };

    if !response.is_success() {
        eprintln!("service_discovery: Consul returned status {} for {}", response.status(), url);
        return Vec::new();
    }

    let body = match response.text() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("service_discovery: Consul response body for {} was not valid UTF-8: {}", url, e);
            return Vec::new();
        }
    };

    let parsed = match json_lite::parse(&body) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("service_discovery: failed to parse Consul response from {}: {}", url, e);
            return Vec::new();
        }
    };

    let Some(entries) = parsed.as_array() else {
        eprintln!("service_discovery: Consul response from {} was not a JSON array", url);
        return Vec::new();
    };

    entries.iter().filter_map(entry_to_backend).collect()
}

fn entry_to_backend(entry: &JsonValue) -> Option<String> {
    let service = entry.get("Service")?;
    let port = service.get("Port")?.as_f64()? as u16;

    let address = service
        .get("Address")
        .and_then(JsonValue::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            entry
                .get("Node")
                .and_then(|n| n.get("Address"))
                .and_then(JsonValue::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })?;

    Some(format!("{}:{}", address, port))
}
