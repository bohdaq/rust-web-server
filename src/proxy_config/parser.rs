//! Hand-rolled TOML parser that extracts `[[upstream]]`, `[[route]]`,
//! `[[tcp_proxy]]`, `[[udp_proxy]]`, `[[ws_proxy]]` sections from
//! `rws.config.toml` into a flat `SectionMap`.
//!
//! The output is a `HashMap<String, Vec<(String, String)>>` where each key is
//! a section path (e.g. `"route[0].action.proxy"`) and each value is the list
//! of key-value pairs found in that section.

use std::collections::HashMap;

/// Section path → list of (key, value) pairs.
pub type SectionMap = HashMap<String, Vec<(String, String)>>;

/// Parse a TOML string into a `SectionMap`.
///
/// Supports the subset of TOML used in `rws.config.toml`:
/// - Array-of-tables `[[X]]`
/// - Standard tables `[X]`
/// - Key-value pairs (string, bool, integer, inline tables, arrays)
pub fn parse(toml: &str) -> SectionMap {
    let mut map: SectionMap = HashMap::new();

    // Outer array table tracking
    let mut outer_name: Option<String> = None; // e.g. "route"
    let mut outer_idx: usize = 0;
    let mut outer_counters: HashMap<String, usize> = HashMap::new(); // "route" → 2
    let mut inner_counters: HashMap<String, usize> = HashMap::new(); // "route[0].middleware.rewrite.request" → 1

    // Current section path (where key-value pairs are written)
    let mut current_section: String = String::new();

    for raw_line in toml.lines() {
        // Strip inline comments (naive: first # not inside a string)
        let line = strip_comment(raw_line).trim().to_string();

        if line.is_empty() {
            continue;
        }

        if line.starts_with("[[") && line.ends_with("]]") {
            // Array-of-tables header
            let name = line[2..line.len() - 2].trim().to_string();

            // Is this a nested array inside the current outer table?
            if let Some(ref on) = outer_name.clone() {
                let prefix = format!("{}.", on);
                if name.starts_with(&prefix) {
                    // Nested array: "route[N].middleware.rewrite.request"
                    let base = format!("{}{}", outer_section_base(on, outer_idx), &name[on.len()..]);
                    let cnt = inner_counters.entry(base.clone()).or_insert(0);
                    let section_path = format!("{}[{}]", base, cnt);
                    *cnt += 1;
                    current_section = section_path.clone();
                    map.entry(current_section.clone()).or_default();
                    continue;
                }
            }

            // Top-level array-of-tables
            let idx = outer_counters.entry(name.clone()).or_insert(0);
            outer_idx = *idx;
            *idx += 1;
            outer_name = Some(name.clone());
            inner_counters.clear();
            current_section = outer_section_base(&name, outer_idx);
            map.entry(current_section.clone()).or_default();
        } else if line.starts_with('[') && line.ends_with(']') {
            // Standard table header
            let name = line[1..line.len() - 1].trim().to_string();

            if let Some(ref on) = outer_name.clone() {
                let prefix = format!("{}.", on);
                if name.starts_with(&prefix) {
                    // Sub-table of current outer: e.g. "route.match" inside "route"
                    current_section = format!(
                        "{}{}",
                        outer_section_base(on, outer_idx),
                        &name[on.len()..]
                    );
                    map.entry(current_section.clone()).or_default();
                    continue;
                }
            }

            // Standalone table (reset outer tracking)
            outer_name = None;
            inner_counters.clear();
            current_section = name.clone();
            map.entry(current_section.clone()).or_default();
        } else if let Some(eq) = line.find('=') {
            // Key = value pair
            let key = line[..eq].trim().to_string();
            let raw_val = line[eq + 1..].trim().to_string();

            if raw_val.starts_with('{') {
                // Inline table: expand into sub-keys
                let inner = &raw_val[1..raw_val.rfind('}').unwrap_or(raw_val.len())];
                for part in split_inline_table(inner) {
                    if let Some(ieq) = part.find('=') {
                        let ik = part[..ieq].trim();
                        let iv = parse_value(part[ieq + 1..].trim());
                        let subkey = format!("{}.{}", key, ik);
                        map.entry(current_section.clone())
                            .or_default()
                            .push((subkey, iv));
                    }
                }
            } else {
                let value = parse_value(&raw_val);
                map.entry(current_section.clone())
                    .or_default()
                    .push((key, value));
            }
        }
    }

    map
}

/// Build the base path for an outer array entry, e.g. `"route[0]"`.
fn outer_section_base(name: &str, idx: usize) -> String {
    format!("{}[{}]", name, idx)
}

/// Strip an inline TOML comment (first `#` not inside a quoted string).
pub(crate) fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_quote = false;
    let mut quote_char = b'"';
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if in_quote {
            if b == quote_char && (i == 0 || bytes[i - 1] != b'\\') {
                in_quote = false;
            }
        } else if b == b'"' || b == b'\'' {
            in_quote = true;
            quote_char = b;
        } else if b == b'#' {
            return &line[..i];
        }
        i += 1;
    }
    line
}

/// Parse a single TOML value into its string representation.
///
/// - Quoted strings → bare string (quotes stripped)
/// - Arrays `["a","b"]` → `"a,b"` (joined with comma)
/// - Booleans / numbers → as-is
pub(crate) fn parse_value(raw: &str) -> String {
    let raw = raw.trim();
    if raw.starts_with('[') && raw.ends_with(']') {
        // Array of scalars
        let inner = &raw[1..raw.len() - 1];
        let items: Vec<String> = split_array_items(inner)
            .into_iter()
            .map(|s| strip_quotes(s.trim()))
            .collect();
        return items.join(",");
    }
    strip_quotes(raw)
}

/// Strip leading/trailing `"` or `'` from a value.
fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) ||
       (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Split a TOML array body (without brackets) into individual items.
/// Handles quoted strings that may contain commas.
fn split_array_items(inner: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut depth = 0i32;
    let mut in_quote = false;
    let mut quote_char = b'"';
    let mut start = 0;
    let bytes = inner.as_bytes();

    for (i, &b) in bytes.iter().enumerate() {
        if in_quote {
            if b == quote_char {
                in_quote = false;
            }
        } else if b == b'"' || b == b'\'' {
            in_quote = true;
            quote_char = b;
        } else if b == b'[' || b == b'{' {
            depth += 1;
        } else if b == b']' || b == b'}' {
            depth -= 1;
        } else if b == b',' && depth == 0 {
            items.push(inner[start..i].trim());
            start = i + 1;
        }
    }
    let last = inner[start..].trim();
    if !last.is_empty() {
        items.push(last);
    }
    items
}

/// Split inline table body `"key = val, key2 = val2"` into parts at top-level commas.
fn split_inline_table(inner: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut in_quote = false;
    let mut quote_char = b'"';
    let mut current = String::new();

    for b in inner.bytes() {
        if in_quote {
            current.push(b as char);
            if b == quote_char {
                in_quote = false;
            }
        } else if b == b'"' || b == b'\'' {
            in_quote = true;
            quote_char = b;
            current.push(b as char);
        } else if b == b'[' || b == b'{' {
            depth += 1;
            current.push(b as char);
        } else if b == b']' || b == b'}' {
            depth -= 1;
            current.push(b as char);
        } else if b == b',' && depth == 0 {
            let part = current.trim().to_string();
            if !part.is_empty() {
                parts.push(part);
            }
            current = String::new();
        } else {
            current.push(b as char);
        }
    }
    let part = current.trim().to_string();
    if !part.is_empty() {
        parts.push(part);
    }
    parts
}

// ── Helper accessors ───────────────────────────────────────────────────────────

/// Get the first value for `key` in `section`, or `None`.
pub(crate) fn get(map: &SectionMap, section: &str, key: &str) -> Option<String> {
    map.get(section)?.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
}

/// Get the first value for `key` in `section`, or empty string.
pub(crate) fn get_str(map: &SectionMap, section: &str, key: &str) -> String {
    get(map, section, key).unwrap_or_default()
}

/// Get the first value for `key` in `section` as `u64`, or `default`.
pub(crate) fn get_u64(map: &SectionMap, section: &str, key: &str, default: u64) -> u64 {
    get(map, section, key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Get the first value for `key` in `section` as `u32`, or `default`.
pub(crate) fn get_u32(map: &SectionMap, section: &str, key: &str, default: u32) -> u32 {
    get(map, section, key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Get the first value for `key` in `section` as a list (split on `,`).
pub(crate) fn get_array(map: &SectionMap, section: &str, key: &str) -> Vec<String> {
    match get(map, section, key) {
        Some(v) if !v.is_empty() => v.split(',').map(|s| s.trim().to_string()).collect(),
        _ => vec![],
    }
}

/// Returns `true` if the section key exists in the map.
pub(crate) fn section_exists(map: &SectionMap, section: &str) -> bool {
    map.contains_key(section)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[server]
ip = "0.0.0.0"
port = 8080

[[upstream]]
name = "api"
backends = ["backend1:8080", "backend2:8080"]
strategy = "round_robin"

[upstream.health_check]
path = "/health"
interval_secs = 10
timeout_ms = 2000
healthy_threshold = 2
unhealthy_threshold = 3

[[route]]
name = "api-route"

[route.match]
path = "/api/*"
method = "GET"

[route.action]
type = "proxy"

[route.action.proxy]
upstream = "api"
connect_timeout_ms = 5000
read_timeout_ms = 30000

[route.middleware]

[route.middleware.rate_limit]
max_requests = 100
window_secs = 60

[[route.middleware.rewrite.request]]
type = "header_set"
name = "X-Env"
value = "production"

[[tcp_proxy]]
name = "db-proxy"
listen = "0.0.0.0:5432"
backends = ["db1:5432", "db2:5432"]
connect_timeout_ms = 3000
"#;

    #[test]
    fn parse_server_section() {
        let map = parse(SAMPLE);
        assert_eq!(get(&map, "server", "ip").as_deref(), Some("0.0.0.0"));
        assert_eq!(get_u64(&map, "server", "port", 0), 8080);
    }

    #[test]
    fn parse_upstream() {
        let map = parse(SAMPLE);
        assert!(section_exists(&map, "upstream[0]"));
        assert_eq!(get_str(&map, "upstream[0]", "name"), "api");
        assert_eq!(
            get_array(&map, "upstream[0]", "backends"),
            vec!["backend1:8080", "backend2:8080"]
        );
    }

    #[test]
    fn parse_upstream_health_check() {
        let map = parse(SAMPLE);
        assert!(section_exists(&map, "upstream[0].health_check"));
        assert_eq!(get_str(&map, "upstream[0].health_check", "path"), "/health");
        assert_eq!(get_u64(&map, "upstream[0].health_check", "interval_secs", 0), 10);
    }

    #[test]
    fn parse_route() {
        let map = parse(SAMPLE);
        assert!(section_exists(&map, "route[0]"));
        assert_eq!(get_str(&map, "route[0]", "name"), "api-route");
        assert_eq!(get_str(&map, "route[0].match", "path"), "/api/*");
        assert_eq!(get_str(&map, "route[0].match", "method"), "GET");
        assert_eq!(get_str(&map, "route[0].action.proxy", "upstream"), "api");
        assert_eq!(get_u64(&map, "route[0].action.proxy", "connect_timeout_ms", 0), 5000);
    }

    #[test]
    fn parse_route_middleware() {
        let map = parse(SAMPLE);
        assert_eq!(get_u32(&map, "route[0].middleware.rate_limit", "max_requests", 0), 100);
        assert_eq!(get_u64(&map, "route[0].middleware.rate_limit", "window_secs", 0), 60);
    }

    #[test]
    fn parse_nested_rewrite_array() {
        let map = parse(SAMPLE);
        assert!(section_exists(&map, "route[0].middleware.rewrite.request[0]"));
        assert_eq!(get_str(&map, "route[0].middleware.rewrite.request[0]", "type"), "header_set");
        assert_eq!(get_str(&map, "route[0].middleware.rewrite.request[0]", "name"), "X-Env");
    }

    #[test]
    fn parse_tcp_proxy() {
        let map = parse(SAMPLE);
        assert!(section_exists(&map, "tcp_proxy[0]"));
        assert_eq!(get_str(&map, "tcp_proxy[0]", "name"), "db-proxy");
        assert_eq!(get_str(&map, "tcp_proxy[0]", "listen"), "0.0.0.0:5432");
        assert_eq!(
            get_array(&map, "tcp_proxy[0]", "backends"),
            vec!["db1:5432", "db2:5432"]
        );
    }

    #[test]
    fn strip_comments_test() {
        assert_eq!(strip_comment("key = \"value\" # a comment"), "key = \"value\" ");
        assert_eq!(strip_comment("# full comment"), "");
        assert_eq!(strip_comment("url = \"http://example.com#fragment\""), "url = \"http://example.com#fragment\"");
    }

    #[test]
    fn parse_value_array() {
        assert_eq!(parse_value(r#"["a", "b", "c"]"#), "a,b,c");
    }

    #[test]
    fn parse_value_string() {
        assert_eq!(parse_value(r#""hello""#), "hello");
    }
}
