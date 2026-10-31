//! RFC 8941 structured-field pieces for the headers rws writes
//! (`Proxy-Status`, `Cache-Status`).

/// A token as is, anything else as a string (RFC 8941).
pub(crate) fn item(name: &str) -> String {
    let token = !name.is_empty()
        && name.as_bytes()[0].is_ascii_alphabetic()
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b":/!#$%&'*+-.^_`|~".contains(&b));
    if token {
        name.to_string()
    } else {
        string(name)
    }
}

/// An RFC 8941 string: printable ASCII, `"` and `\` escaped, the rest
/// dropped (header values must stay one safe line).
pub(crate) fn string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars().take(256) {
        match c {
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            ' '..='~' => out.push(c),
            _ => {}
        }
    }
    out.push('"');
    out
}
