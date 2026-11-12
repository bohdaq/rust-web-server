//! Listener filters (Gateway API v1.7, experimental, GEP-5224): a listener's
//! `filters.requests` run in order on every HTTP request, before a route is
//! picked. rws supports `ExternalAuth`:
//!
//! - **HTTP**: the request's method and path (after `http.path`, the
//!   prefix) go to the authorization server with `Host`, `Authorization`,
//!   `Content-Length` and the `allowedHeaders`, and the body up to
//!   `forwardBody.maxSize`. `200` allows the request, which gets the
//!   server's response headers (`allowedResponseHeaders`, else all but
//!   `Host`/`:authority` and framing); any other status is the answer.
//! - **GRPC**: Envoy's `ext_authz` (`envoy.service.auth.v3.Authorization/Check`)
//!   over HTTP/2 — the request's attributes (method, path, host, scheme,
//!   headers, body) in a `CheckRequest`; status `OK` allows it (with the
//!   `ok_response`'s header changes), anything else answers with the
//!   `denied_response` (`403` by default).
//!
//! A request an authorization server denies goes no further; changes to
//! `Host`/`:authority` are ignored (the listener was already chosen). An
//! authorization server that can't be reached denies with `403`: the
//! filter fails closed.

pub mod proto;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crate::header::Header;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

use super::backend_tls::BackendTls;
use super::types::HttpExternalAuthFilter;

/// How long an authorization request may take.
const AUTH_TIMEOUT: Duration = Duration::from_secs(10);

/// One `ExternalAuth` filter, its server resolved.
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledAuth {
    pub filter: HttpExternalAuthFilter,
    /// The server's ready `ip:port`s; `Err` when the reference doesn't resolve.
    pub endpoints: Result<Vec<String>, String>,
    /// TLS to the server, when a BackendTLSPolicy covers it.
    pub tls: Option<Result<BackendTls, String>>,
}

/// What an authorization server decided.
#[derive(Debug)]
pub enum Decision {
    /// Go on, with these headers set and these removed.
    Allow { set: Vec<(String, String)>, remove: Vec<String> },
    /// This is the answer.
    Deny(Response),
}

/// Headers a filter never changes: the listener was chosen by them.
fn protected(name: &str) -> bool {
    matches!(name.to_ascii_lowercase().as_str(), "host" | ":authority")
}

/// Run `filters` in order on `request`: the request as they left it
/// (`None`: unchanged), or the answer when one denies it.
pub fn run(filters: &[CompiledAuth], request: &Request, client: crate::proxy::Peer<'_>, counter: &AtomicUsize) -> Result<Option<Request>, Response> {
    if filters.is_empty() {
        return Ok(None);
    }
    let mut current = request.clone();
    for filter in filters {
        match check(filter, &current, client, counter) {
            Decision::Deny(response) => return Err(response),
            Decision::Allow { set, remove } => {
                for name in remove.iter().filter(|n| !protected(n)) {
                    current.headers.retain(|h| !h.name.eq_ignore_ascii_case(name));
                }
                for (name, value) in set.into_iter().filter(|(n, _)| !protected(n)) {
                    current.headers.retain(|h| !h.name.eq_ignore_ascii_case(&name));
                    current.headers.push(Header::new(name, value));
                }
            }
        }
    }
    Ok(Some(current))
}

fn forbidden(message: &str) -> Response {
    Response::text(STATUS_CODE_REASON_PHRASE.n403_forbidden, message)
}

/// Ask one authorization server.
pub fn check(auth: &CompiledAuth, request: &Request, client: crate::proxy::Peer<'_>, counter: &AtomicUsize) -> Decision {
    let endpoint = match &auth.endpoints {
        Ok(endpoints) if !endpoints.is_empty() => endpoints[counter.fetch_add(1, Ordering::Relaxed) % endpoints.len()].clone(),
        _ => return Decision::Deny(forbidden("the authorization server isn't available")),
    };
    let Some((ip, port)) = endpoint.rsplit_once(':').and_then(|(ip, port)| Some((ip.trim_start_matches('[').trim_end_matches(']').to_string(), port.parse::<u16>().ok()?))) else {
        return Decision::Deny(forbidden("the authorization server isn't available"));
    };
    let tls = match &auth.tls {
        None => None,
        Some(Ok(tls)) => Some(tls),
        Some(Err(_)) => return Decision::Deny(forbidden("the authorization server's TLS isn't usable")),
    };
    if auth.filter.protocol.eq_ignore_ascii_case("GRPC") {
        check_grpc(&auth.filter, request, client, &ip, port, tls)
    } else {
        check_http(&auth.filter, request, client, &ip, port, tls)
    }
}

/// The body to forward: up to `forwardBody.maxSize` bytes.
fn forwarded_body<'a>(filter: &HttpExternalAuthFilter, request: &'a Request) -> &'a [u8] {
    let max = filter.forward_body.as_ref().map_or(0, |f| f.max_size.max(0) as usize);
    &request.body[..request.body.len().min(max)]
}

/// The HTTP authorization request for `request`.
pub fn http_request(filter: &HttpExternalAuthFilter, request: &Request) -> Request {
    let config = filter.http.clone().unwrap_or_default();
    let prefix = config.path.trim_end_matches('/');
    let mut headers = Vec::new();
    for h in &request.headers {
        let always = matches!(h.name.to_ascii_lowercase().as_str(), "host" | "authorization");
        if always || config.allowed_headers.iter().any(|a| a.eq_ignore_ascii_case(&h.name)) {
            headers.push(h.clone());
        }
    }
    let body = forwarded_body(filter, request).to_vec();
    headers.retain(|h| !h.name.eq_ignore_ascii_case("content-length"));
    headers.push(Header::new("Content-Length", body.len().to_string()));
    Request { method: request.method.clone(), request_uri: format!("{prefix}{}", request.request_uri), http_version: "HTTP/1.1".to_string(), headers, body: body.into() }
}

/// Response headers never copied into the request.
fn not_copied(name: &str) -> bool {
    matches!(name.to_ascii_lowercase().as_str(), "host" | ":authority" | "content-length" | "transfer-encoding" | "connection" | "keep-alive" | "proxy-connection" | "te" | "upgrade" | "trailer")
}

/// An HTTP authorization server's answer as a decision.
pub fn http_decision(filter: &HttpExternalAuthFilter, response: Response) -> Decision {
    if response.status_code != 200 {
        return Decision::Deny(response);
    }
    let allowed = filter.http.as_ref().map(|h| h.allowed_response_headers.clone()).unwrap_or_default();
    let set = response
        .headers
        .iter()
        .filter(|h| !not_copied(&h.name))
        .filter(|h| allowed.is_empty() || allowed.iter().any(|a| a.eq_ignore_ascii_case(&h.name)))
        .map(|h| (h.name.to_string(), h.value.to_string()))
        .collect();
    Decision::Allow { set, remove: Vec::new() }
}

fn check_http(filter: &HttpExternalAuthFilter, request: &Request, client: crate::proxy::Peer<'_>, ip: &str, port: u16, tls: Option<&BackendTls>) -> Decision {
    let auth_request = http_request(filter, request);
    let host = request.header("host").map(|h| h.value.to_string()).unwrap_or_else(|| format!("{ip}:{port}"));
    let answered = match tls {
        Some(tls) => crate::proxy::forward_tls_with_host(&tls.config, &tls.label, &auth_request, client, ip, port, &tls.server_name, &host, AUTH_TIMEOUT, AUTH_TIMEOUT),
        None => crate::proxy::forward_pooled_with_host(crate::proxy::shared_pool(), &auth_request, client, ip, port, &host, AUTH_TIMEOUT, AUTH_TIMEOUT),
    };
    match answered {
        Ok(response) => http_decision(filter, buffered(response)),
        Err(_) => Decision::Deny(forbidden("the authorization server didn't answer")),
    }
}

/// A response with a streamed body read into memory (an authorization
/// answer is small).
fn buffered(mut response: Response) -> Response {
    if let Some(mut pipe) = response.stream_pipe.take() {
        let mut body = Vec::new();
        let _ = std::io::Read::read_to_end(&mut std::io::Read::take(&mut pipe, 1 << 20), &mut body);
        response.content_range_list = vec![crate::range::Range::get_content_range(body, "application/octet-stream".to_string())];
    }
    response
}

fn body_of(response: &Response) -> Vec<u8> {
    response.content_range_list.iter().flat_map(|c| c.body.iter().copied()).collect()
}

fn check_grpc(filter: &HttpExternalAuthFilter, request: &Request, client: crate::proxy::Peer<'_>, ip: &str, port: u16, tls: Option<&BackendTls>) -> Decision {
    let allowed = filter.grpc.as_ref().map(|g| g.allowed_headers.clone()).unwrap_or_default();
    let headers: Vec<(String, String)> = request
        .headers
        .iter()
        .filter(|h| allowed.is_empty() || allowed.iter().any(|a| a.eq_ignore_ascii_case(&h.name)))
        .map(|h| (h.name.to_ascii_lowercase(), h.value.to_string()))
        .collect();
    let host = request.header("host").map(|h| h.value.to_string()).unwrap_or_default();
    let scheme = if client.tls { "https" } else { "http" };
    let message = proto::check_request(&proto::HttpAttributes {
        method: &request.method,
        path: &request.request_uri,
        host: &host,
        scheme,
        headers: &headers,
        body: forwarded_body(filter, request),
        source_ip: client.ip,
    });
    let mut frame = vec![0u8];
    frame.extend_from_slice(&(message.len() as u32).to_be_bytes());
    frame.extend_from_slice(&message);
    let call = Request {
        method: "POST".to_string(),
        request_uri: "/envoy.service.auth.v3.Authorization/Check".to_string(),
        http_version: "HTTP/2".to_string(),
        headers: vec![Header::new("Content-Type", "application/grpc"), Header::new("TE", "trailers")],
        body: frame.into(),
    };
    let authority = format!("{ip}:{port}");
    let answered = match tls {
        Some(tls) => crate::proxy::forward_h2_tls_with_host(&tls.h2_config, &call, client, ip, port, &tls.server_name, &authority, AUTH_TIMEOUT, AUTH_TIMEOUT),
        None => crate::proxy::forward_h2c_with_host(&call, client, ip, port, &authority, AUTH_TIMEOUT, AUTH_TIMEOUT),
    };
    let Ok(response) = answered else { return Decision::Deny(forbidden("the authorization server didn't answer")) };
    let response = buffered(response);
    let grpc_status = response
        .trailers
        .as_ref()
        .map(|t| t.get())
        .into_iter()
        .flatten()
        .chain(response.headers.iter().cloned())
        .find(|h| h.name.eq_ignore_ascii_case("grpc-status"))
        .map(|h| h.value.to_string());
    if grpc_status.as_deref() != Some("0") {
        return Decision::Deny(forbidden("the authorization check failed"));
    }
    let body = body_of(&response);
    let Some(message) = body.get(5..).filter(|_| body.first() == Some(&0)) else {
        return Decision::Deny(forbidden("the authorization server's answer isn't a CheckResponse"));
    };
    match proto::check_response(message) {
        Ok(verdict) => grpc_decision(verdict),
        Err(_) => Decision::Deny(forbidden("the authorization server's answer isn't a CheckResponse")),
    }
}

/// A `CheckResponse` as a decision.
pub fn grpc_decision(verdict: proto::Verdict) -> Decision {
    if verdict.code == 0 {
        return Decision::Allow { set: verdict.headers, remove: verdict.headers_to_remove };
    }
    let status = u16::try_from(verdict.denied_status).ok().filter(|s| (400..600).contains(s)).unwrap_or(403);
    let reason = Response::status_code_reason_phrase_list().into_iter().find(|p| *p.status_code == status as i16).unwrap_or(STATUS_CODE_REASON_PHRASE.n403_forbidden);
    let mut response = Response::text(reason, &verdict.denied_body);
    for (name, value) in verdict.headers.into_iter().filter(|(n, _)| !protected(n)) {
        response.headers.retain(|h| !h.name.eq_ignore_ascii_case(&name));
        response.headers.push(Header::new(name, value));
    }
    Decision::Deny(response)
}

#[cfg(test)]
mod tests;
