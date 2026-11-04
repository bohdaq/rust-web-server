//! Snapshot assertions for generated JSON: [`assert_openapi_snapshot`]
//! compares an app's OpenAPI document with a file committed beside the
//! tests, so a change to the API shows up in review as a change to that
//! file.
//!
//! Both sides are normalized before comparing — parsed, object keys sorted,
//! printed with two-space indentation — so key order and whitespace never
//! count, and a mismatch fails with a line diff of the normalized texts.
//!
//! With `RWS_UPDATE_SNAPSHOTS=1` in the environment (any value but empty,
//! `0` or `false`) the assertion writes the normalized document to the file
//! instead — creating it and its directory if needed — and passes:
//!
//! ```text
//! RWS_UPDATE_SNAPSHOTS=1 cargo test     # accept the current documents
//! git diff tests/snapshots/             # review what changed
//! ```
//!
//! A missing file fails (with that hint) when the variable isn't set, so a
//! CI run never passes by writing its own snapshot. Relative paths are
//! relative to the working directory, which under `cargo test` is the
//! package's root.

use std::path::Path;

use super::{TestClient, TestResponse};
use crate::application::Application;
use crate::service_discovery::json_lite::{self, JsonValue};

/// The environment variable that makes snapshot assertions write instead
/// of compare.
pub const UPDATE_VARIABLE: &str = "RWS_UPDATE_SNAPSHOTS";

/// Something an OpenAPI document can be read from.
pub trait OpenApiSource {
    /// The document's JSON text.
    fn openapi_json(&self) -> String;
}

impl OpenApiSource for str {
    fn openapi_json(&self) -> String {
        self.to_string()
    }
}

impl OpenApiSource for String {
    fn openapi_json(&self) -> String {
        self.clone()
    }
}

impl OpenApiSource for TestResponse {
    /// The response body (e.g. of `GET /openapi.json`).
    fn openapi_json(&self) -> String {
        String::from_utf8_lossy(self.body_bytes()).into_owned()
    }
}

impl<A: Application> OpenApiSource for TestClient<A> {
    /// `GET /openapi.json` through the app — where
    /// `AppWithState::openapi` serves the document.
    fn openapi_json(&self) -> String {
        let response = self.get("/openapi.json").send();
        if response.status() != 200 {
            panic!("GET /openapi.json answered {} — register the document with .openapi(OpenApiConfig::new(…))", response.status());
        }
        response.openapi_json()
    }
}

/// Compare an OpenAPI document with the snapshot at `path` (see the module
/// docs for normalization and `RWS_UPDATE_SNAPSHOTS`). `source` is the
/// document's text (`app.openapi_spec(&config)`), a `TestResponse` of it, or
/// a [`TestClient`] whose app serves `/openapi.json`.
///
/// Panics with a diff on a mismatch, and when the document isn't JSON or
/// has no `openapi` version field (a `404` page is no snapshot).
///
/// ```rust,no_run
/// # extern crate rws_server as rust_web_server;
/// # #[cfg(feature = "openapi")]
/// # fn example() {
/// use rust_web_server::app::App;
/// use rust_web_server::openapi::OpenApiConfig;
/// use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
/// use rust_web_server::test_client::{assert_openapi_snapshot, TestClient};
///
/// let app = App::with_state(())
///     .get("/users/:id", |_req, _params, _conn, _state| {
///         Response::get_response(STATUS_CODE_REASON_PHRASE.n200_ok, None, None)
///     })
///     .openapi(OpenApiConfig::new("Users", "1.0.0"));
/// assert_openapi_snapshot(&TestClient::new(app), "tests/snapshots/openapi.json");
/// # }
/// ```
#[track_caller]
pub fn assert_openapi_snapshot<S: OpenApiSource + ?Sized>(source: &S, path: impl AsRef<Path>) {
    let document = source.openapi_json();
    let value = match json_lite::parse(&document) {
        Ok(value) => value,
        Err(e) => panic!("the OpenAPI document isn't JSON ({e}): {}", excerpt(&document)),
    };
    if value.get("openapi").and_then(JsonValue::as_str).is_none() {
        panic!("not an OpenAPI document (no \"openapi\" version field): {}", excerpt(&document));
    }
    if let Err(message) = check_snapshot(&value, path.as_ref(), update_requested()) {
        panic!("{message}");
    }
}

/// [`assert_openapi_snapshot`] for any JSON document (a JSON Schema, an MCP
/// `tools/list` result, a response body).
#[track_caller]
pub fn assert_json_snapshot(json: &str, path: impl AsRef<Path>) {
    let value = match json_lite::parse(json) {
        Ok(value) => value,
        Err(e) => panic!("not JSON ({e}): {}", excerpt(json)),
    };
    if let Err(message) = check_snapshot(&value, path.as_ref(), update_requested()) {
        panic!("{message}");
    }
}

fn update_requested() -> bool {
    std::env::var(UPDATE_VARIABLE).is_ok_and(|v| !matches!(v.trim(), "" | "0" | "false"))
}

fn excerpt(text: &str) -> String {
    let mut cut: String = text.chars().take(200).collect();
    if cut.len() < text.len() {
        cut.push('…');
    }
    cut
}

/// Compare `value` with the snapshot at `path`, or write it there when
/// `update` is set. `Err` is the failure message.
#[doc(hidden)]
pub fn check_snapshot(value: &JsonValue, path: &Path, update: bool) -> Result<(), String> {
    let actual = normalize(value);
    let existing = std::fs::read_to_string(path);
    if update {
        let unchanged = existing.as_ref().ok().and_then(|text| json_lite::parse(text).ok()).is_some_and(|old| normalize(&old) == actual);
        if !unchanged || existing.as_deref().ok() != Some(actual.as_str()) {
            if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
                std::fs::create_dir_all(dir).map_err(|e| format!("can't create {}: {e}", dir.display()))?;
            }
            std::fs::write(path, &actual).map_err(|e| format!("can't write {}: {e}", path.display()))?;
            if !unchanged {
                eprintln!("updated snapshot {}", path.display());
            }
        }
        return Ok(());
    }
    let text = match existing {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!(
                "no snapshot at {} — run the test with {UPDATE_VARIABLE}=1 to write it, then commit it",
                path.display()
            ))
        }
        Err(e) => return Err(format!("can't read the snapshot {}: {e}", path.display())),
    };
    let expected = match json_lite::parse(&text) {
        Ok(old) => normalize(&old),
        Err(e) => return Err(format!("the snapshot {} isn't JSON ({e}) — rewrite it with {UPDATE_VARIABLE}=1", path.display())),
    };
    if expected == actual {
        return Ok(());
    }
    Err(format!(
        "the document differs from the snapshot {} (- snapshot, + now):\n{}\nIf the change is intended, run the test with {UPDATE_VARIABLE}=1 and commit the file.",
        path.display(),
        line_diff(&expected, &actual)
    ))
}

/// `value` as JSON text: object keys sorted (stable, so duplicates keep
/// their order), two-space indentation, a final newline.
#[doc(hidden)]
pub fn normalize(value: &JsonValue) -> String {
    let mut out = String::new();
    write_value(value, 0, &mut out);
    out.push('\n');
    out
}

/// Recursion is bounded: `json_lite::parse` refuses nesting deeper than
/// `json_lite::MAX_DEPTH`.
fn write_value(value: &JsonValue, indent: usize, out: &mut String) {
    let pad = |out: &mut String, n: usize| out.extend(std::iter::repeat_n(' ', n * 2));
    match value {
        JsonValue::Array(items) if items.is_empty() => out.push_str("[]"),
        JsonValue::Object(pairs) if pairs.is_empty() => out.push_str("{}"),
        JsonValue::Array(items) => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                pad(out, indent + 1);
                write_value(item, indent + 1, out);
                out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
            }
            pad(out, indent);
            out.push(']');
        }
        JsonValue::Object(pairs) => {
            let mut sorted: Vec<&(String, JsonValue)> = pairs.iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            out.push_str("{\n");
            for (i, (key, item)) in sorted.iter().enumerate() {
                pad(out, indent + 1);
                out.push_str(&json_lite::quote(key));
                out.push_str(": ");
                write_value(item, indent + 1, out);
                out.push_str(if i + 1 < sorted.len() { ",\n" } else { "\n" });
            }
            pad(out, indent);
            out.push('}');
        }
        scalar => out.push_str(&json_lite::to_text(scalar)),
    }
}

/// The most lines either side may have for a full diff (an LCS table of
/// `a × b` cells); past it the changed middle is shown as removed + added.
const DIFF_CELLS: usize = 4_000_000;

/// A unified-style line diff of `expected` → `actual`, with three lines of
/// context around each change.
#[doc(hidden)]
pub fn line_diff(expected: &str, actual: &str) -> String {
    let a: Vec<&str> = expected.lines().collect();
    let b: Vec<&str> = actual.lines().collect();
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..].iter().rev().zip(b[prefix..].iter().rev()).take_while(|(x, y)| x == y).count();
    let (mid_a, mid_b) = (&a[prefix..a.len() - suffix], &b[prefix..b.len() - suffix]);

    // (tag, line): ' ' same, '-' only in expected, '+' only in actual.
    let mut ops: Vec<(char, &str)> = a[..prefix].iter().map(|l| (' ', *l)).collect();
    if (mid_a.len() + 1).saturating_mul(mid_b.len() + 1) <= DIFF_CELLS {
        let (n, m) = (mid_a.len(), mid_b.len());
        // lcs[i][j]: the longest common subsequence of mid_a[i..], mid_b[j..].
        let mut lcs = vec![0u32; (n + 1) * (m + 1)];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                lcs[i * (m + 1) + j] = if mid_a[i] == mid_b[j] {
                    lcs[(i + 1) * (m + 1) + j + 1] + 1
                } else {
                    lcs[(i + 1) * (m + 1) + j].max(lcs[i * (m + 1) + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n || j < m {
            if i < n && j < m && mid_a[i] == mid_b[j] {
                ops.push((' ', mid_a[i]));
                i += 1;
                j += 1;
            } else if i < n && (j == m || lcs[(i + 1) * (m + 1) + j] >= lcs[i * (m + 1) + j + 1]) {
                // Removals before additions, as `diff -u` shows them.
                ops.push(('-', mid_a[i]));
                i += 1;
            } else {
                ops.push(('+', mid_b[j]));
                j += 1;
            }
        }
    } else {
        ops.extend(mid_a.iter().map(|l| ('-', *l)));
        ops.extend(mid_b.iter().map(|l| ('+', *l)));
    }
    ops.extend(a[a.len() - suffix..].iter().map(|l| (' ', *l)));

    // Keep changes and three lines of context; "…" marks what's left out.
    const CONTEXT: usize = 3;
    let changed: Vec<usize> = ops.iter().enumerate().filter(|(_, (t, _))| *t != ' ').map(|(i, _)| i).collect();
    if changed.is_empty() {
        return String::new();
    }
    let near = |i: usize| changed.iter().any(|&c| c.abs_diff(i) <= CONTEXT);
    let mut out = String::new();
    let mut skipped = false;
    for (i, (tag, line)) in ops.iter().enumerate() {
        if *tag == ' ' && !near(i) {
            skipped = true;
            continue;
        }
        if skipped {
            out.push_str("  …\n");
            skipped = false;
        }
        out.push(*tag);
        out.push(' ');
        out.push_str(line);
        out.push('\n');
    }
    if skipped {
        out.push_str("  …\n");
    }
    out
}
