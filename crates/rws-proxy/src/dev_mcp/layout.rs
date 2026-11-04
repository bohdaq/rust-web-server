//! `layout`: the project's `rws.app.toml` and which of the conventional
//! paths exist ([`crate::app_manifest`]), read on every call so an agent
//! sees the files it just added.

use std::path::{Path, PathBuf};

use crate::app_manifest::{AppManifest, EntryKind};
use crate::mcp::{McpContent, McpServer};

use super::{quoted, read_only_hint};

/// At most this many names are listed per directory.
const MAX_ENTRIES: usize = 100;

pub(super) fn install(mcp: McpServer, dir: PathBuf) -> McpServer {
    mcp.tool(
        "layout",
        "The app's layout: its name, rws.app.toml (or the conventional defaults when there is none), and for each part — main, lib, routes, models, jobs, migrations, templates, tests, static — its path, whether it exists, and the files in it",
        r#"{"type":"object"}"#,
        move |_| Ok(McpContent::json(layout_json(&dir))),
    )
    .tool_annotations("layout", read_only_hint())
}

/// What `layout` answers. A manifest that doesn't parse is reported in
/// `error`, with the conventional paths beside it.
pub(crate) fn layout_json(dir: &Path) -> String {
    let (manifest, error) = match AppManifest::load(dir) {
        Ok(manifest) => (manifest, None),
        Err(e) => (AppManifest::default(), Some(e.to_string())),
    };
    let paths: Vec<String> = manifest
        .layout(dir)
        .iter()
        .map(|entry| {
            let entries = if entry.exists && entry.kind == EntryKind::Dir {
                let names: Vec<String> = dir_entries(&dir.join(&entry.path)).iter().map(|n| quoted(n)).collect();
                format!(r#","entries":[{}]"#, names.join(","))
            } else {
                String::new()
            };
            format!(
                r#"{{"role":{},"path":{},"kind":{},"exists":{}{}}}"#,
                quoted(entry.role),
                quoted(&entry.path),
                quoted(entry.kind.as_str()),
                entry.exists,
                entries
            )
        })
        .collect();
    let warnings: Vec<String> = manifest.warnings.iter().map(|w| quoted(w)).collect();
    format!(
        r#"{{"dir":{},"manifest":{},"name":{},"error":{},"warnings":[{}],"paths":[{}]}}"#,
        quoted(&dir.to_string_lossy()),
        manifest.file.as_ref().map(|_| quoted(crate::app_manifest::FILE_NAME)).unwrap_or_else(|| "null".to_string()),
        quoted(&manifest.name),
        error.as_deref().map(quoted).unwrap_or_else(|| "null".to_string()),
        warnings.join(","),
        paths.join(",")
    )
}

/// The names in `dir`, sorted, directories with a trailing `/`; at most
/// [`MAX_ENTRIES`], then `"…"`.
fn dir_entries(dir: &Path) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut names: Vec<String> = read
        .filter_map(Result::ok)
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                format!("{name}/")
            } else {
                name
            }
        })
        .collect();
    names.sort();
    if names.len() > MAX_ENTRIES {
        names.truncate(MAX_ENTRIES);
        names.push("…".to_string());
    }
    names
}
