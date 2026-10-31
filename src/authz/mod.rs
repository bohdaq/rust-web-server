//! Authorization (18.14.0, #32): who the caller is ([`Identity`]), and —
//! from later tranches — what they may do, as relationship-based access
//! control (Zanzibar-style tuples and a schema).
//!
//! Every authenticator in rws hands the authenticated caller to what runs
//! after it as one [`Identity`]: `JwtLayer`, `OidcAuth`, `SamlSp`,
//! `BasicAuthLayer`, `ForwardAuthLayer` (with
//! [`identity_header`](crate::auth::forward::ForwardAuthLayer::identity_header))
//! and MCP bearer / OAuth tokens. A handler reads it with
//! [`Identity::current`]:
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::authz::Identity;
//! use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
//!
//! let app = App::with_state(()).get("/me", |_, _, _, _| match Identity::current() {
//!     Some(me) => Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, me.subject()),
//!     None => Response::text(STATUS_CODE_REASON_PHRASE.n401_unauthorized, "who are you?"),
//! });
//! ```
//!
//! The identity is not a request header: the authenticator runs the rest of
//! the request inside [`with_identity`], so a client can't send one.

pub(crate) mod identity;

pub use identity::{with_identity, Identity};
#[cfg(feature = "http2")]
pub use identity::scope_identity;

#[cfg(test)]
pub(crate) mod test_support {
    use crate::application::Application;
    use crate::request::Request;
    use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
    use crate::server::ConnectionInfo;

    /// Answers with the caller: `<subject> via <source>`, or `anonymous`.
    pub(crate) struct WhoAmI;

    impl Application for WhoAmI {
        fn execute(&self, _request: &Request, _connection: &ConnectionInfo) -> Result<Response, String> {
            let who = super::Identity::current().map_or("anonymous".to_string(), |i| format!("{} via {}", i.subject(), i.source()));
            Ok(Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, &who))
        }
    }

    pub(crate) fn body(response: &Response) -> String {
        String::from_utf8_lossy(&response.content_range_list.first().map(|c| c.body.clone()).unwrap_or_default()).to_string()
    }
}

#[cfg(test)]
mod tests;
