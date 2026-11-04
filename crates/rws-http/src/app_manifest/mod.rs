//! `rws.app.toml`: the app manifest, and the conventional layout of an rws
//! application (18.28.0, roadmap #87).
//!
//! One layout an agent (or a person) can rely on without reading the
//! project first:
//!
//! | Role | Path | What goes there |
//! |---|---|---|
//! | `main` | `src/main.rs` | wiring only: settings, `dev_mcp`, `Server::run_tls` |
//! | `lib` | `src/lib.rs` | `app()` — the routes and middleware, built in a function tests call |
//! | `routes` | `src/routes/` | typed handlers (`#[get]` …), one module per resource |
//! | `models` | `src/models/` | `#[derive(Model)]` types |
//! | `jobs` | `src/jobs/` | background jobs and scheduled tasks |
//! | `migrations` | `migrations/` | `NNNN_name.sql` with an optional `NNNN_name.down.sql` |
//! | `templates` | `templates/` | Tera templates |
//! | `tests` | `tests/` | integration tests against `app()` (`TestClient`) |
//! | `static` | `static/` | files served as they are |
//!
//! The manifest at the project root names the app and says where a project
//! differs from that layout; every key is optional, so an empty or absent
//! file means "conventional":
//!
//! ```toml
//! # rws.app.toml
//! [app]
//! name = "todos"
//!
//! [paths]
//! static = "frontend/dist"   # only what differs from the layout
//! ```
//!
//! ```rust
//! # extern crate rws_http as rust_web_server;
//! use rust_web_server::app_manifest::AppManifest;
//!
//! let manifest: AppManifest = "[paths]\nstatic = \"public\"".parse().unwrap();
//! assert_eq!(manifest.paths.static_files, "public");
//! assert_eq!(manifest.paths.routes, "src/routes"); // the convention
//! ```
//!
//! The parser reads one line at a time (no recursion), takes at most
//! [`MAX_MANIFEST_SIZE`] bytes, and answers bad input with a
//! [`ManifestError`] naming the line — it never panics. Keys it doesn't know
//! are kept as [`AppManifest::warnings`], not errors, so a manifest written
//! for a newer release still loads.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::entry_point::config_file::toml_str::{unquote, Quotes};

#[cfg(test)]
mod tests;

/// The manifest's file name, at the project root.
pub const FILE_NAME: &str = "rws.app.toml";

/// The largest manifest [`AppManifest::from_str`] and [`AppManifest::load`]
/// accept (64 KiB); a real one is a few hundred bytes.
pub const MAX_MANIFEST_SIZE: usize = 64 * 1024;

/// The largest `Cargo.toml` [`AppManifest::load`] reads the package name
/// from (1 MiB).
const MAX_CARGO_TOML_SIZE: u64 = 1024 * 1024;

/// Where each part of the app lives, relative to the project root. The
/// [`Default`] is the conventional layout (see the [module docs](self)).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppPaths {
    /// The binary's entry point, wiring only (`src/main.rs`).
    pub main: String,
    /// The library that builds the app, `app()` (`src/lib.rs`).
    pub lib: String,
    /// Typed handlers, one module per resource (`src/routes`).
    pub routes: String,
    /// `#[derive(Model)]` types (`src/models`).
    pub models: String,
    /// Background jobs (`src/jobs`).
    pub jobs: String,
    /// SQL migrations, `NNNN_name.sql` and `NNNN_name.down.sql` (`migrations`).
    pub migrations: String,
    /// Templates (`templates`).
    pub templates: String,
    /// Integration tests (`tests`).
    pub tests: String,
    /// Static files (`static`; the key is `static` in the manifest).
    pub static_files: String,
}

impl Default for AppPaths {
    fn default() -> Self {
        AppPaths {
            main: "src/main.rs".to_string(),
            lib: "src/lib.rs".to_string(),
            routes: "src/routes".to_string(),
            models: "src/models".to_string(),
            jobs: "src/jobs".to_string(),
            migrations: "migrations".to_string(),
            templates: "templates".to_string(),
            tests: "tests".to_string(),
            static_files: "static".to_string(),
        }
    }
}

/// The roles in [`AppPaths`], as the manifest's `[paths]` keys name them,
/// in layout order.
pub const ROLES: [&str; 9] = ["main", "lib", "routes", "models", "jobs", "migrations", "templates", "tests", "static"];

impl AppPaths {
    /// `(role, path)` for every role, in [`ROLES`] order.
    pub fn entries(&self) -> [(&'static str, &str); 9] {
        [
            ("main", &self.main),
            ("lib", &self.lib),
            ("routes", &self.routes),
            ("models", &self.models),
            ("jobs", &self.jobs),
            ("migrations", &self.migrations),
            ("templates", &self.templates),
            ("tests", &self.tests),
            ("static", &self.static_files),
        ]
    }

    /// The path of `role` (a [`ROLES`] name), or `None` for another name.
    pub fn get(&self, role: &str) -> Option<&str> {
        self.entries().into_iter().find(|(r, _)| *r == role).map(|(_, p)| p)
    }

    fn slot(&mut self, role: &str) -> Option<&mut String> {
        Some(match role {
            "main" => &mut self.main,
            "lib" => &mut self.lib,
            "routes" => &mut self.routes,
            "models" => &mut self.models,
            "jobs" => &mut self.jobs,
            "migrations" => &mut self.migrations,
            "templates" => &mut self.templates,
            "tests" => &mut self.tests,
            "static" => &mut self.static_files,
            _ => return None,
        })
    }
}

/// Whether a role's path is a file (`main`, `lib`) or a directory (the
/// rest).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
}

impl EntryKind {
    /// `"file"` or `"dir"`.
    pub fn as_str(self) -> &'static str {
        match self {
            EntryKind::File => "file",
            EntryKind::Dir => "dir",
        }
    }

    fn of(role: &str) -> EntryKind {
        if role == "main" || role == "lib" {
            EntryKind::File
        } else {
            EntryKind::Dir
        }
    }
}

/// One role of the layout as found on disk ([`AppManifest::layout`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutEntry {
    /// The role ([`ROLES`]).
    pub role: &'static str,
    /// Its path, relative to the project root.
    pub path: String,
    /// A file or a directory.
    pub kind: EntryKind,
    /// Whether that file (or directory) exists.
    pub exists: bool,
}

/// `rws.app.toml`, parsed: the app's name and where each part of it lives.
///
/// [`Default`] is the conventional layout with no name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppManifest {
    /// `[app] name` (or a top-level `name`). [`AppManifest::load`] falls back
    /// to the `[package] name` of the project's `Cargo.toml`, then to the
    /// directory's name; [`AppManifest::from_str`] leaves it empty.
    pub name: String,
    /// `[paths]`, the layout's defaults for every key not given.
    pub paths: AppPaths,
    /// The file it was read from — `None` when [`AppManifest::load`] found
    /// none (the conventional layout) or for [`AppManifest::from_str`].
    pub file: Option<PathBuf>,
    /// Keys and tables the parser doesn't know, as `line N: …` notes.
    pub warnings: Vec<String>,
}

/// Why a manifest couldn't be read: the line (1-based; `0` when not about
/// one line) and what to change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestError {
    pub line: usize,
    pub message: String,
}

impl ManifestError {
    fn at(line: usize, message: impl Into<String>) -> Self {
        ManifestError { line, message: message.into() }
    }
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{FILE_NAME}: {}", self.message)
        } else {
            write!(f, "{FILE_NAME} line {}: {}", self.line, self.message)
        }
    }
}

impl std::error::Error for ManifestError {}

impl std::str::FromStr for AppManifest {
    type Err = ManifestError;

    fn from_str(text: &str) -> Result<Self, ManifestError> {
        AppManifest::parse(text)
    }
}

impl AppManifest {
    /// The conventional layout for an app called `name`.
    pub fn conventional(name: impl Into<String>) -> Self {
        AppManifest { name: name.into(), ..AppManifest::default() }
    }

    /// Parse a manifest's text (also `text.parse::<AppManifest>()`).
    ///
    /// Tables `[app]` (`name`) and `[paths]` (the [`ROLES`]); values are
    /// strings; paths are relative to the project root and stay inside it
    /// (no leading `/`, no drive letter, no `..`). Unknown keys and tables
    /// become [`Self::warnings`].
    pub fn parse(text: &str) -> Result<Self, ManifestError> {
        if text.len() > MAX_MANIFEST_SIZE {
            return Err(ManifestError::at(0, format!("larger than {MAX_MANIFEST_SIZE} bytes")));
        }
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut manifest = AppManifest::default();
        let mut seen: Vec<String> = Vec::new();
        let mut table = String::new();
        for (index, raw) in text.lines().enumerate() {
            let number = index + 1;
            let line = strip_comment(raw).trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with("[[") {
                return Err(ManifestError::at(number, "array tables ([[…]]) are not part of rws.app.toml; use [app] and [paths]"));
            }
            if let Some(rest) = line.strip_prefix('[') {
                let Some(name) = rest.strip_suffix(']') else {
                    return Err(ManifestError::at(number, format!("`{line}` is not a table header; write [app] or [paths]")));
                };
                table = name.trim().to_string();
                if table != "app" && table != "paths" {
                    manifest.warnings.push(format!("line {number}: unknown table [{table}] ignored (known: [app], [paths])"));
                }
                continue;
            }
            let Some((raw_key, raw_value)) = split_key_value(line) else {
                return Err(ManifestError::at(number, format!("expected `key = \"value\"`, found `{line}`")));
            };
            let key = raw_key.trim();
            if key.is_empty() {
                return Err(ManifestError::at(number, "a key is missing before `=`"));
            }
            // A dotted key at the top level (`paths.routes = "…"`) is the
            // same as the key in its table.
            let (section, key) = match (table.is_empty(), key.split_once('.')) {
                (true, Some((t, k))) => (t.trim().to_string(), k.trim()),
                _ => (table.clone(), key),
            };
            if !section.is_empty() && section != "app" && section != "paths" {
                if table.is_empty() {
                    manifest.warnings.push(format!("line {number}: unknown key `{section}.{key}` ignored"));
                }
                continue;
            }
            let value = string_value(raw_value.trim(), key, number)?;
            let qualified = format!("{}.{key}", if section.is_empty() { "app" } else { &section });
            if seen.contains(&qualified) {
                return Err(ManifestError::at(number, format!("`{key}` is set twice; keep one")));
            }
            match (section.as_str(), key) {
                ("" | "app", "name") => {
                    if value.trim().is_empty() {
                        return Err(ManifestError::at(number, "`name` is empty; name the app or leave the key out"));
                    }
                    manifest.name = value.trim().to_string();
                }
                ("paths", role) => match manifest.paths.slot(role) {
                    Some(slot) => *slot = checked_path(&value, role, number)?,
                    None => manifest.warnings.push(format!(
                        "line {number}: unknown key `{role}` in [paths] ignored (known: {})",
                        ROLES.join(", ")
                    )),
                },
                (_, other) => manifest.warnings.push(format!("line {number}: unknown key `{other}` in [app] ignored (known: name)")),
            }
            seen.push(qualified);
        }
        Ok(manifest)
    }

    /// The manifest of the project in `dir`: its `rws.app.toml`, or the
    /// conventional layout when there is none. The name falls back to
    /// `Cargo.toml`'s `[package] name`, then to the directory's name.
    pub fn load(dir: impl AsRef<Path>) -> Result<Self, ManifestError> {
        let dir = dir.as_ref();
        let file = dir.join(FILE_NAME);
        let mut manifest = match std::fs::metadata(&file) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => AppManifest::default(),
            Err(e) => return Err(ManifestError::at(0, format!("{}: {e}", file.display()))),
            Ok(meta) if !meta.is_file() => return Err(ManifestError::at(0, format!("{} is not a file", file.display()))),
            Ok(meta) if meta.len() > MAX_MANIFEST_SIZE as u64 => {
                return Err(ManifestError::at(0, format!("{} is larger than {MAX_MANIFEST_SIZE} bytes", file.display())))
            }
            Ok(_) => {
                let bytes = std::fs::read(&file).map_err(|e| ManifestError::at(0, format!("{}: {e}", file.display())))?;
                let text = String::from_utf8(bytes).map_err(|_| ManifestError::at(0, format!("{} is not UTF-8", file.display())))?;
                let mut manifest = AppManifest::parse(&text)?;
                manifest.file = Some(file);
                manifest
            }
        };
        if manifest.name.is_empty() {
            manifest.name = cargo_package_name(dir)
                .or_else(|| dir.canonicalize().ok().and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned())))
                .unwrap_or_default();
        }
        Ok(manifest)
    }

    /// Every role of the layout, with whether its path exists under `dir`
    /// (a regular file for `main`/`lib`, a directory for the rest).
    pub fn layout(&self, dir: impl AsRef<Path>) -> Vec<LayoutEntry> {
        let dir = dir.as_ref();
        self.paths
            .entries()
            .into_iter()
            .map(|(role, path)| {
                let kind = EntryKind::of(role);
                let full = dir.join(path);
                let exists = match kind {
                    EntryKind::File => full.is_file(),
                    EntryKind::Dir => full.is_dir(),
                };
                LayoutEntry { role, path: path.to_string(), kind, exists }
            })
            .collect()
    }
}

/// Drop a trailing `# comment`, ignoring `#` inside quoted strings.
fn strip_comment(line: &str) -> &str {
    let mut quotes = Quotes::default();
    for (i, c) in line.char_indices() {
        if !quotes.step(c) && c == '#' {
            return &line[..i];
        }
    }
    line
}

/// `key = value` split at the first `=` outside quotes.
fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let mut quotes = Quotes::default();
    for (i, c) in line.char_indices() {
        if !quotes.step(c) && c == '=' {
            return Some((&line[..i], &line[i + 1..]));
        }
    }
    None
}

/// A quoted TOML string's contents; anything else is an error naming `key`.
fn string_value(raw: &str, key: &str, line: usize) -> Result<String, ManifestError> {
    // The closing quote must end the value: `"a" "b"` is not one string.
    if string_end(raw) != Some(raw.len()) {
        return Err(ManifestError::at(line, format!("`{key}` must be a quoted string, e.g. {key} = \"…\" (found `{raw}`)")));
    }
    let value = unquote(raw);
    if value.contains('\0') {
        return Err(ManifestError::at(line, format!("`{key}` contains a NUL byte")));
    }
    Ok(value)
}

/// The byte index just past the quote that closes the string `raw` starts
/// with (`None` when it doesn't start with a quote or never closes).
fn string_end(raw: &str) -> Option<usize> {
    let mut chars = raw.char_indices();
    let (_, quote) = chars.next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let mut escaped = false;
    for (i, c) in chars {
        if escaped {
            escaped = false;
        } else if quote == '"' && c == '\\' {
            escaped = true;
        } else if c == quote {
            return Some(i + c.len_utf8());
        }
    }
    None
}

/// A `[paths]` value, checked to stay inside the project: relative, no
/// `..`; a trailing `/` and leading `./` dropped.
fn checked_path(value: &str, role: &str, line: usize) -> Result<String, ManifestError> {
    let path = value.trim();
    let path = path.strip_prefix("./").unwrap_or(path);
    let path = path.trim_end_matches(['/', '\\']);
    if path.is_empty() {
        return Err(ManifestError::at(line, format!("`{role}` is empty; give a path relative to the project root, or leave the key out")));
    }
    let bytes = path.as_bytes();
    let drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    if path.starts_with('/') || path.starts_with('\\') || drive {
        return Err(ManifestError::at(line, format!("`{role}` = \"{path}\" is absolute; paths are relative to the project root")));
    }
    if path.split(['/', '\\']).any(|part| part == "..") {
        return Err(ManifestError::at(line, format!("`{role}` = \"{path}\" leaves the project (`..`); keep it inside the project root")));
    }
    Ok(path.to_string())
}

/// `[package] name` from `dir/Cargo.toml`, if it has one.
fn cargo_package_name(dir: &Path) -> Option<String> {
    let file = dir.join("Cargo.toml");
    let meta = std::fs::metadata(&file).ok()?;
    if !meta.is_file() || meta.len() > MAX_CARGO_TOML_SIZE {
        return None;
    }
    let text = std::fs::read_to_string(&file).ok()?;
    let mut in_package = false;
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        let Some((key, value)) = split_key_value(line) else { continue };
        if key.trim() == "name" {
            let name = unquote(value.trim());
            return (!name.is_empty() && !name.contains('{')).then_some(name);
        }
    }
    None
}
