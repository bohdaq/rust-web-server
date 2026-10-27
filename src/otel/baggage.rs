//! W3C Baggage (<https://www.w3.org/TR/baggage/>): application key/value
//! pairs that travel with a request across services in the `baggage` header.
//!
//! [`super::OtelLayer`] reads the incoming header into the request's context;
//! [`super::current_baggage`] reads it in a handler and [`super::set_baggage`]
//! adds to it; outbound calls made under that context (the HTTP client, the
//! proxies) send it on. Values are percent-decoded here and encoded again on
//! the way out.

/// Limits from the specification: at most 64 list-members and 8192 bytes in
/// a header; anything beyond is dropped, not an error.
pub const MAX_MEMBERS: usize = 64;
pub const MAX_BYTES: usize = 8192;

/// One `key=value;properties` member: `value` decoded, `properties` raw.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Baggage {
    entries: Vec<(String, String, String)>,
}

impl Baggage {
    /// Parse a `baggage` header value. Malformed members are skipped; members
    /// past [`MAX_MEMBERS`] or [`MAX_BYTES`] are dropped.
    pub fn parse(header: &str) -> Baggage {
        let mut baggage = Baggage::default();
        let mut bytes = 0;
        for member in header.split(',') {
            let member = member.trim_matches(|c| c == ' ' || c == '\t');
            if member.is_empty() {
                continue;
            }
            if baggage.entries.len() == MAX_MEMBERS || bytes + member.len() > MAX_BYTES {
                break;
            }
            let (pair, properties) = match member.split_once(';') {
                Some((pair, properties)) => (pair, properties.trim_matches(|c| c == ' ' || c == '\t')),
                None => (member, ""),
            };
            let Some((key, value)) = pair.split_once('=') else { continue };
            let key = key.trim_matches(|c| c == ' ' || c == '\t');
            let value = value.trim_matches(|c| c == ' ' || c == '\t');
            if !is_token(key) || !value.bytes().all(is_baggage_octet) {
                continue;
            }
            let Some(value) = percent_decode(value) else { continue };
            bytes += member.len() + 1;
            baggage.set_with_properties(key, &value, properties);
        }
        baggage
    }

    /// The header value: members joined by `,`, values percent-encoded.
    pub fn to_header(&self) -> String {
        let mut out = String::new();
        for (key, value, properties) in &self.entries {
            let member = if properties.is_empty() {
                format!("{key}={}", percent_encode(value))
            } else {
                format!("{key}={};{properties}", percent_encode(value))
            };
            if out.len() + member.len() + 1 > MAX_BYTES {
                break;
            }
            if !out.is_empty() {
                out.push(',');
            }
            out.push_str(&member);
        }
        out
    }

    /// The (decoded) value of `key`.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.iter().find(|(k, _, _)| k == key).map(|(_, v, _)| v.as_str())
    }

    /// Set `key` (replacing an existing value). A key that isn't an HTTP
    /// token, or a member past [`MAX_MEMBERS`], is ignored; returns whether it
    /// was set.
    pub fn set(&mut self, key: &str, value: &str) -> bool {
        self.set_with_properties(key, value, "")
    }

    fn set_with_properties(&mut self, key: &str, value: &str, properties: &str) -> bool {
        if !is_token(key) {
            return false;
        }
        if let Some(entry) = self.entries.iter_mut().find(|(k, _, _)| k == key) {
            entry.1 = value.to_string();
            entry.2 = properties.to_string();
            return true;
        }
        if self.entries.len() >= MAX_MEMBERS {
            return false;
        }
        self.entries.push((key.to_string(), value.to_string(), properties.to_string()));
        true
    }

    /// Remove `key`.
    pub fn remove(&mut self, key: &str) {
        self.entries.retain(|(k, _, _)| k != key);
    }

    /// `(key, value)` pairs in order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|(k, v, _)| (k.as_str(), v.as_str()))
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// RFC 9110 token.
fn is_token(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}

/// `baggage-octet`: printable ASCII except space, `"`, `,`, `;` and `\`.
fn is_baggage_octet(b: u8) -> bool {
    matches!(b, 0x21 | 0x23..=0x2B | 0x2D..=0x3A | 0x3C..=0x5B | 0x5D..=0x7E)
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        if is_baggage_octet(b) && b != b'%' {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests;
