/// JSON-RPC 2.0 error codes.
#[allow(dead_code)]
pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
#[allow(dead_code)]
pub const INTERNAL_ERROR: i32 = -32603;

// ── field extractors ──────────────────────────────────────────────────────────

/// Extract a JSON string value for the given key.  Handles `\"` escapes.
pub fn extract_str(json: &str, key: &str) -> Option<String> {
    let kp = format!("\"{}\"", key);
    let mut search = json;
    while let Some(pos) = search.find(&kp) {
        let after = &search[pos + kp.len()..];
        let rest = after.trim_start_matches(|c: char| c.is_whitespace() || c == ':');
        if rest.starts_with('"') {
            let inner = &rest[1..];
            let mut result = String::new();
            let mut chars = inner.chars();
            loop {
                match chars.next()? {
                    '"' => return Some(result),
                    '\\' => match chars.next()? {
                        '"' => result.push('"'),
                        '\\' => result.push('\\'),
                        '/' => result.push('/'),
                        'n' => result.push('\n'),
                        'r' => result.push('\r'),
                        't' => result.push('\t'),
                        'u' => {
                            let hex: String = chars.by_ref().take(4).collect();
                            if let Ok(n) = u32::from_str_radix(&hex, 16) {
                                if let Some(c) = char::from_u32(n) {
                                    result.push(c);
                                }
                            }
                        }
                        c => result.push(c),
                    },
                    c => result.push(c),
                }
            }
        }
        // Key matched but wasn't followed by a string value — keep searching.
        search = &search[pos + kp.len()..];
    }
    None
}

/// Extract the raw JSON value (object, array, string, number, `true`, `false`, `null`)
/// for the given key.
pub fn extract_raw(json: &str, key: &str) -> Option<String> {
    let kp = format!("\"{}\"", key);
    let mut search = json;
    while let Some(pos) = search.find(&kp) {
        let after = &search[pos + kp.len()..];
        let rest = after.trim_start_matches(|c: char| c.is_whitespace() || c == ':');
        if let Some(v) = extract_value(rest) {
            return Some(v);
        }
        search = &search[pos + kp.len()..];
    }
    None
}

fn extract_value(s: &str) -> Option<String> {
    let first = s.chars().next()?;
    match first {
        '{' => bracket_extract(s, '{', '}'),
        '[' => bracket_extract(s, '[', ']'),
        '"' => {
            let mut end = 1usize;
            let bytes = s.as_bytes();
            while end < bytes.len() {
                match bytes[end] {
                    b'\\' => end += 2,
                    b'"' => { end += 1; break; }
                    _ => end += 1,
                }
            }
            Some(s[..end].to_string())
        }
        _ => {
            let end = s.find(|c: char| matches!(c, ',' | '}' | ']') || c.is_whitespace())
                .unwrap_or(s.len());
            let trimmed = s[..end].trim();
            if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
        }
    }
}

fn bracket_extract(s: &str, open: char, close: char) -> Option<String> {
    let mut depth = 0usize;
    let mut in_str = false;
    let mut prev_escape = false;
    for (i, ch) in s.char_indices() {
        if prev_escape { prev_escape = false; continue; }
        if in_str {
            if ch == '\\' { prev_escape = true; }
            else if ch == '"' { in_str = false; }
            continue;
        }
        if ch == '"' { in_str = true; continue; }
        if ch == open { depth += 1; }
        else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(s[..i + ch.len_utf8()].to_string());
            }
        }
    }
    None
}

/// Return the raw `id` JSON value, or `None` if the `id` key is absent.
///
/// `None` → notification (no response needed).
/// `Some("null")` → request with null id.
/// `Some("1")` or `Some("\"abc\"")` → normal request.
pub fn extract_id(json: &str) -> Option<String> {
    if !has_key(json, "id") { return None; }
    extract_raw(json, "id")
}

fn has_key(json: &str, key: &str) -> bool {
    let kp = format!("\"{}\"", key);
    let mut search = json;
    while let Some(pos) = search.find(&kp) {
        let after = &search[pos + kp.len()..];
        let trimmed = after.trim_start_matches(|c: char| c.is_whitespace());
        if trimmed.starts_with(':') {
            return true;
        }
        search = &search[pos + kp.len()..];
    }
    false
}
