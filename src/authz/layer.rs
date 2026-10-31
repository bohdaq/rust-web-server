//! [`AuthzLayer`]: a route allows a request when its caller has a relation
//! to an object named from the request.

use std::sync::Arc;

use super::{Authorizer, Identity, ObjectRef};
use crate::application::Application;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

/// An object name built from the request: literal text with `{path:N}`
/// (the Nth path segment, from 1, percent-decoded), `{query:name}` and
/// `{header:Name}`. `doc:{path:2}` on `/docs/42/edit` is `doc:42`. Query
/// parameters and headers come from the client as sent — use a header only
/// when something in front (forward auth's `copy_header`) sets it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectTemplate {
    parts: Vec<Part>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Part {
    Literal(String),
    Path(usize),
    Query(String),
    Header(String),
}

impl ObjectTemplate {
    pub fn parse(text: &str) -> Result<ObjectTemplate, String> {
        let mut parts = Vec::new();
        let mut rest = text;
        while let Some(open) = rest.find('{') {
            if open > 0 {
                parts.push(Part::Literal(rest[..open].to_string()));
            }
            let close = rest[open..].find('}').ok_or_else(|| format!("object {text:?}: '{{' without '}}'"))? + open;
            let inner = &rest[open + 1..close];
            parts.push(match inner.split_once(':') {
                Some(("path", n)) => match n.parse::<usize>() {
                    Ok(n) if n >= 1 => Part::Path(n),
                    _ => return Err(format!("object {text:?}: {{path:N}} needs N ≥ 1")),
                },
                Some(("query", name)) if !name.is_empty() => Part::Query(name.to_string()),
                Some(("header", name)) if !name.is_empty() => Part::Header(name.to_string()),
                _ => return Err(format!("object {text:?}: unknown placeholder {{{inner}}} (use path:N, query:name or header:Name)")),
            });
            rest = &rest[close + 1..];
        }
        if !rest.is_empty() {
            parts.push(Part::Literal(rest.to_string()));
        }
        if rest.contains('}') || parts.is_empty() {
            return Err(format!("object {text:?}: stray '}}' or empty"));
        }
        let template = ObjectTemplate { parts };
        if template.object_type().is_none() {
            return Err(format!("object {text:?}: must start with a literal type, e.g. doc:{{path:2}}"));
        }
        Ok(template)
    }

    /// The literal type before the first `:`.
    pub fn object_type(&self) -> Option<&str> {
        match self.parts.first() {
            Some(Part::Literal(text)) => text.split_once(':').map(|(kind, _)| kind).filter(|k| !k.is_empty()),
            _ => None,
        }
    }

    /// The object for `request`; `None` when a placeholder has no value or
    /// the result isn't a valid object name.
    ///
    /// Path segments are percent-decoded once. A path with a `.` or `..`
    /// segment (raw or encoded), or a segment that decodes to something with
    /// a `/`, names nothing: a proxy passes the path on as it came, and a
    /// backend that resolves `/docs/42/../7` would serve doc 7 after a check
    /// on doc 42.
    pub fn render(&self, request: &Request) -> Option<ObjectRef> {
        let raw = crate::url::URL::request_target_path(&request.request_uri).unwrap_or("/");
        let mut segments = Vec::new();
        for segment in raw.split('/').filter(|s| !s.is_empty()) {
            let decoded = crate::app::controller::static_resource::percent_decode_path(segment).ok()?.into_owned();
            if decoded == "." || decoded == ".." || decoded.contains('/') || decoded.contains('\\') {
                return None;
            }
            segments.push(decoded);
        }
        let mut out = String::new();
        for part in &self.parts {
            match part {
                Part::Literal(text) => out.push_str(text),
                Part::Path(n) => out.push_str(segments.get(n - 1)?),
                Part::Query(name) => {
                    let query = request.request_uri.split_once('?')?.1;
                    let value = query.split('&').find_map(|pair| pair.split_once('=').filter(|(k, _)| k == name).map(|(_, v)| v))?;
                    if value.is_empty() {
                        return None;
                    }
                    out.push_str(value);
                }
                Part::Header(name) => out.push_str(request.header(name).map(|h| h.value.trim()).filter(|v| !v.is_empty())?),
            }
        }
        ObjectRef::parse(&out).ok()
    }
}

type Pick = Arc<dyn Fn(&Request) -> Option<(String, ObjectRef)> + Send + Sync>;

/// Middleware that lets a request through when its caller
/// ([`Identity::current`], set by an authenticator wrapped before — outside —
/// this layer) has
/// a relation to an object: `401` without a caller, `403` when the answer
/// is no, the object can't be named, or the check fails (logged).
///
/// ```rust,no_run
/// use rust_web_server::app::App;
/// use rust_web_server::authz::{AuthzLayer, Authorizer, MemoryStore, Schema};
/// use rust_web_server::core::New;
///
/// let schema = Schema::new().relation("doc", "viewer", "user").unwrap();
/// let authz = Authorizer::new(schema, MemoryStore::new()).unwrap();
/// // The first layer wrapped is the outermost: wrap the authenticator first —
/// // e.g. `.wrap(JwtLayer::new(secret))` (the `auth` feature) — so it has set
/// // the caller by the time this layer checks.
/// let app = App::new().wrap(AuthzLayer::new(authz, "viewer", "doc:{path:2}").unwrap()); // /docs/42 → doc:42
/// ```
#[derive(Clone)]
pub struct AuthzLayer {
    authz: Authorizer,
    pick: Pick,
}

impl std::fmt::Debug for AuthzLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthzLayer").finish_non_exhaustive()
    }
}

impl AuthzLayer {
    /// Require `relation` on the object `template` names (see
    /// [`ObjectTemplate`]); the relation must exist on the template's type.
    pub fn new(authz: Authorizer, relation: &str, template: &str) -> Result<AuthzLayer, String> {
        let template = ObjectTemplate::parse(template)?;
        let kind = template.object_type().unwrap_or_default().to_string();
        if !authz.schema().has_relation(&kind, relation) {
            return Err(format!("{kind}#{relation} isn't defined in the schema"));
        }
        let relation = relation.to_string();
        Ok(AuthzLayer { authz, pick: Arc::new(move |request| Some((relation.clone(), template.render(request)?))) })
    }

    /// Decide per request which relation on which object is required;
    /// `None` denies.
    pub fn with(authz: Authorizer, pick: impl Fn(&Request) -> Option<(String, ObjectRef)> + Send + Sync + 'static) -> AuthzLayer {
        AuthzLayer { authz, pick: Arc::new(pick) }
    }
}

impl Middleware for AuthzLayer {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        let Some(caller) = Identity::current() else {
            return Ok(denied(crate::response::STATUS_CODE_REASON_PHRASE.n401_unauthorized, "authentication required"));
        };
        let Some((relation, object)) = (self.pick)(request) else {
            return Ok(denied(crate::response::STATUS_CODE_REASON_PHRASE.n403_forbidden, "forbidden"));
        };
        match self.authz.check_subject(&caller.as_subject(), &relation, &object) {
            Ok(true) => next.execute(request, connection),
            Ok(false) => Ok(denied(crate::response::STATUS_CODE_REASON_PHRASE.n403_forbidden, "forbidden")),
            Err(e) => {
                eprintln!("authz: {} {relation} {object}: {e}; denied", caller.subject());
                Ok(denied(crate::response::STATUS_CODE_REASON_PHRASE.n403_forbidden, "forbidden"))
            }
        }
    }

    fn accepts_streamed_body(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }
}

fn denied(status: &crate::response::StatusCodeReasonPhrase, message: &str) -> Response {
    crate::error::problem::problem_with_text(status, message, &format!(r#"{{"error":"{message}"}}"#))
}
