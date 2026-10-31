use std::cell::RefCell;
use std::sync::Arc;

/// The authenticated caller of the current request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    subject: Arc<str>,
    source: &'static str,
    claims: Option<Arc<str>>,
}

impl Identity {
    /// A caller named `subject` (`"user:alice"` — a type, `:`, an id; a
    /// bare id gets the type `user`), authenticated by `source` (`"jwt"`,
    /// `"oidc"`, `"saml"`, `"basic"`, `"forward"`, `"mcp"`, or your own).
    pub fn new(subject: impl AsRef<str>, source: &'static str) -> Identity {
        let subject = subject.as_ref();
        let subject: Arc<str> = if subject.contains(':') { Arc::from(subject) } else { Arc::from(format!("user:{subject}")) };
        Identity { subject, source, claims: None }
    }

    /// The same, carrying the token's or assertion's claims (a JSON object).
    pub fn with_claims(mut self, claims: impl AsRef<str>) -> Identity {
        self.claims = Some(Arc::from(claims.as_ref()));
        self
    }

    /// An identity from a JSON claims object: its `claim` (`"sub"` usually) as
    /// the subject id. `None` when the claim is missing or not a string.
    pub fn from_claims(claims: &str, claim: &str, source: &'static str) -> Option<Identity> {
        let value = claim_value(claims, claim)?;
        (!value.is_empty()).then(|| Identity::new(value, source).with_claims(claims))
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
        self.subject.split_once(':').map_or("user", |(t, _)| t)
    }

    /// The id part of the subject (`alice`).
    pub fn subject_id(&self) -> &str {
        self.subject.split_once(':').map_or(&self.subject, |(_, id)| id)
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
pub(crate) fn claim_value(claims: &str, name: &str) -> Option<String> {
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
pub(crate) fn enter(identity: Identity) -> IdentityGuard {
    IdentityGuard(CURRENT.with(|c| c.borrow_mut().replace(identity)))
}

/// Puts the previous identity back when dropped.
pub(crate) struct IdentityGuard(Option<Identity>);

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
