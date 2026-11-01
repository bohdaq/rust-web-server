//! TOML strings for the two config-file readers (settings here, proxy routes
//! in `proxy_config::parser`): a basic string `"…"` decodes its backslash
//! escapes, a literal string `'…'` is taken as written, and [`Quotes`] lets a
//! scanner skip over both without ending a string at an escaped quote.

/// Where a scan over one line of TOML is: outside strings, or inside a basic
/// (`"…"`, backslash escapes) or literal (`'…'`, none) string.
#[derive(Default, Clone, Copy)]
pub(crate) struct Quotes {
    quote: Option<char>,
    escaped: bool,
}

impl Quotes {
    /// Feed the next character: `true` when it belongs to a quoted string,
    /// its quotes included, so a scanner acts on it only when `false`.
    pub(crate) fn step(&mut self, c: char) -> bool {
        match self.quote {
            Some(q) => {
                if self.escaped {
                    self.escaped = false;
                } else if q == '"' && c == '\\' {
                    self.escaped = true;
                } else if c == q {
                    self.quote = None;
                }
                true
            }
            None if c == '"' || c == '\'' => {
                self.quote = Some(c);
                true
            }
            None => false,
        }
    }
}

/// A value as written in the file, as the string it means: a basic string
/// without its quotes and with its escapes decoded, a literal string without
/// its quotes, anything else (numbers, booleans, bare words) unchanged.
pub(crate) fn unquote(s: &str) -> String {
    if s.len() >= 2 && s.starts_with('\'') && s.ends_with('\'') {
        return s[1..s.len() - 1].to_string();
    }
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        return unescape(&s[1..s.len() - 1]);
    }
    s.to_string()
}

/// TOML's escapes (`\n \r \t \b \f \e \" \\ \uXXXX \UXXXXXXXX`). A backslash
/// that starts none of them is kept with what follows, as written: TOML calls
/// that an error, and values such as `"^/v\d+/"` worked before escapes were
/// decoded.
fn unescape(s: &str) -> String {
    if !s.contains('\\') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('\\') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let mut chars = after.chars();
        let decoded = match chars.next() {
            Some('n') => Some(('\n', 1)),
            Some('r') => Some(('\r', 1)),
            Some('t') => Some(('\t', 1)),
            Some('b') => Some(('\u{8}', 1)),
            Some('f') => Some(('\u{c}', 1)),
            Some('e') => Some(('\u{1b}', 1)),
            Some('"') => Some(('"', 1)),
            Some('\\') => Some(('\\', 1)),
            Some('u') => hex_char(&after[1..], 4).map(|c| (c, 5)),
            Some('U') => hex_char(&after[1..], 8).map(|c| (c, 9)),
            _ => None,
        };
        match decoded {
            Some((c, len)) => {
                out.push(c);
                rest = &after[len..];
            }
            None => {
                out.push('\\');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The character named by the first `digits` hex digits of `s`, if they are
/// hex digits and name a Unicode scalar value.
fn hex_char(s: &str, digits: usize) -> Option<char> {
    let hex = s.get(..digits)?;
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    char::from_u32(u32::from_str_radix(hex, 16).ok()?)
}
