//! The account forms: the app's templates when it gave some, else plain
//! built-in HTML.

use super::{local_path, AuthFlows};
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

fn html(body: String) -> Response {
    let content = crate::range::Range::get_content_range(body.into_bytes(), crate::mime_type::MimeType::TEXT_HTML.to_string());
    Response::get_response(STATUS_CODE_REASON_PHRASE.n200_ok, None, Some(vec![content]))
}

fn query(request: &Request, name: &str) -> String {
    super::Input::read(request).get(name).to_string()
}

/// The page for `rest` (`/login`, `/signup`, `/password/forgot`, `/password/reset`).
pub(super) fn page(flows: &AuthFlows, request: &Request, rest: &str) -> Response {
    let prefix = &flows.inner.prefix;
    let token = query(request, "token");
    let return_to = local_path(&query(request, "return_to")).unwrap_or("").to_string();
    let name = match rest {
        "/login" => "login",
        "/signup" => "signup",
        "/password/forgot" => "forgot",
        _ => "reset",
    };

    #[cfg(feature = "tera")]
    if let Some(engine) = &flows.inner.templates {
        let template = format!("auth/{name}.html");
        if engine.has_template(&template) {
            let mut ctx = crate::template::page_context(request);
            ctx.insert("prefix", prefix);
            ctx.insert("token", &token);
            ctx.insert("return_to", &return_to);
            return match engine.render(&template, &ctx) {
                Ok(body) => html(body),
                Err(e) => {
                    eprintln!("[auth] {template}: {e}");
                    crate::error::problem::problem(STATUS_CODE_REASON_PHRASE.n500_internal_server_error, "the page failed to render")
                }
            };
        }
    }

    let flash = crate::flash::Flash::read(request);
    let messages: String = flash.messages().iter().map(|m| format!("<p class=\"{}\">{}</p>", escape(&m.kind), escape(&m.text))).collect();
    let csrf = csrf_field(request);
    let hidden_return = if return_to.is_empty() { String::new() } else { format!("<input type=\"hidden\" name=\"return_to\" value=\"{}\">", escape(&return_to)) };
    let p = escape(prefix);
    let (title, form, links) = match name {
        "login" => (
            "Sign in",
            format!("<form method=\"post\" action=\"{p}/login\">{csrf}{hidden_return}<label>Email <input type=\"email\" name=\"email\" autocomplete=\"username\" required></label><label>Password <input type=\"password\" name=\"password\" autocomplete=\"current-password\" required></label><button>Sign in</button></form>"),
            format!("<a href=\"{p}/signup\">Create an account</a> · <a href=\"{p}/password/forgot\">Forgot your password?</a>"),
        ),
        "signup" => (
            "Create an account",
            format!("<form method=\"post\" action=\"{p}/signup\">{csrf}{hidden_return}<label>Email <input type=\"email\" name=\"email\" autocomplete=\"username\" required></label><label>Password <input type=\"password\" name=\"password\" autocomplete=\"new-password\" minlength=\"{}\" required></label><button>Create account</button></form>", flows.inner.min_password),
            format!("<a href=\"{p}/login\">Already have an account? Sign in</a>"),
        ),
        "forgot" => (
            "Reset your password",
            format!("<form method=\"post\" action=\"{p}/password/forgot\">{csrf}<label>Email <input type=\"email\" name=\"email\" autocomplete=\"username\" required></label><button>Email me a link</button></form>"),
            format!("<a href=\"{p}/login\">Back to sign in</a>"),
        ),
        _ => (
            "Choose a new password",
            format!("<form method=\"post\" action=\"{p}/password/reset\">{csrf}<input type=\"hidden\" name=\"token\" value=\"{}\"><label>New password <input type=\"password\" name=\"password\" autocomplete=\"new-password\" minlength=\"{}\" required></label><button>Change password</button></form>", escape(&token), flows.inner.min_password),
            format!("<a href=\"{p}/login\">Back to sign in</a>"),
        ),
    };
    html(format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{title}</title>\
         <style>body{{font:16px system-ui,sans-serif;max-width:24rem;margin:3rem auto;padding:0 1rem}}label{{display:block;margin:.75rem 0}}input{{display:block;width:100%;padding:.5rem;box-sizing:border-box}}button{{padding:.5rem 1rem;margin:.5rem 0}}.alert{{color:#b00020}}.notice{{color:#1b5e20}}</style></head>\
         <body><h1>{title}</h1>{messages}{form}<p>{links}</p></body></html>"
    ))
}

#[cfg(feature = "csrf")]
fn csrf_field(request: &Request) -> String {
    let token = crate::csrf::CsrfToken::from_request(request).map(|t| t.value().to_string()).or_else(|| {
        request.header("cookie").and_then(|h| crate::cookie::CookieJar::parse(&h.value).get("_csrf").map(|c| c.value.clone()))
    });
    match token {
        Some(token) => format!("<input type=\"hidden\" name=\"_csrf\" value=\"{}\">", escape(&token)),
        None => String::new(),
    }
}

#[cfg(not(feature = "csrf"))]
fn csrf_field(_request: &Request) -> String {
    String::new()
}
