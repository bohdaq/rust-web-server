use std::cell::RefCell;
use std::sync::Arc;

/// The authenticated caller of the current request: an object (a type and
/// an id) — never a set like `group:eng#member`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    kind: Arc<str>,
    id: Arc<str>,
    subject: Arc<str>,
    source: &'static str,
    claims: Option<Arc<str>>,
}

impl Identity {
    /// A caller you name in code: `"service:billing"` (a type, `:`, an id)
    /// or a bare id, which gets the type `user`. `source` says who vouched
    /// (`"jwt"`, `"oidc"`, `"saml"`, `"basic"`, `"forward"`, `"mcp"`, or your
    /// own). For a value that came from outside, use [`Identity::user`].
    pub fn new(subject: impl AsRef<str>, source: &'static str) -> Identity {
        let subject = subject.as_ref();
        let (kind, id) = subject.split_once(':').unwrap_or(("user", subject));
        Identity::typed(kind, id, source)
    }

    fn typed(kind: &str, id: &str, source: &'static str) -> Identity {
        Identity { kind: Arc::from(kind), id: Arc::from(id), subject: Arc::from(format!("{kind}:{id}")), source, claims: None }
    }

    /// The user whose id is `id` exactly — what authenticators use for a
    /// value from a token, a header or a login. A `:` stays part of the id
    /// (`user:a:b`). `None` when `id` can't name an object: empty, over 256
    /// characters, or holding `#`, `@`, a space or a control character —
    /// so a token whose `sub` is `group:eng#member` can't pass as that group.
    pub fn user(id: &str, source: &'static str) -> Option<Identity> {
        crate::authz::rebac::ObjectRef::parse(&format!("user:{id}")).ok()?;
        Some(Identity::typed("user", id, source))
    }

    /// The same, carrying the token's or assertion's claims (a JSON object).
    pub fn with_claims(mut self, claims: impl AsRef<str>) -> Identity {
        self.claims = Some(Arc::from(claims.as_ref()));
        self
    }

    /// An identity from a JSON claims object: its `claim` (`"sub"` usually) as
    /// the subject id. `None` when the claim is missing or not a string.
    pub fn from_claims(claims: &str, claim: &str, source: &'static str) -> Option<Identity> {
        Some(Identity::user(&claim_value(claims, claim)?, source)?.with_claims(claims))
    }

    /// The caller of the request being served on this thread or task.
    pub fn current() -> Option<Identity> {
        #[cfg(feature = "http2")]
        if let Ok(Some(identity)) = TASK.try_with(|i| i.clone()) {
            return Some(identity);
        }
        CURRENT.with(|c| c.borrow().clone())
    }

    /// `type:id`, e.g. `user:alice`.
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// The type part of the subject (`user`).
    pub fn subject_type(&self) -> &str {
        &self.kind
    }

    /// The id part of the subject (`alice`).
    pub fn subject_id(&self) -> &str {
        &self.id
    }

    /// The caller as a tuple subject: an object, never a set.
    pub fn as_subject(&self) -> crate::authz::Subject {
        crate::authz::Subject { object: crate::authz::ObjectRef { kind: self.kind.to_string(), id: self.id.to_string() }, relation: None }
    }

    /// Which authenticator vouched for it.
    pub fn source(&self) -> &'static str {
        self.source
    }

    /// The raw claims JSON, when the authenticator had any.
    pub fn claims(&self) -> Option<&str> {
        self.claims.as_deref()
    }

    /// One claim as a string (top-level members only).
    pub fn claim(&self, name: &str) -> Option<String> {
        claim_value(self.claims.as_deref()?, name)
    }
}

/// A top-level string member of a JSON object.
#[doc(hidden)]
pub fn claim_value(claims: &str, name: &str) -> Option<String> {
    let members = crate::mcp::json_rpc::object_members(claims)?;
    let (_, raw) = members.into_iter().find(|(key, _)| key == name)?;
    raw.trim().starts_with('"').then(|| crate::mcp::json_rpc::extract_str(&format!("{{\"v\":{raw}}}"), "v")).flatten()
}

thread_local! {
    static CURRENT: RefCell<Option<Identity>> = const { RefCell::new(None) };
}

#[cfg(feature = "http2")]
tokio::task_local! {
    static TASK: Option<Identity>;
}

/// Run `f` as `identity` (what [`Identity::current`] returns inside it); the
/// previous one comes back afterwards, also on panic. Authenticators call
/// this around the rest of the request.
pub fn with_identity<R>(identity: Identity, f: impl FnOnce() -> R) -> R {
    let _guard = enter(identity);
    f()
}

/// This thread runs as `identity` until the guard is dropped.
#[doc(hidden)]
pub fn enter(identity: Identity) -> IdentityGuard {
    IdentityGuard(CURRENT.with(|c| c.borrow_mut().replace(identity)))
}

/// Puts the previous identity back when dropped.
#[doc(hidden)]
pub struct IdentityGuard(Option<Identity>);

impl Drop for IdentityGuard {
    fn drop(&mut self) {
        let previous = self.0.take();
        CURRENT.with(|c| *c.borrow_mut() = previous);
    }
}

/// [`with_identity`] for async code: `future` runs as `identity`.
#[cfg(feature = "http2")]
pub async fn scope_identity<F: std::future::Future>(identity: Identity, future: F) -> F::Output {
    TASK.scope(Some(identity), future).await
}
