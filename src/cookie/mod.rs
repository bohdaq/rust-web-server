#[cfg(test)]
mod tests;

#[cfg(feature = "crypto")]
mod crypto_ext;
#[cfg(feature = "crypto")]
pub use crypto_ext::{decrypt_cookie, encrypted_cookie, signed_cookie, verify_signed_cookie};

/// A single HTTP cookie name/value pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cookie {
    pub name: String,
    pub value: String,
}

/// Parses the `Cookie` request header into a collection of [`Cookie`] values.
///
/// # Example
/// ```
/// use rust_web_server::cookie::CookieJar;
///
/// let jar = CookieJar::parse("session=abc123; theme=dark");
/// assert_eq!(jar.get("session").unwrap().value, "abc123");
/// ```
pub struct CookieJar {
    pub cookies: Vec<Cookie>,
}

impl CookieJar {
    /// Parses the raw value of the `Cookie` header (e.g. `"a=1; b=2"`).
    pub fn parse(header_value: &str) -> CookieJar {
        let cookies = header_value
            .split(';')
            .filter_map(|pair| {
                let pair = pair.trim();
                let mut parts = pair.splitn(2, '=');
                let name = parts.next()?.trim().to_string();
                let value = parts.next().unwrap_or("").trim().to_string();
                if name.is_empty() { None } else { Some(Cookie { name, value }) }
            })
            .collect();
        CookieJar { cookies }
    }

    /// Returns the first cookie with the given name, or `None`.
    pub fn get(&self, name: &str) -> Option<&Cookie> {
        self.cookies.iter().find(|c| c.name == name)
    }
}

/// Builder for the `Set-Cookie` response header value.
///
/// # Example
/// ```
/// use rust_web_server::cookie::SetCookie;
///
/// let header_value = SetCookie::new("session", "abc123")
///     .path("/")
///     .http_only()
///     .secure()
///     .same_site("Lax")
///     .build();
///
/// assert!(header_value.starts_with("session=abc123"));
/// assert!(header_value.contains("HttpOnly"));
/// ```
pub struct SetCookie {
    pub name: String,
    pub value: String,
    pub path: Option<String>,
    pub domain: Option<String>,
    pub max_age: Option<i64>,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<String>,
}

impl SetCookie {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        SetCookie {
            name: name.into(),
            value: value.into(),
            path: None,
            domain: None,
            max_age: None,
            secure: false,
            http_only: false,
            same_site: None,
        }
    }

    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    pub fn max_age(mut self, seconds: i64) -> Self {
        self.max_age = Some(seconds);
        self
    }

    pub fn secure(mut self) -> Self {
        self.secure = true;
        self
    }

    pub fn http_only(mut self) -> Self {
        self.http_only = true;
        self
    }

    pub fn same_site(mut self, policy: impl Into<String>) -> Self {
        self.same_site = Some(policy.into());
        self
    }

    /// Builds the `Set-Cookie` header value string.
    pub fn build(&self) -> String {
        let mut s = format!("{}={}", self.name, self.value);
        if let Some(ref p) = self.path { s.push_str(&format!("; Path={}", p)); }
        if let Some(ref d) = self.domain { s.push_str(&format!("; Domain={}", d)); }
        if let Some(age) = self.max_age { s.push_str(&format!("; Max-Age={}", age)); }
        if self.secure { s.push_str("; Secure"); }
        if self.http_only { s.push_str("; HttpOnly"); }
        if let Some(ref ss) = self.same_site { s.push_str(&format!("; SameSite={}", ss)); }
        s
    }
}
