//! The optional first-party script (`[traffic] script = true`, 18.31.0):
//! rws serves `/_rws/t.js` itself, and the script reports to `/_rws/t` on
//! the same origin what the server can't see — time on page, scroll depth,
//! screen size, single-page-app route changes and custom events
//! (`rws.track("signup", {plan: "pro"})`). Nothing is stored in the
//! browser; the visitor is the same daily hash as for every request.
//!
//! [`TrafficScriptLayer`] answers only those two paths and passes every
//! other request straight through ([`Middleware::is_passthrough`]), so it
//! costs nothing elsewhere and doesn't take proxy routes off the async
//! forwarder.

use crate::application::Application;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

/// The script's URL path.
pub const SCRIPT_PATH: &str = "/_rws/t.js";
/// Where the script reports.
pub const BEACON_PATH: &str = "/_rws/t";
/// The largest report accepted.
pub const MAX_BEACON: usize = 2048;
/// Where the site's apps send events (18.35, [`super::record::app_event`]).
pub const APP_EVENTS_PATH: &str = "/_rws/e";

/// The script. No dependencies, no storage, ~1.6 KB (0.8 KB compressed). The first page view is
/// already counted by the server, so it reports only what happens after:
/// leaving a page (time, scroll, width), route changes, events.
pub const SCRIPT: &str = r#"(()=>{"use strict";const w=window,d=document,n=navigator,E="/_rws/t";if(n.webdriver&&!w.rwsTrackAutomation)return;
let p=location.pathname+location.search,s=Date.now(),m=0,on=d.visibilityState!=="hidden",shown=on?s:0,acc=0;
const send=b=>{const t=JSON.stringify(b);try{if(n.sendBeacon&&n.sendBeacon(E,t))return}catch(e){}try{fetch(E,{method:"POST",body:t,keepalive:true,credentials:"omit"})}catch(e){}};
const depth=()=>{const h=d.documentElement,y=(w.scrollY||0)+w.innerHeight;m=Math.max(m,Math.min(1,y/Math.max(h.scrollHeight,1)))};
const seen=()=>acc+(on?Date.now()-shown:0);
const leave=()=>{if(!p)return;depth();send({t:"leave",p,ms:seen(),sd:Math.round(m*100),w:w.innerWidth||0});p=null};
const start=np=>{p=np;s=Date.now();m=0;acc=0;shown=s};
addEventListener("scroll",depth,{passive:true});
d.addEventListener("visibilitychange",()=>{if(d.visibilityState==="hidden"){if(on){acc+=Date.now()-shown;on=false}leave()}else{if(!p)start(location.pathname+location.search);on=true;shown=Date.now()}});
addEventListener("pagehide",leave);
const nav=()=>{const np=location.pathname+location.search;if(np===p)return;const from=p;leave();start(np);send({t:"view",p:np,r:from})};
for(const k of["pushState","replaceState"]){const o=history[k];history[k]=function(){const r=o.apply(this,arguments);nav();return r}}
addEventListener("popstate",nav);
w.rws=w.rws||{};w.rws.track=(name,props)=>{const q={};if(props&&typeof props==="object")for(const k of Object.keys(props).slice(0,4))q[String(k).slice(0,32)]=String(props[k]).slice(0,64);send({t:"event",p:location.pathname+location.search,n:String(name).slice(0,64),q})};
depth()})();
"#;

/// Answers `/_rws/t.js` and `/_rws/t` when `[traffic] script` is on (it
/// reads the setting when built); otherwise, and for every other path,
/// passes the request on.
#[derive(Clone, Copy, Debug, Default)]
pub struct TrafficScriptLayer {
    on: bool,
}

impl TrafficScriptLayer {
    /// The layer, on when `[traffic] script = true`.
    pub fn from_env() -> Self {
        Self { on: super::config::script() }
    }

    /// The layer, on or off regardless of the setting.
    pub fn new(on: bool) -> Self {
        Self { on }
    }

    fn mine(&self, request: &Request) -> bool {
        // Every other path: one byte compare.
        if !request.request_uri.starts_with("/_rws/") {
            return false;
        }
        let path = request.request_uri.split(['?', '#']).next().unwrap_or("");
        if self.on && (path == SCRIPT_PATH || path == BEACON_PATH) {
            return true;
        }
        // App events and the owner link work whenever traffic is counted.
        (path == APP_EVENTS_PATH || path == super::owner::PATH) && super::record::enabled()
    }
}

/// The client as the access log sees it (behind a trusted proxy, the
/// forwarded address), so the visitor hash matches its other requests.
fn client(request: &Request, connection: &ConnectionInfo) -> Option<std::net::IpAddr> {
    let peer = &connection.client.ip;
    let forwarded = request.header("x-forwarded-for").is_some() || request.header("forwarded").is_some();
    let ip = if forwarded { crate::ip_filter::client_ip(request, peer).into_owned() } else { peer.clone() };
    ip.trim_matches(['[', ']']).parse::<std::net::IpAddr>().ok()
}

/// `POST /_rws/e`: an app's event. Any origin (an extension's is its own,
/// a native app sends none); `OPTIONS` answers a preflight.
fn app_event(request: &Request, connection: &ConnectionInfo) -> Response {
    let cors = |mut r: Response| {
        r.headers.push(crate::header::Header::new("Access-Control-Allow-Origin", "*"));
        r
    };
    if request.method == "OPTIONS" {
        let mut r = cors(plain(STATUS_CODE_REASON_PHRASE.n204_no_content));
        r.headers.push(crate::header::Header::new("Access-Control-Allow-Methods", "POST"));
        r.headers.push(crate::header::Header::new("Access-Control-Allow-Headers", "content-type"));
        r.headers.push(crate::header::Header::new("Access-Control-Max-Age", "86400"));
        return r;
    }
    if request.method != "POST" {
        return plain(STATUS_CODE_REASON_PHRASE.n405_method_not_allowed);
    }
    if request.body.len() > MAX_BEACON {
        return cors(plain(STATUS_CODE_REASON_PHRASE.n413_payload_too_large));
    }
    let header = |name: &str| request.header(name).map(|h| h.value.as_ref()).filter(|v: &&str| !v.is_empty());
    let Some(ip) = client(request, connection) else { return cors(plain(STATUS_CODE_REASON_PHRASE.n400_bad_request)) };
    let body = String::from_utf8_lossy(&request.body);
    match super::record::app_event(ip, header("host").unwrap_or(""), header("user-agent"), &body, super::record::now()) {
        Ok(()) => cors(plain(STATUS_CODE_REASON_PHRASE.n204_no_content)),
        Err(super::record::AppEventError::Malformed) => cors(plain(STATUS_CODE_REASON_PHRASE.n400_bad_request)),
        Err(super::record::AppEventError::UnknownApp) => cors(plain(STATUS_CODE_REASON_PHRASE.n403_forbidden)),
    }
}

/// `GET /_rws/me?k=<key>`: this browser is the owner's (a cookie, for a
/// year); `?off`: not any more.
fn owner_link(request: &Request, connection: &ConnectionInfo) -> Response {
    if request.method != "GET" && request.method != "HEAD" {
        return plain(STATUS_CODE_REASON_PHRASE.n405_method_not_allowed);
    }
    let query = request.request_uri.split_once('?').map_or("", |(_, q)| q.split('#').next().unwrap_or(q));
    let off = query.split('&').any(|p| p == "off" || p.starts_with("off="));
    let key = query.split('&').find_map(|p| p.strip_prefix("k=")).unwrap_or("");
    let host = request.header("host").map(|h| h.value.as_ref()).unwrap_or("");
    // From the request: escaped, it lands in HTML.
    let host: String = host.split(':').next().unwrap_or(host).chars().take(253).map(|c| match c {
        '&' => "&amp;".to_string(),
        '<' => "&lt;".to_string(),
        '>' => "&gt;".to_string(),
        '"' => "&quot;".to_string(),
        '\'' => "&#39;".to_string(),
        c => c.to_string(),
    }).collect();
    let secure = connection.sni_hostname.is_some() || request.header("x-forwarded-proto").is_some_and(|h| h.value.eq_ignore_ascii_case("https"));
    let attrs = format!("Path=/; HttpOnly; SameSite=Lax{}", if secure { "; Secure" } else { "" });
    let (status, cookie, text) = if off {
        (STATUS_CODE_REASON_PHRASE.n200_ok, Some(format!("{}=; Max-Age=0; {attrs}", super::owner::COOKIE)), format!("This browser's visits to {host} are counted like anyone's again."))
    } else if super::owner::key_matches(key) {
        if let Some(ip) = client(request, connection) {
            super::record::mark_owner(ip, request.header("user-agent").map(|h| h.value.as_ref()), super::record::now());
        }
        (
            STATUS_CODE_REASON_PHRASE.n200_ok,
            Some(format!("{}={}; Max-Age=31536000; {attrs}", super::owner::COOKIE, super::owner::key())),
            format!("This browser's visits to {host} are now counted as yours, apart from everyone else's. To undo, open /_rws/me?off."),
        )
    } else {
        (STATUS_CODE_REASON_PHRASE.n403_forbidden, None, "That link isn't this site's owner link.".to_string())
    };
    let html = format!("<!doctype html><meta charset=utf-8><meta name=viewport content=\"width=device-width\"><meta name=robots content=noindex><title>rws traffic</title><p style=\"font:16px system-ui;margin:2rem\">{text}</p>");
    let mut response = Response::text(status, &html);
    if let Some(range) = response.content_range_list.first_mut() {
        range.content_type = "text/html; charset=utf-8".to_string();
    }
    response.headers.retain(|h| !h.name.eq_ignore_ascii_case("cache-control"));
    response.headers.push(crate::header::Header::new("Cache-Control", "no-store"));
    response.headers.push(crate::header::Header::new("Referrer-Policy", "no-referrer"));
    response.headers.push(crate::header::Header::new("X-Robots-Tag", "noindex"));
    if let Some(cookie) = cookie {
        response.headers.push(crate::header::Header::new("Set-Cookie", cookie));
    }
    response
}

fn plain(status: &crate::response::StatusCodeReasonPhrase) -> Response {
    let mut response = Response::with_status(status);
    response.headers.push(crate::header::Header::new("Cache-Control", "no-store"));
    response
}

impl Middleware for TrafficScriptLayer {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        if !self.mine(request) {
            return next.execute(request, connection);
        }
        let path = request.request_uri.split(['?', '#']).next().unwrap_or("");
        if path == APP_EVENTS_PATH {
            return Ok(app_event(request, connection));
        }
        if path == super::owner::PATH {
            return Ok(owner_link(request, connection));
        }
        if path == SCRIPT_PATH {
            if request.method != "GET" && request.method != "HEAD" {
                return Ok(plain(STATUS_CODE_REASON_PHRASE.n405_method_not_allowed));
            }
            return Ok(script_response(request));
        }
        if request.method != "POST" {
            return Ok(plain(STATUS_CODE_REASON_PHRASE.n405_method_not_allowed));
        }
        if request.body.len() > MAX_BEACON {
            return Ok(plain(STATUS_CODE_REASON_PHRASE.n413_payload_too_large));
        }
        let header = |name: &str| request.header(name).map(|h| h.value.as_ref()).filter(|v: &&str| !v.is_empty());
        let host = header("host").unwrap_or("");
        // Same origin only: a page elsewhere can't post reports for this site.
        if let Some(origin) = header("origin") {
            let origin_host = origin.split_once("://").map_or(origin, |(_, rest)| rest);
            if !origin_host.eq_ignore_ascii_case(host) {
                return Ok(plain(STATUS_CODE_REASON_PHRASE.n403_forbidden));
            }
        }
        if let Some(ip) = client(request, connection) {
            let body = String::from_utf8_lossy(&request.body);
            super::record::beacon(ip, host, header("user-agent"), &body, super::record::now());
        }
        Ok(plain(STATUS_CODE_REASON_PHRASE.n204_no_content))
    }

    fn runs_inline(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }

    fn is_passthrough(&self, request: &Request, _connection: &ConnectionInfo) -> bool {
        !self.mine(request)
    }

    fn accepts_streamed_body(&self, request: &Request, _connection: &ConnectionInfo) -> bool {
        !self.mine(request)
    }

    fn layer_name(&self) -> &'static str {
        "TrafficScriptLayer"
    }
}

/// A weak, content-derived ETag for the script.
fn etag() -> &'static str {
    static ETAG: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ETAG.get_or_init(|| {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        SCRIPT.hash(&mut h);
        format!("\"rws-t-{:016x}\"", h.finish())
    })
}

fn script_response(request: &Request) -> Response {
    let not_modified = request.header("if-none-match").is_some_and(|v| v.value.split(',').any(|t| t.trim() == etag()));
    let mut response = if not_modified {
        plain(STATUS_CODE_REASON_PHRASE.n304_not_modified)
    } else {
        Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, SCRIPT)
    };
    if !not_modified {
        if let Some(range) = response.content_range_list.first_mut() {
            range.content_type = "text/javascript; charset=utf-8".to_string();
        }
    }
    response.headers.retain(|h| !h.name.eq_ignore_ascii_case("cache-control"));
    response.headers.push(crate::header::Header::new("Cache-Control", "public, max-age=3600"));
    response.headers.push(crate::header::Header::new("ETag", etag()));
    response
}
