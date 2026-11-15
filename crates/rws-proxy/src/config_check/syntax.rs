//! TOML syntax for `rws --check` (18.37.0): the server's reader is
//! line-based and forgiving (an unclosed `[[route]` header was skipped, and
//! `--check` said "configuration OK"), so the check reads the file as TOML
//! first and reports the first mistakes with line and column.
//!
//! Structural mistakes are errors: a table header that isn't closed, a line
//! that is neither a header nor `key = value`, a string, array or inline
//! table that isn't closed, an empty value. A bare word where TOML wants a
//! quoted string (`ip = 0.0.0.0`) is a warning: rws reads it anyway.

/// One finding: 1-based line and column, and the message.
#[derive(Debug, PartialEq)]
pub struct Finding {
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub error: bool,
}

/// Check `content` as TOML; at most 20 findings.
pub fn check(content: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() && out.len() < 20 {
        let number = i + 1;
        let raw = lines[i];
        let indent = raw.len() - raw.trim_start().len();
        let line = raw.trim();
        i += 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let at = |column: usize, message: String, error: bool| Finding { line: number, column, message, error };
        if line.starts_with('[') {
            let code = strip_comment(line).trim_end();
            let double = code.starts_with("[[");
            let close = if double { "]]" } else { "]" };
            if !code.ends_with(close) || (double && code.len() < 4) {
                out.push(at(indent + code.len() + 1, format!("table header {code} is not closed: expected `{close}`"), true));
                continue;
            }
            let name = code.trim_start_matches('[').trim_end_matches(']').trim();
            if !double && code.ends_with("]]") {
                out.push(at(indent + 1, format!("table header {code} opens with `[` but closes with `]]`"), true));
            } else if name.is_empty() || !valid_key_path(name) {
                out.push(at(indent + 2, format!("table header {code}: `{name}` is not a valid table name"), true));
            }
            continue;
        }
        let Some(eq) = find_outside_quotes(line, '=') else {
            out.push(at(indent + 1, format!("expected `key = value` or a [table] header, found `{}`", shorten(line)), true));
            continue;
        };
        let key = line[..eq].trim();
        if key.is_empty() || !valid_key_path(key) {
            out.push(at(indent + 1, format!("`{}` is not a valid key", shorten(key)), true));
            continue;
        }
        let value_start = indent + eq + 1;
        let mut value = strip_comment(line[eq + 1..].trim()).trim().to_string();
        let value_column = value_start + (line[eq + 1..].len() - line[eq + 1..].trim_start().len()) + 1;
        if value.is_empty() {
            out.push(at(value_column, format!("`{key} =` has no value"), true));
            continue;
        }
        // Multi-line strings and arrays/inline tables over several lines.
        for delim in ["\"\"\"", "'''"] {
            if value.starts_with(delim) && (value.len() < 6 || !value[3..].contains(delim)) {
                let mut closed = false;
                while i < lines.len() {
                    let next = lines[i];
                    i += 1;
                    if next.contains(delim) {
                        closed = true;
                        break;
                    }
                }
                if !closed {
                    out.push(at(value_column, format!("the multi-line string for `{key}` is not closed: expected {delim}"), true));
                }
                value.clear();
                break;
            }
        }
        if value.is_empty() {
            continue;
        }
        if value.starts_with('[') || value.starts_with('{') {
            let mut depth = balance(&value);
            while depth.0 > 0 && depth.1.is_none() && i < lines.len() {
                let next = strip_comment(lines[i].trim()).to_string();
                i += 1;
                value.push(' ');
                value.push_str(&next);
                depth = balance(&value);
            }
            if let Some(problem) = depth.1 {
                out.push(at(value_column, format!("`{key}`: {problem}"), true));
            } else if depth.0 > 0 {
                let close = if value.starts_with('[') { "]" } else { "}" };
                out.push(at(value_column, format!("`{key}`: not closed: expected `{close}`"), true));
            }
            continue;
        }
        if value.starts_with('"') || value.starts_with('\'') {
            match closing_quote(&value) {
                None => out.push(at(value_column, format!("the string for `{key}` is not closed: expected {}", &value[..1]), true)),
                Some(end) if !value[end + 1..].trim().is_empty() => out.push(at(
                    value_column + end + 1,
                    format!("unexpected `{}` after the string for `{key}`", shorten(value[end + 1..].trim())),
                    true,
                )),
                _ => {}
            }
            continue;
        }
        if !scalar(&value) {
            out.push(at(value_column, format!("`{key} = {}`: a string needs quotes in TOML: {key} = \"{}\"", shorten(&value), value.replace('"', "\\\"")), false));
        }
    }
    out
}

fn shorten(s: &str) -> String {
    if s.chars().count() > 40 { format!("{}…", s.chars().take(40).collect::<String>()) } else { s.to_string() }
}

/// Bare (`a-b_c`), quoted (`"a b"`) and dotted (`a.b`) keys.
fn valid_key_path(path: &str) -> bool {
    let mut rest = path.trim();
    loop {
        let (part, tail) = if rest.starts_with('"') || rest.starts_with('\'') {
            let Some(end) = closing_quote(rest) else { return false };
            (&rest[..=end], rest[end + 1..].trim_start())
        } else {
            let end = rest.find('.').unwrap_or(rest.len());
            let part = rest[..end].trim();
            if part.is_empty() || !part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                return false;
            }
            (part, &rest[end..])
        };
        let _ = part;
        if tail.is_empty() {
            return true;
        }
        let Some(after) = tail.strip_prefix('.') else { return false };
        rest = after.trim();
        if rest.is_empty() {
            return false;
        }
    }
}

/// Byte index of the quote closing the string `s` starts with.
fn closing_quote(s: &str) -> Option<usize> {
    let quote = s.chars().next()?;
    let mut escaped = false;
    for (i, c) in s.char_indices().skip(1) {
        if quote == '"' && escaped {
            escaped = false;
            continue;
        }
        if quote == '"' && c == '\\' {
            escaped = true;
        } else if c == quote {
            return Some(i);
        }
    }
    None
}

fn find_outside_quotes(s: &str, wanted: char) -> Option<usize> {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        match quote {
            Some('"') if escaped => escaped = false,
            Some('"') if c == '\\' => escaped = true,
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '#' => return None,
            None if c == wanted => return Some(i),
            None => {}
        }
    }
    None
}

/// Drop a trailing `# comment` outside quotes.
fn strip_comment(s: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        match quote {
            Some('"') if escaped => escaped = false,
            Some('"') if c == '\\' => escaped = true,
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '#' => return &s[..i],
            None => {}
        }
    }
    s
}

/// Open bracket depth of an array or inline table, and the first mismatch.
fn balance(s: &str) -> (usize, Option<String>) {
    let mut stack: Vec<char> = Vec::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for c in s.chars() {
        match quote {
            Some('"') if escaped => escaped = false,
            Some('"') if c == '\\' => escaped = true,
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None => match c {
                '"' | '\'' => quote = Some(c),
                '[' | '{' => stack.push(c),
                ']' | '}' => {
                    let open = if c == ']' { '[' } else { '{' };
                    if stack.pop() != Some(open) {
                        return (0, Some(format!("unexpected `{c}`")));
                    }
                }
                _ => {}
            },
        }
    }
    if quote.is_some() {
        return (stack.len().max(1), Some("a string inside is not closed".to_string()));
    }
    (stack.len(), None)
}

/// A TOML number, boolean or date/time.
fn scalar(v: &str) -> bool {
    if v == "true" || v == "false" || v == "inf" || v == "nan" || v == "+inf" || v == "-inf" {
        return true;
    }
    let digits = v.trim_start_matches(['+', '-']);
    if let Some(hex) = digits.strip_prefix("0x").or_else(|| digits.strip_prefix("0o")).or_else(|| digits.strip_prefix("0b")) {
        return !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit() || c == '_');
    }
    if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit() || matches!(c, '_' | '.' | 'e' | 'E' | '+' | '-')) && digits.starts_with(|c: char| c.is_ascii_digit()) {
        return true;
    }
    // Dates and times: 1979-05-27, 07:32:00, 1979-05-27T07:32:00Z.
    v.starts_with(|c: char| c.is_ascii_digit())
        && v.chars().all(|c| c.is_ascii_digit() || matches!(c, '-' | ':' | 'T' | 't' | 'Z' | 'z' | '.' | '+' | ' '))
        && (v.contains('-') || v.contains(':'))
}

#[cfg(test)]
mod tests {
    use super::check;

    fn errors(s: &str) -> Vec<String> {
        check(s).into_iter().filter(|f| f.error).map(|f| format!("{}:{} {}", f.line, f.column, f.message)).collect()
    }

    #[test]
    fn valid_toml_has_no_findings() {
        let ok = "# c\nport = 8080\nip = \"0.0.0.0\" # x\n[cors]\nallow_all = true\n\n[[route]]\npath = \"/api/*\"\nupstream = \"api\"\nheaders = { set = { \"x-a\" = \"b\" } }\nlist = [\n  \"a\",\n  \"b\", # c\n]\nwhen = 1979-05-27T07:32:00Z\ns = '''\nmulti\n'''\n[admin.\"quoted key\"]\nx = 1_000\n";
        assert_eq!(check(ok), vec![]);
    }

    #[test]
    fn unclosed_header_is_an_error_with_position() {
        assert_eq!(errors("port = 1\n[[route]\npath = \"/\"\n"), vec!["2:9 table header [[route] is not closed: expected `]]`"]);
        assert_eq!(errors("[admin\n").len(), 1);
    }

    #[test]
    fn structural_mistakes_are_errors() {
        assert!(errors("just words\n")[0].contains("expected `key = value`"));
        assert!(errors("a = \"open\n")[0].contains("not closed"));
        assert!(errors("a = [1, 2\n")[0].contains("not closed"));
        assert!(errors("a = { b = 1 ]\n")[0].contains("unexpected `]`"));
        assert!(errors("a =\n")[0].contains("no value"));
        assert!(errors("a = \"x\" y\n")[0].contains("after the string"));
    }

    #[test]
    fn bare_words_are_warnings() {
        let f = check("ip = 0.0.0.0\nname = hello\n");
        assert_eq!(f.len(), 1);
        assert!(!f[0].error && f[0].message.contains("needs quotes"));
    }
}
