//! Minimal recursive-descent JSON value parser — just enough to read Consul,
//! Docker, and etcd HTTP API responses (nested objects/arrays, strings,
//! numbers, bools, null). No `serde` dependency; `service_discovery` is
//! always compiled (no feature gate), so it can't depend on the optional
//! `serde` feature. Mirrors this crate's existing per-module "own tiny
//! encoders" pattern (e.g. `mcp::json_rpc`, `sso::saml::xml`) rather than
//! introducing a crate-wide JSON value type.

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    /// Looks up `key` in an object; `None` for any other variant or a
    /// missing key.
    pub(crate) fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            JsonValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub(crate) fn as_f64(&self) -> Option<f64> {
        match self {
            JsonValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub(crate) fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            JsonValue::Array(items) => Some(items),
            _ => None,
        }
    }
}

/// Parses one complete JSON value from `input`, ignoring (erroring on)
/// trailing content beyond it other than whitespace.
/// Nesting deeper than this is refused: parsing (and dropping) recurses
/// per level, and the input may come from anywhere.
pub(crate) const MAX_DEPTH: usize = 128;

pub(crate) fn parse(input: &str) -> Result<JsonValue, String> {
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0usize;
    skip_whitespace(&chars, &mut pos);
    let value = parse_value(&chars, &mut pos, 0)?;
    skip_whitespace(&chars, &mut pos);
    Ok(value)
}

fn skip_whitespace(chars: &[char], pos: &mut usize) {
    while *pos < chars.len() && chars[*pos].is_whitespace() {
        *pos += 1;
    }
}

fn parse_value(chars: &[char], pos: &mut usize, depth: usize) -> Result<JsonValue, String> {
    skip_whitespace(chars, pos);
    if depth >= MAX_DEPTH && matches!(chars.get(*pos), Some('{') | Some('[')) {
        return Err(format!("nested deeper than {MAX_DEPTH} levels at position {pos}"));
    }
    match chars.get(*pos) {
        Some('{') => parse_object(chars, pos, depth + 1),
        Some('[') => parse_array(chars, pos, depth + 1),
        Some('"') => parse_string(chars, pos).map(JsonValue::String),
        Some('t') => parse_literal(chars, pos, "true", JsonValue::Bool(true)),
        Some('f') => parse_literal(chars, pos, "false", JsonValue::Bool(false)),
        Some('n') => parse_literal(chars, pos, "null", JsonValue::Null),
        Some(c) if *c == '-' || c.is_ascii_digit() => parse_number(chars, pos),
        Some(c) => Err(format!("unexpected character '{}' at position {}", c, pos)),
        None => Err("unexpected end of input".to_string()),
    }
}

fn parse_literal(chars: &[char], pos: &mut usize, literal: &str, value: JsonValue) -> Result<JsonValue, String> {
    let literal_chars: Vec<char> = literal.chars().collect();
    if chars.len() < *pos + literal_chars.len() || chars[*pos..*pos + literal_chars.len()] != literal_chars[..] {
        return Err(format!("expected '{}' at position {}", literal, pos));
    }
    *pos += literal_chars.len();
    Ok(value)
}

fn parse_object(chars: &[char], pos: &mut usize, depth: usize) -> Result<JsonValue, String> {
    *pos += 1; // consume '{'
    let mut pairs = Vec::new();
    skip_whitespace(chars, pos);
    if chars.get(*pos) == Some(&'}') {
        *pos += 1;
        return Ok(JsonValue::Object(pairs));
    }
    loop {
        skip_whitespace(chars, pos);
        if chars.get(*pos) != Some(&'"') {
            return Err(format!("expected string key at position {}", pos));
        }
        let key = parse_string(chars, pos)?;
        skip_whitespace(chars, pos);
        if chars.get(*pos) != Some(&':') {
            return Err(format!("expected ':' at position {}", pos));
        }
        *pos += 1;
        let value = parse_value(chars, pos, depth)?;
        pairs.push((key, value));
        skip_whitespace(chars, pos);
        match chars.get(*pos) {
            Some(',') => { *pos += 1; }
            Some('}') => { *pos += 1; break; }
            _ => return Err(format!("expected ',' or '}}' at position {}", pos)),
        }
    }
    Ok(JsonValue::Object(pairs))
}

fn parse_array(chars: &[char], pos: &mut usize, depth: usize) -> Result<JsonValue, String> {
    *pos += 1; // consume '['
    let mut items = Vec::new();
    skip_whitespace(chars, pos);
    if chars.get(*pos) == Some(&']') {
        *pos += 1;
        return Ok(JsonValue::Array(items));
    }
    loop {
        let value = parse_value(chars, pos, depth)?;
        items.push(value);
        skip_whitespace(chars, pos);
        match chars.get(*pos) {
            Some(',') => { *pos += 1; }
            Some(']') => { *pos += 1; break; }
            _ => return Err(format!("expected ',' or ']' at position {}", pos)),
        }
    }
    Ok(JsonValue::Array(items))
}

fn parse_string(chars: &[char], pos: &mut usize) -> Result<String, String> {
    *pos += 1; // consume opening '"'
    let mut out = String::new();
    loop {
        match chars.get(*pos) {
            Some('"') => { *pos += 1; break; }
            Some('\\') => {
                *pos += 1;
                match chars.get(*pos) {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('/') => out.push('/'),
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('r') => out.push('\r'),
                    Some('b') => out.push('\u{8}'),
                    Some('f') => out.push('\u{c}'),
                    Some('u') => {
                        let hex: String = chars.get(*pos + 1..*pos + 5)
                            .ok_or("truncated \\u escape")?
                            .iter().collect();
                        let code = u32::from_str_radix(&hex, 16).map_err(|_| "invalid \\u escape")?;
                        out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                        *pos += 4;
                    }
                    _ => return Err(format!("invalid escape at position {}", pos)),
                }
                *pos += 1;
            }
            Some(c) => { out.push(*c); *pos += 1; }
            None => return Err("unterminated string".to_string()),
        }
    }
    Ok(out)
}

fn parse_number(chars: &[char], pos: &mut usize) -> Result<JsonValue, String> {
    let start = *pos;
    if chars.get(*pos) == Some(&'-') { *pos += 1; }
    while chars.get(*pos).is_some_and(|c| c.is_ascii_digit()) { *pos += 1; }
    if chars.get(*pos) == Some(&'.') {
        *pos += 1;
        while chars.get(*pos).is_some_and(|c| c.is_ascii_digit()) { *pos += 1; }
    }
    if matches!(chars.get(*pos), Some('e') | Some('E')) {
        *pos += 1;
        if matches!(chars.get(*pos), Some('+') | Some('-')) { *pos += 1; }
        while chars.get(*pos).is_some_and(|c| c.is_ascii_digit()) { *pos += 1; }
    }
    let s: String = chars[start..*pos].iter().collect();
    s.parse::<f64>().map(JsonValue::Number).map_err(|_| format!("invalid number '{}'", s))
}

// ── JSON out (shared by A2A and MCP's OpenAPI tools) ───────────────────────

pub(crate) fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A JSON value back to JSON text.
pub(crate) fn to_text(value: &JsonValue) -> String {
    match value {
        JsonValue::Null => "null".to_string(),
        JsonValue::Bool(b) => b.to_string(),
        JsonValue::Number(n) if n.fract() == 0.0 && n.abs() < 9.0e15 => format!("{}", *n as i64),
        JsonValue::Number(n) => format!("{n}"),
        JsonValue::String(s) => quote(s),
        JsonValue::Array(items) => format!("[{}]", items.iter().map(to_text).collect::<Vec<_>>().join(",")),
        JsonValue::Object(pairs) => format!("{{{}}}", pairs.iter().map(|(k, v)| format!("{}:{}", quote(k), to_text(v))).collect::<Vec<_>>().join(",")),
    }
}
