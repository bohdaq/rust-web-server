//! The admin UI: a lit-material app built from `admin-ui/` and embedded
//! here (`admin-ui/build.sh` writes `src/admin/ui/`, so `cargo build`
//! never needs Node). Served at `/admin/` without the token — the files hold
//! no secrets; the app asks for the token and sends it to the API.

use crate::header::Header;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

const INDEX: &[u8] = include_bytes!("ui/index.html");
const SCRIPT: &[u8] = include_bytes!("ui/admin.js");
const STYLE: &[u8] = include_bytes!("ui/admin.css");

/// Scripts and styles only from the listener itself; fonts are inlined.
/// `style-src` allows inline style attributes, which components set; no
/// inline scripts anywhere.
pub(crate) const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; font-src 'self' data:; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

/// The UI file at `path`, if it is one.
pub(crate) fn file(path: &str) -> Option<Response> {
    let (body, content_type) = match path {
        "/admin/" | "/admin/index.html" => (INDEX, "text/html; charset=utf-8"),
        "/admin/admin.js" => (SCRIPT, "text/javascript; charset=utf-8"),
        "/admin/admin.css" => (STYLE, "text/css; charset=utf-8"),
        _ => return None,
    };
    let range = crate::range::Range::get_content_range(body.to_vec(), content_type.to_string());
    let mut response = Response::get_response(STATUS_CODE_REASON_PHRASE.n200_ok, None, Some(vec![range]));
    for (name, value) in [
        ("Content-Security-Policy", CSP),
        ("X-Content-Type-Options", "nosniff"),
        ("Referrer-Policy", "no-referrer"),
        ("Cache-Control", "no-cache"),
        ("Cross-Origin-Opener-Policy", "same-origin"),
    ] {
        response.headers.push(Header::new(name, value));
    }
    Some(response)
}

/// `GET /admin` → `/admin/`.
pub(crate) fn redirect() -> Response {
    let mut response = Response::get_response(STATUS_CODE_REASON_PHRASE.n308_permanent_redirect, None, None);
    response.headers.push(Header::new("Location", "/admin/"));
    response
}
