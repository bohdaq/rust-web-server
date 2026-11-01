//! The JSON string escaper every module uses (18.24.0; before, several
//! modules had their own, some of which left control characters raw).

/// `s` as the inside of a JSON string literal (RFC 8259 §7): quote,
/// backslash and every control character escaped.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = std::fmt::Write::write_fmt(&mut out, format_args!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn escapes_quotes_backslashes_and_controls() {
        assert_eq!(super::escape("a\"b\\c\n\u{1}é"), "a\\\"b\\\\c\\n\\u0001é");
    }
}
