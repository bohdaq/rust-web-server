//! The HTML page rws shows a browser for its own errors (18.37.0): 404,
//! 502, 503, maintenance and the rest. One self-contained template (inline
//! CSS, light and dark, a title, the status, one sentence, a link home) in
//! the look of the directory listing.
//!
//! API clients are unchanged: [`negotiated`] turns an
//! `application/problem+json` response into this page only when the
//! request's `Accept` prefers `text/html` (a browser navigation). Under a
//! `Content-Security-Policy` that forbids inline styles the page still
//! reads, unstyled.

use crate::request::Request;
use crate::response::Response;

/// Whether the request prefers HTML: `Accept` lists `text/html` (not with
/// `q=0`) ahead of any JSON type. `*/*` alone (fetch, curl) is not HTML.
pub fn wants_html(request: &Request) -> bool {
    let Some(accept) = request.header("Accept") else { return false };
    for part in accept.value.split(',') {
        let mut fields = part.split(';');
        let kind = fields.next().unwrap_or("").trim().to_ascii_lowercase();
        let refused = fields.any(|f| {
            let f = f.trim();
            f.strip_prefix("q=").is_some_and(|q| q.trim().parse::<f32>().is_ok_and(|q| q == 0.0))
        });
        if kind.contains("json") {
            return false;
        }
        if kind == "text/html" || kind == "application/xhtml+xml" {
            return !refused;
        }
    }
    false
}

/// The sentence under the status for a code.
fn sentence(status: i16, detail: &str) -> String {
    match status {
        404 => "There is nothing at this address. Check the link for typos, or start from the home page.".to_string(),
        403 => "You don't have access to this page.".to_string(),
        401 => "This page needs you to sign in.".to_string(),
        429 => "Too many requests from your address. Wait a moment, then try again.".to_string(),
        502 => "The server behind this site didn't answer properly. Try again in a moment.".to_string(),
        504 => "The server behind this site took too long to answer. Try again in a moment.".to_string(),
        503 if detail == "maintenance" => maintenance_message(),
        503 => "This site can't answer right now. Try again in a moment.".to_string(),
        s if s >= 500 => "Something went wrong on the server. Try again in a moment.".to_string(),
        _ => "The request couldn't be completed.".to_string(),
    }
}

/// `[maintenance] message` (`RWS_CONFIG_MAINTENANCE_MESSAGE`), or the default.
pub fn maintenance_message() -> String {
    std::env::var("RWS_CONFIG_MAINTENANCE_MESSAGE")
        .ok()
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "This site is down for maintenance. It will be back shortly.".to_string())
}

/// `Retry-After` seconds sent with maintenance responses
/// (`RWS_CONFIG_MAINTENANCE_RETRY_AFTER_SECS`, default 120; 0 sends none).
pub fn maintenance_retry_after() -> u64 {
    std::env::var("RWS_CONFIG_MAINTENANCE_RETRY_AFTER_SECS").ok().and_then(|v| v.trim().parse().ok()).unwrap_or(120)
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The page for `status`/`reason`; `detail` picks the sentence
/// (`"maintenance"`) and is otherwise not shown (it may be technical).
pub fn html(status: i16, reason: &str, detail: &str) -> String {
    let title = if status == 503 && detail == "maintenance" { "Down for maintenance".to_string() } else { reason.to_string() };
    format!(
        concat!(
            "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n",
            "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n",
            "<meta name=\"robots\" content=\"noindex\">\n",
            "<title>{status} {title}</title>\n<style>\n{css}</style>\n</head>\n<body>\n",
            "<main>\n<p class=\"code\">{status}</p>\n<h1>{title}</h1>\n<p>{sentence}</p>\n",
            "<p><a href=\"/\">Go to the home page</a></p>\n</main>\n",
            "<footer>served by rws</footer>\n</body>\n</html>\n",
        ),
        status = status,
        title = escape(&title),
        sentence = escape(&sentence(status, detail)),
        css = CSS,
    )
}

const CSS: &str = ":root{color-scheme:light dark;--bg:#f7f7f9;--text:#1b1f27;--muted:#4b5563;--accent:#c2410c}
@media (prefers-color-scheme:dark){:root{--bg:#0b0d12;--text:#e7e9ee;--muted:#a3acb9;--accent:#fb923c}}
*{box-sizing:border-box}
body{margin:0;min-height:100vh;display:flex;flex-direction:column;justify-content:center;background:var(--bg);color:var(--text);font:16px/1.55 -apple-system,BlinkMacSystemFont,\"Segoe UI\",Inter,Roboto,sans-serif;padding:2rem 1rem}
main{max-width:34rem;margin:0 auto;width:100%}
.code{margin:0;font:600 .85rem/1 ui-monospace,SFMono-Regular,Menlo,monospace;letter-spacing:.08em;color:var(--muted)}
h1{margin:.4rem 0 .75rem;font-size:1.75rem;line-height:1.2;letter-spacing:-.02em}
p{margin:0 0 1rem;color:var(--muted)}
a{color:var(--accent);font-weight:600;text-underline-offset:3px;display:inline-block;padding:.5rem 0}
a:focus-visible{outline:2px solid var(--accent);outline-offset:3px;border-radius:4px}
footer{margin-top:3rem;text-align:center;color:var(--muted);font-size:.8rem}
";

/// `response` as the HTML page when it is an rws-made problem details
/// response (`application/problem+json`) and `request` prefers HTML;
/// otherwise unchanged.
pub fn negotiated(mut response: Response, request: &Request) -> Response {
    if !wants_html(request) {
        return response;
    }
    let Some(part) = response.content_range_list.first() else { return response };
    if part.content_type != super::problem::PROBLEM_JSON || response.content_range_list.len() != 1 {
        return response;
    }
    let body = String::from_utf8_lossy(&part.body);
    let detail = body
        .split_once("\"detail\":\"")
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(d, _)| d.to_string())
        .unwrap_or_default();
    let page = html(response.status_code, &response.reason_phrase, &detail);
    response.content_range_list = vec![crate::range::Range::get_content_range(page.into_bytes(), crate::mime_type::MimeType::TEXT_HTML.to_string())];
    if !response.headers.iter().any(|h| h.name.eq_ignore_ascii_case("Vary") && h.value.to_ascii_lowercase().contains("accept")) {
        response.headers.push(crate::header::Header { name: "Vary".into(), value: "Accept".into() });
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(accept: Option<&str>) -> Request {
        let head = match accept {
            Some(a) => format!("GET / HTTP/1.1\r\nHost: x\r\nAccept: {a}\r\n\r\n"),
            None => "GET / HTTP/1.1\r\nHost: x\r\n\r\n".to_string(),
        };
        Request::parse(head.as_bytes()).unwrap()
    }

    #[test]
    fn browsers_get_html_api_clients_json() {
        assert!(wants_html(&request(Some("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"))));
        assert!(!wants_html(&request(Some("*/*"))));
        assert!(!wants_html(&request(Some("application/json, text/html"))));
        assert!(!wants_html(&request(Some("text/html;q=0"))));
        assert!(!wants_html(&request(None)));
    }

    #[test]
    fn problem_json_becomes_a_page_for_browsers_only() {
        let browser = request(Some("text/html"));
        let r = negotiated(crate::error::problem(crate::response::STATUS_CODE_REASON_PHRASE.n502_bad_gateway, ""), &browser);
        let body = String::from_utf8_lossy(&r.content_range_list[0].body).to_string();
        assert_eq!(r.content_range_list[0].content_type, "text/html");
        assert!(body.contains("<title>502 Bad Gateway</title>") && body.contains("href=\"/\""));
        let api = negotiated(crate::error::problem(crate::response::STATUS_CODE_REASON_PHRASE.n502_bad_gateway, ""), &request(Some("application/json")));
        assert_eq!(api.content_range_list[0].body, br#"{"type":"about:blank","title":"Bad Gateway","status":502}"#.to_vec());
    }

    #[test]
    fn maintenance_has_its_own_title() {
        let page = html(503, "Service Unavailable", "maintenance");
        assert!(page.contains("Down for maintenance"));
    }
}
