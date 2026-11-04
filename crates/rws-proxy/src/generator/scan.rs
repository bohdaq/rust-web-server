//! Reading the project's own files: where `app()` is and what it returns,
//! which `mod` lines a module has, the `rust-web-server` features in
//! `Cargo.toml`, the routes and fields of a generated route module.
//!
//! These read source text, not a syntax tree: one pass, loops only (no
//! recursion, whatever the nesting), and anything they can't place is
//! left to the person — the plan says what to add by hand.

/// What kind of app `app()` builds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppKind {
    /// `AppWithState<S>`: sync handlers.
    Sync,
    /// `AsyncAppWithState<S>` (`http2`): async handlers; sync ones work too.
    Async,
}

/// `pub fn app(…) -> AppWithState<S>` as found in `src/lib.rs`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppFn {
    pub kind: AppKind,
    /// The state type as a path from a submodule (`crate::Store`, `()`).
    pub state: String,
    /// `app()` takes no arguments.
    pub no_params: bool,
    /// Byte range of the function's body, braces included.
    pub body: (usize, usize),
}

/// The byte index just past the bracket matching the one at `open`
/// (`(`/`[`/`{`/`<`), skipping string literals; `None` when unbalanced.
pub fn matching_close(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let (opener, closer) = match bytes.get(open)? {
        b'(' => (b'(', b')'),
        b'[' => (b'[', b']'),
        b'{' => (b'{', b'}'),
        b'<' => (b'<', b'>'),
        _ => return None,
    };
    let mut depth = 0usize;
    let mut i = open;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' {
            // Skip the string literal.
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
        } else if b == opener {
            depth += 1;
        } else if b == closer {
            // `->` inside a `<…>` scan is an arrow, not a closer.
            if !(opener == b'<' && i > 0 && bytes[i - 1] == b'-') {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
        }
        i += 1;
    }
    None
}

/// Find `fn app(` in `lib` and read its signature.
pub fn find_app_fn(lib: &str) -> Option<AppFn> {
    let mut from = 0;
    let start = loop {
        let at = from + lib[from..].find("fn app(")?;
        let boundary = at == 0 || lib.as_bytes()[at - 1].is_ascii_whitespace();
        if boundary {
            break at;
        }
        from = at + 1;
    };
    let open = start + "fn app".len();
    let close = matching_close(lib, open)?;
    let no_params = lib[open + 1..close - 1].trim().is_empty();
    let rest = &lib[close..];
    let arrow = rest.trim_start();
    let arrow_at = close + (rest.len() - arrow.len());
    let ty_text = arrow.strip_prefix("->")?;
    let ty_start = arrow_at + 2;
    let body_open = ty_start + ty_text.find('{')?;
    let ty = lib[ty_start..body_open].trim();
    let ty = ty.split(" where").next().unwrap_or(ty).trim();
    let lt = ty.find('<')?;
    let head = ty[..lt].trim();
    let name = head.rsplit("::").next().unwrap_or(head).trim();
    let kind = match name {
        "AppWithState" => AppKind::Sync,
        "AsyncAppWithState" => AppKind::Async,
        _ => return None,
    };
    let gt = ty.rfind('>')?;
    if gt <= lt {
        return None;
    }
    let state = ty[lt + 1..gt].trim();
    let body_close = matching_close(lib, body_open)?;
    Some(AppFn { kind, state: state_path(state), no_params, body: (body_open, body_close) })
}

/// The state type as seen from a module under `src/`: a bare name defined
/// or imported in `lib.rs` becomes `crate::Name`.
fn state_path(state: &str) -> String {
    let state = state.trim();
    if state == "()" || state.starts_with("crate::") || state.starts_with("::") {
        return state.to_string();
    }
    if let Some(rest) = state.strip_prefix("self::") {
        return format!("crate::{rest}");
    }
    let first = state.split("::").next().unwrap_or(state);
    let simple = state.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && !first.contains('<');
    if simple && !state.contains("::") {
        format!("crate::{state}")
    } else {
        state.to_string()
    }
}

/// The name a line declares as a module (`pub mod x;`, `mod x;`,
/// `pub(crate) mod x;`).
fn declared_mod(line: &str) -> Option<&str> {
    let line = line.trim();
    let rest = ["pub mod ", "pub(crate) mod ", "mod "].iter().find_map(|p| line.strip_prefix(p))?;
    let name = rest.strip_suffix(';')?.trim();
    (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).then_some(name)
}

/// Whether `text` declares `mod name;`.
pub fn has_mod(text: &str, name: &str) -> bool {
    text.lines().any(|line| declared_mod(line) == Some(name))
}

/// `text` with `pub mod name;` added beside the other `mod` lines (in
/// name order where they are sorted), or after the leading `//!` docs
/// and `#![…]` attributes when there are none; `None` when it is already
/// there.
pub fn add_mod_line(text: &str, name: &str) -> Option<String> {
    if has_mod(text, name) {
        return None;
    }
    let new_line = format!("pub mod {name};");
    let lines: Vec<&str> = text.lines().collect();
    let mods: Vec<(usize, &str)> = lines.iter().enumerate().filter_map(|(i, l)| declared_mod(l).map(|n| (i, n))).collect();
    let at = if mods.is_empty() {
        None
    } else {
        // Before the first module that sorts after it, else after the last.
        Some(mods.iter().find(|(_, n)| *n > name).map(|(i, _)| *i).unwrap_or(mods[mods.len() - 1].0 + 1))
    };
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    match at {
        Some(at) => out.insert(at, new_line),
        None => {
            // After the header: `//!` docs, `#![…]`, and the blank lines after them.
            let header = lines.iter().take_while(|l| l.trim_start().starts_with("//!") || l.trim_start().starts_with("#![")).count();
            if header == 0 {
                out.insert(0, new_line);
                if lines.first().is_some_and(|l| !l.trim().is_empty()) {
                    out.insert(1, String::new());
                }
            } else {
                out.insert(header, String::new());
                out.insert(header + 1, new_line);
                if lines.get(header).is_some_and(|l| !l.trim().is_empty()) {
                    out.insert(header + 2, String::new());
                }
            }
        }
    }
    Some(join_lines(out, text))
}

/// `text` with `line` (a `pub use …;`) added after the last `pub use`
/// line, or at the end; `None` when a line equal to it is there.
pub fn add_use_line(text: &str, line: &str) -> Option<String> {
    if text.lines().any(|l| l.trim() == line) {
        return None;
    }
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    match lines.iter().rposition(|l| l.trim_start().starts_with("pub use ")) {
        Some(last) => out.insert(last + 1, line.to_string()),
        None => {
            while out.last().is_some_and(|l| l.trim().is_empty()) {
                out.pop();
            }
            out.push(String::new());
            out.push(line.to_string());
        }
    }
    Some(join_lines(out, text))
}

fn join_lines(lines: Vec<String>, original: &str) -> String {
    let mut text = lines.join("\n");
    if original.is_empty() || original.ends_with('\n') {
        text.push('\n');
    }
    text
}

/// In `app()`'s body, wrap the first `<routes>::<module>::register(…)`
/// call in `<routes>::<name>::register(…)`, so the new routes are added
/// the way the others are. `Ok(None)`: already registered; `Err(())`: no
/// register call to wrap.
pub fn wire_register(lib: &str, app: &AppFn, routes_mod: &str, name: &str) -> Result<Option<String>, ()> {
    let (start, end) = app.body;
    let body = &lib[start..end];
    let ours = format!("{routes_mod}::{name}::register(");
    if body.contains(&ours) {
        return Ok(None);
    }
    let prefix = format!("{routes_mod}::");
    let mut from = 0;
    while let Some(found) = body[from..].find(&prefix) {
        let at = from + found;
        from = at + prefix.len();
        let before_ok = at == 0 || !matches!(body.as_bytes()[at - 1], b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b':');
        let rest = &body[at + prefix.len()..];
        let module_len = rest.bytes().take_while(|b| b.is_ascii_alphanumeric() || *b == b'_').count();
        if !before_ok || module_len == 0 || !rest[module_len..].starts_with("::register(") {
            continue;
        }
        let open = start + at + prefix.len() + module_len + "::register".len();
        let close = matching_close(lib, open).ok_or(())?;
        let call_start = start + at;
        let mut out = String::with_capacity(lib.len() + ours.len() + 1);
        out.push_str(&lib[..call_start]);
        out.push_str(&ours);
        out.push_str(&lib[call_start..close]);
        out.push(')');
        out.push_str(&lib[close..]);
        return Ok(Some(out));
    }
    Err(())
}

/// The `rust-web-server` dependency's features in `Cargo.toml`, when it
/// is written on one line (`rust-web-server = { version = "…", features = […] }`).
pub fn rws_features(cargo: &str) -> Option<Vec<String>> {
    let line = rws_dependency_line(cargo)?;
    let line = cargo.lines().nth(line)?;
    let Some(at) = line.find("features") else { return Some(Vec::new()) };
    let open = at + line[at..].find('[')?;
    let close = matching_close(line, open)?;
    Some(
        line[open + 1..close - 1]
            .split(',')
            .map(|f| f.trim().trim_matches('"').to_string())
            .filter(|f| !f.is_empty())
            .collect(),
    )
}

fn rws_dependency_line(cargo: &str) -> Option<usize> {
    let mut section = "";
    for (i, line) in cargo.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            section = trimmed;
            continue;
        }
        if section != "[dependencies]" {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("rust-web-server") {
            if rest.trim_start().starts_with('=') {
                return Some(i);
            }
        }
    }
    None
}

/// `Cargo.toml` with `feature` added to the `rust-web-server`
/// dependency's features; `Ok(None)` when it is there, `Err(())` when the
/// dependency isn't written in a form this can edit safely.
pub fn add_rws_feature(cargo: &str, feature: &str) -> Result<Option<String>, ()> {
    let index = rws_dependency_line(cargo).ok_or(())?;
    let line = cargo.lines().nth(index).ok_or(())?;
    let quoted = format!("\"{feature}\"");
    let eq = line.find('=').ok_or(())?;
    let value = line[eq + 1..].trim();
    let new_line = if value.starts_with('"') {
        // `rust-web-server = "18.28"`
        let end = 1 + value[1..].find('"').ok_or(())?;
        format!("{} {{ version = {}, features = [{quoted}] }}", line[..eq + 1].trim_end(), &value[..=end])
    } else if value.starts_with('{') {
        let open = line.find('{').ok_or(())?;
        let close = matching_close(line, open).ok_or(())?;
        match line.find("features") {
            Some(at) => {
                let fopen = at + line[at..].find('[').ok_or(())?;
                let fclose = matching_close(line, fopen).ok_or(())?;
                let inner = line[fopen + 1..fclose - 1].trim();
                if inner.split(',').any(|f| f.trim() == quoted) {
                    return Ok(None);
                }
                let list = if inner.is_empty() { quoted } else { format!("{}, {quoted}", inner.trim_end_matches(',').trim_end()) };
                format!("{}{list}{}", &line[..fopen + 1], &line[fclose - 1..])
            }
            None => {
                let inside = line[open + 1..close - 1].trim_end();
                let sep = if inside.trim().is_empty() || inside.ends_with(',') { "" } else { "," };
                format!("{}{inside}{sep} features = [{quoted}] }}{}", &line[..open + 1], &line[close..])
            }
        }
    } else {
        return Err(());
    };
    let mut lines: Vec<String> = cargo.lines().map(str::to_string).collect();
    lines[index] = new_line;
    Ok(Some(join_lines(lines, cargo)))
}

/// `[package] name` of a `Cargo.toml`, as a Rust crate name (`-` → `_`).
pub fn crate_name(cargo: &str) -> Option<String> {
    let mut in_package = false;
    for line in cargo.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("name") {
            let value = rest.trim_start().strip_prefix('=')?.trim();
            let value = value.strip_prefix('"')?;
            let value = &value[..value.find('"')?];
            return Some(value.replace('-', "_"));
        }
    }
    None
}

/// Whether a `[dependencies]` line names `dependency` and contains `needle`
/// (`tokio` with `"macros"`).
pub fn dependency_has(cargo: &str, dependency: &str, needle: &str) -> bool {
    let mut section = "";
    for line in cargo.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            section = trimmed;
            continue;
        }
        if section == "[dependencies]" || section == "[dev-dependencies]" {
            if let Some(rest) = trimmed.strip_prefix(dependency) {
                if rest.trim_start().starts_with('=') && rest.contains(needle) {
                    return true;
                }
            }
        }
    }
    false
}

/// In a test file, the argument of the first `TestClient::new(app(…))`.
pub fn test_client_app_expr(text: &str) -> Option<String> {
    let needle = "TestClient::new(";
    let mut from = 0;
    while let Some(found) = text[from..].find(needle) {
        let open = from + found + needle.len() - 1;
        from = open + 1;
        let Some(close) = matching_close(text, open) else { continue };
        let arg = text[open + 1..close - 1].trim();
        if arg.starts_with("app(") && !arg.contains('\n') && arg.len() <= 200 {
            return Some(arg.to_string());
        }
    }
    None
}

/// A route a module declares with `#[get("…")]` & co.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredRoute {
    pub method: &'static str,
    pub pattern: String,
}

/// Every `#[get("…")]`, `#[post("…")]`, `#[put("…")]`, `#[patch("…")]`,
/// `#[delete("…")]` line of a module.
pub fn declared_routes(text: &str) -> Vec<DeclaredRoute> {
    let mut routes = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        for (attr, method) in [("#[get(\"", "GET"), ("#[post(\"", "POST"), ("#[put(\"", "PUT"), ("#[patch(\"", "PATCH"), ("#[delete(\"", "DELETE")] {
            if let Some(rest) = line.strip_prefix(attr) {
                if let Some(end) = rest.find('"') {
                    routes.push(DeclaredRoute { method, pattern: rest[..end].to_string() });
                }
            }
        }
    }
    routes
}

/// The `pub name: Type,` lines of `pub struct <name> { … }`, as written.
/// `None` when there is no such struct; a field this can't read is
/// returned as `Err(line)`.
pub fn struct_fields(text: &str, name: &str) -> Option<Vec<Result<(String, String), String>>> {
    let header = format!("pub struct {name} {{");
    let mut lines = text.lines().skip_while(|l| l.trim() != header);
    lines.next()?;
    let mut fields = Vec::new();
    for line in lines {
        let line = line.trim();
        if line == "}" {
            return Some(fields);
        }
        if line.is_empty() || line.starts_with("//") || line.starts_with("#[") {
            continue;
        }
        let parsed = line
            .strip_prefix("pub ")
            .and_then(|l| l.strip_suffix(','))
            .and_then(|l| l.split_once(':'))
            .map(|(n, t)| (n.trim().to_string(), t.trim().to_string()));
        fields.push(parsed.ok_or_else(|| line.to_string()));
    }
    None
}

/// The number of a migration file name (`0007_add_x.sql` → 7), and the
/// width it is written in.
pub fn migration_number(file: &str) -> Option<(u64, usize)> {
    let digits = file.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits > 18 || file.as_bytes().get(digits) != Some(&b'_') || !file.ends_with(".sql") {
        return None;
    }
    file[..digits].parse().ok().map(|n| (n, digits))
}
