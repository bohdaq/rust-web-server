//! Normalization before matching: what an attacker encodes, the WAF
//! decodes. Every loop here is bounded by its input (fuzzed).

/// The most of one value that is looked at.
pub const MAX_VALUE: usize = 16 * 1024;

/// Decode a value the way a browser, a server or a database might read it:
/// percent-encoding (twice, for double encoding; `%uXXXX` too), HTML
/// entities, `\xHH` / `\uXXXX` escapes; then NUL and control characters
/// removed, SQL `/* comments */` turned into spaces, lower-cased, runs of
/// whitespace collapsed.
pub fn normalize(input: &str) -> String {
    let mut s: String = input.chars().take(MAX_VALUE).collect();
    for _ in 0..2 {
        let next = percent_decode(&s);
        if next == s {
            break;
        }
        s = next;
    }
    s = html_entities(&s);
    s = js_escapes(&s);
    s = strip_sql_comments(&s);
    let mut out = String::with_capacity(s.len());
    let mut space = false;
    for c in s.chars() {
        let c = if c.is_whitespace() || c == '\u{0b}' || c == '\u{0c}' { ' ' } else { c };
        if c.is_control() {
            continue;
        }
        if c == ' ' {
            if !space {
                out.push(' ');
            }
            space = true;
        } else {
            out.extend(c.to_lowercase());
            space = false;
        }
    }
    out
}

fn hex(b: u8) -> Option<u32> {
    (b as char).to_digit(16)
}

/// `%HH` and `%uHHHH`; `+` is left alone (query values were form-decoded).
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            if matches!(b.get(i + 1), Some(b'u' | b'U')) {
                if let (Some(a), Some(c), Some(d), Some(e)) = (b.get(i + 2).and_then(|x| hex(*x)), b.get(i + 3).and_then(|x| hex(*x)), b.get(i + 4).and_then(|x| hex(*x)), b.get(i + 5).and_then(|x| hex(*x))) {
                    if let Some(ch) = char::from_u32(a << 12 | c << 8 | d << 4 | e) {
                        let mut buf = [0u8; 4];
                        out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        i += 6;
                        continue;
                    }
                }
            }
            if let (Some(h), Some(l)) = (b.get(i + 1).and_then(|x| hex(*x)), b.get(i + 2).and_then(|x| hex(*x))) {
                out.push((h << 4 | l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Named entities that matter for markup and scripts, and numeric ones
/// (`&#60;`, `&#x3c;`, with or without the `;`).
pub fn html_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    const NAMED: [(&str, char); 14] = [
        ("lt", '<'), ("gt", '>'), ("quot", '"'), ("apos", '\''), ("amp", '&'), ("colon", ':'), ("lpar", '('), ("rpar", ')'),
        ("tab", '\t'), ("newline", '\n'), ("sol", '/'), ("bsol", '\\'), ("equals", '='), ("nbsp", ' '),
    ];
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let mut consumed = None;
        if let Some(num) = after.strip_prefix('#') {
            let (digits, radix, skip) = match num.strip_prefix(['x', 'X']) {
                Some(h) => (h.chars().take_while(|c| c.is_ascii_hexdigit()).take(8).collect::<String>(), 16, 2),
                None => (num.chars().take_while(|c| c.is_ascii_digit()).take(8).collect::<String>(), 10, 1),
            };
            if !digits.is_empty() {
                if let Some(ch) = u32::from_str_radix(&digits, radix).ok().and_then(char::from_u32) {
                    out.push(ch);
                    let mut len = skip + digits.len();
                    if after[len..].starts_with(';') {
                        len += 1;
                    }
                    consumed = Some(len);
                }
            }
        } else {
            let name: String = after.chars().take_while(|c| c.is_ascii_alphabetic()).take(10).collect();
            if let Some((_, ch)) = NAMED.iter().find(|(n, _)| n.eq_ignore_ascii_case(&name)) {
                out.push(*ch);
                let mut len = name.len();
                if after[len..].starts_with(';') {
                    len += 1;
                }
                consumed = Some(len);
            }
        }
        match consumed {
            Some(len) => rest = &after[len..],
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// `\xHH` and `\uHHHH`.
pub fn js_escapes(s: &str) -> String {
    if !s.contains('\\') {
        return s.to_string();
    }
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            let take = match chars[i + 1] {
                'x' | 'X' => 2,
                'u' | 'U' => 4,
                _ => 0,
            };
            if take > 0 && i + 2 + take <= chars.len() {
                let digits: String = chars[i + 2..i + 2 + take].iter().collect();
                if let Some(ch) = u32::from_str_radix(&digits, 16).ok().and_then(char::from_u32) {
                    out.push(ch);
                    i += 2 + take;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// `/* … */` (and MySQL's `/*! … */`) become a space: `un/**/ion` → `un ion`
/// stays harmless, `union/**/select` → `union select` is caught.
pub fn strip_sql_comments(s: &str) -> String {
    if !s.contains("/*") {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        out.push(' ');
        let inner_and_rest = &rest[start + 2..];
        let (inner, after) = match inner_and_rest.find("*/") {
            Some(end) => (&inner_and_rest[..end], &inner_and_rest[end + 2..]),
            None => (inner_and_rest, ""),
        };
        // MySQL runs what's inside `/*! … */` (and `/*!50000 … */`): keep it,
        // or `union/*!select*/` would hide the SELECT (18.24.0).
        if let Some(code) = inner.strip_prefix('!') {
            out.push_str(code.trim_start_matches(|c: char| c.is_ascii_digit()));
            out.push(' ');
        }
        rest = after;
    }
    out.push_str(rest);
    out
}
