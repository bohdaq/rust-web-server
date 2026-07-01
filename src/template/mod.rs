//! Tera HTML template engine integration.
//!
//! Requires `features = ["tera"]`.
//!
//! # Quick start
//!
//! 1. Put templates in `templates/` (e.g. `templates/index.html`).
//! 2. Call [`init`] (or [`init_from_env`]) once at startup.
//! 3. Call [`render`] from any handler.
//!
//! ```rust,no_run
//! use rust_web_server::template::{self, Context};
//!
//! // At startup:
//! template::init("templates").unwrap();
//!
//! // In a handler:
//! let mut ctx = Context::new();
//! ctx.insert("title", "Home");
//! ctx.insert("items", &["Rust", "rws", "Tera"]);
//! let response = template::render("index.html", &ctx).unwrap();
//! ```
//!
//! Templates use [Jinja2 syntax](https://keats.github.io/tera/docs/#templates):
//! `{{ variable }}`, `{% if cond %}`, `{% for item in list %}`, `{% extends "base.html" %}`.

#[cfg(test)]
mod tests;

use std::sync::OnceLock;

pub use tera::Context;
pub use tera::Value;

use crate::core::New;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

static ENGINE: OnceLock<TeraEngine> = OnceLock::new();

/// A Tera template engine instance.
#[derive(Debug)]
pub struct TeraEngine {
    inner: tera::Tera,
}

impl TeraEngine {
    /// Load all templates matching `glob_pattern`, e.g. `"templates/**/*"`.
    ///
    /// The pattern is passed directly to [`tera::Tera::new`].
    pub fn from_glob(pattern: &str) -> Result<Self, String> {
        let tera = tera::Tera::new(pattern)
            .map_err(|e| format!("template engine init failed: {}", e))?;
        Ok(TeraEngine { inner: tera })
    }

    /// Load all templates from `dir`. Equivalent to `from_glob("dir/**/*")`.
    pub fn from_dir(dir: &str) -> Result<Self, String> {
        let pattern = format!("{}/**/*", dir.trim_end_matches('/'));
        Self::from_glob(&pattern)
    }

    /// Build an engine from in-memory template strings — useful for testing.
    ///
    /// `templates` is a slice of `(name, content)` pairs.
    pub fn from_raw(templates: &[(&str, &str)]) -> Result<Self, String> {
        let mut tera = tera::Tera::default();
        for (name, content) in templates {
            tera.add_raw_template(name, content)
                .map_err(|e| format!("failed to add template '{}': {}", name, e))?;
        }
        Ok(TeraEngine { inner: tera })
    }

    /// Render `template_name` with `ctx` and return the output as a `String`.
    pub fn render(&self, template_name: &str, ctx: &Context) -> Result<String, String> {
        self.inner
            .render(template_name, ctx)
            .map_err(|e| format!("render '{}' failed: {}", template_name, e))
    }

    /// Render `template_name` and wrap the output in a `200 OK` HTML [`Response`].
    pub fn response(&self, template_name: &str, ctx: &Context) -> Result<Response, String> {
        let html = self.render(template_name, ctx)?;
        Ok(html_response(html))
    }
}

// ── Global singleton ──────────────────────────────────────────────────────────

/// Initialize the global template engine from a directory on disk.
///
/// Call once at startup before any handlers run. Subsequent calls return an `Err`.
pub fn init(dir: &str) -> Result<(), String> {
    let engine = TeraEngine::from_dir(dir)?;
    ENGINE
        .set(engine)
        .map_err(|_| "template engine already initialized".to_string())
}

/// Initialize the global engine from `RWS_CONFIG_TEMPLATE_DIR` (default: `"templates"`).
pub fn init_from_env() -> Result<(), String> {
    let dir = std::env::var(crate::entry_point::Config::RWS_CONFIG_TEMPLATE_DIR)
        .unwrap_or_else(|_| {
            crate::entry_point::Config::RWS_CONFIG_TEMPLATE_DIR_DEFAULT_VALUE.to_string()
        });
    init(&dir)
}

/// Return a reference to the global template engine.
///
/// # Panics
///
/// Panics if [`init`] has not been called yet.
pub fn global() -> &'static TeraEngine {
    ENGINE
        .get()
        .expect("template engine not initialized — call template::init() at startup")
}

/// Render `template_name` with `ctx` using the global engine and return a `200 OK` HTML response.
pub fn render(template_name: &str, ctx: &Context) -> Result<Response, String> {
    global().response(template_name, ctx)
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn html_response(html: String) -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r.content_range_list = vec![Range::get_content_range(
        html.into_bytes(),
        MimeType::TEXT_HTML.to_string(),
    )];
    r
}
