//! The Vite manifest helper (18.35.0, #94): the `<script>` and `<link>`
//! tags for an entry point of a frontend built by [Vite](https://vite.dev),
//! from its `.vite/manifest.json` in production or from the Vite dev
//! server in development. `serde` feature.
//!
//! ```rust
//! # extern crate rws_server as rust_web_server;
//! use rust_web_server::vite::Vite;
//!
//! let manifest = r#"{
//!   "src/main.ts": {"file": "assets/main-4f2a.js", "isEntry": true, "css": ["assets/main-9c1b.css"], "imports": ["_vendor.js"]},
//!   "_vendor.js": {"file": "assets/vendor-77aa.js"}
//! }"#;
//! let vite = Vite::from_manifest_str(manifest, "/").unwrap();
//! let tags = vite.tags("src/main.ts").unwrap();
//! assert!(tags.contains(r#"<script type="module" src="/assets/main-4f2a.js"></script>"#));
//! assert!(tags.contains(r#"<link rel="stylesheet" href="/assets/main-9c1b.css">"#));
//! assert!(tags.contains(r#"<link rel="modulepreload" href="/assets/vendor-77aa.js">"#));
//!
//! let dev = Vite::dev("http://localhost:5173");
//! assert!(dev.tags("src/main.ts").unwrap().contains("http://localhost:5173/@vite/client"));
//! ```
//!
//! Configure Vite with `build.manifest: true`; the manifest is
//! `dist/.vite/manifest.json`. [`Vite::from_env`] picks the mode:
//! `VITE_DEV_SERVER` set → dev server, else the manifest at `VITE_MANIFEST`
//! (default `frontend/dist/.vite/manifest.json`). [`install`] makes one
//! process-wide, which the `vite(entry=…)` template function reads.

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashSet};
use std::sync::{OnceLock, RwLock};

/// One manifest chunk (only the fields the tags need).
#[derive(Debug, Clone, Default, serde::Deserialize)]
struct Chunk {
    file: String,
    #[serde(default)]
    css: Vec<String>,
    #[serde(default)]
    imports: Vec<String>,
}

/// The tags for a Vite build or dev server.
#[derive(Debug, Clone)]
pub enum Vite {
    /// Built assets: chunks keyed by their source path, served under `base`.
    Manifest { base: String, chunks: BTreeMap<String, ChunkFiles> },
    /// The dev server at this URL (`http://localhost:5173`).
    Dev(String),
}

/// A manifest chunk's output files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChunkFiles {
    pub file: String,
    pub css: Vec<String>,
    pub imports: Vec<String>,
}

impl Vite {
    /// Read the manifest at `path`, assets served under `base` (`/`).
    pub fn from_manifest_file(path: &str, base: &str) -> Result<Vite, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("Vite manifest {path}: {e}"))?;
        Self::from_manifest_str(&text, base)
    }

    /// Parse a manifest's JSON, assets served under `base` (`/`).
    pub fn from_manifest_str(json: &str, base: &str) -> Result<Vite, String> {
        let raw: BTreeMap<String, Chunk> = serde_json::from_str(json).map_err(|e| format!("Vite manifest: {e}"))?;
        let chunks = raw.into_iter().map(|(k, c)| (k, ChunkFiles { file: c.file, css: c.css, imports: c.imports })).collect();
        let base = if base.ends_with('/') { base.to_string() } else { format!("{base}/") };
        Ok(Vite::Manifest { base, chunks })
    }

    /// Tags that load entries from the Vite dev server at `url`.
    pub fn dev(url: &str) -> Vite {
        Vite::Dev(url.trim_end_matches('/').to_string())
    }

    /// `VITE_DEV_SERVER` → [`Vite::dev`]; else the manifest at
    /// `VITE_MANIFEST` (default `frontend/dist/.vite/manifest.json`), assets
    /// under `VITE_BASE` (default `/`).
    pub fn from_env() -> Result<Vite, String> {
        if let Ok(url) = std::env::var("VITE_DEV_SERVER") {
            if !url.is_empty() {
                return Ok(Vite::dev(&url));
            }
        }
        let path = std::env::var("VITE_MANIFEST").unwrap_or_else(|_| "frontend/dist/.vite/manifest.json".into());
        let base = std::env::var("VITE_BASE").unwrap_or_else(|_| "/".into());
        Self::from_manifest_file(&path, &base)
    }

    /// The HTML for `entry` (`src/main.ts`): its CSS and that of the chunks
    /// it imports, `modulepreload` links for the imports, and the module
    /// script. `None` when the manifest has no such entry.
    pub fn tags(&self, entry: &str) -> Option<String> {
        match self {
            Vite::Dev(url) => Some(format!(
                "<script type=\"module\" src=\"{0}/@vite/client\"></script>\n<script type=\"module\" src=\"{0}/{1}\"></script>",
                escape(url),
                escape(entry.trim_start_matches('/'))
            )),
            Vite::Manifest { base, chunks } => {
                let root = chunks.get(entry)?;
                let mut css = Vec::new();
                let mut preload = Vec::new();
                css.extend(root.css.iter().cloned());
                // Every chunk the entry imports, directly or not, once (the
                // graph can have cycles).
                let mut seen: HashSet<&str> = HashSet::new();
                let mut queue: Vec<&str> = root.imports.iter().map(String::as_str).collect();
                while let Some(key) = queue.pop() {
                    if !seen.insert(key) {
                        continue;
                    }
                    if let Some(chunk) = chunks.get(key) {
                        preload.push(chunk.file.clone());
                        css.extend(chunk.css.iter().cloned());
                        queue.extend(chunk.imports.iter().map(String::as_str));
                    }
                }
                let mut out = Vec::new();
                let mut emitted = HashSet::new();
                for file in css {
                    if emitted.insert(file.clone()) {
                        out.push(format!("<link rel=\"stylesheet\" href=\"{}{}\">", escape(base), escape(&file)));
                    }
                }
                for file in preload {
                    if emitted.insert(file.clone()) {
                        out.push(format!("<link rel=\"modulepreload\" href=\"{}{}\">", escape(base), escape(&file)));
                    }
                }
                out.push(format!("<script type=\"module\" src=\"{}{}\"></script>", escape(base), escape(&root.file)));
                Some(out.join("\n"))
            }
        }
    }

    /// The URL of the built file for `source` (an image, a font): its
    /// manifest `file` under `base`, or the dev server's URL for it.
    pub fn asset(&self, source: &str) -> Option<String> {
        match self {
            Vite::Dev(url) => Some(format!("{url}/{}", source.trim_start_matches('/'))),
            Vite::Manifest { base, chunks } => chunks.get(source).map(|c| format!("{base}{}", c.file)),
        }
    }
}

/// HTML attribute escaping.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;")
}

fn global() -> &'static RwLock<Option<Vite>> {
    static VITE: OnceLock<RwLock<Option<Vite>>> = OnceLock::new();
    VITE.get_or_init(|| RwLock::new(None))
}

/// Make `vite` the process-wide helper the `vite` template function uses
/// (installing again replaces it).
pub fn install(vite: Vite) {
    if let Ok(mut slot) = global().write() {
        *slot = Some(vite);
    }
}

/// [`Vite::tags`] on the installed helper; `None` before [`install`].
pub fn tags(entry: &str) -> Option<String> {
    global().read().ok()?.as_ref()?.tags(entry)
}

/// [`Vite::asset`] on the installed helper; `None` before [`install`].
pub fn asset(source: &str) -> Option<String> {
    global().read().ok()?.as_ref()?.asset(source)
}
