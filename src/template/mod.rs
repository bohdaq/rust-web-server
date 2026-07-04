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
//!
//! # Hot reload
//!
//! Edit a template file and it's re-read from disk on the next [`reload`] —
//! no server restart, unlike a code change. [`reload`] is wired into the same
//! `SIGHUP` hook (`crate::config_reload::reload`) used for CORS, rate limits,
//! and TLS certs, so `kill -HUP $(pidof rws)` — or your own trigger calling
//! `config_reload::reload()` — picks up edited templates too:
//!
//! ```rust,no_run
//! use rust_web_server::template;
//!
//! template::init("templates").unwrap();
//! // ... edit templates/index.html on disk ...
//! template::reload().unwrap(); // re-globs the directory, replacing all templates
//! ```
//!
//! Only engines created from a glob ([`init`], [`init_from_env`],
//! [`TeraEngine::from_dir`], [`TeraEngine::from_glob`]) can reload — an
//! engine built from [`TeraEngine::from_raw`] has no directory to re-read,
//! so [`TeraEngine::reload`] returns `Err` for it.

#[cfg(test)]
mod tests;

use std::sync::{OnceLock, RwLock};

pub use tera::Context;
pub use tera::Value;

use crate::core::New;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

static ENGINE: OnceLock<RwLock<TeraEngine>> = OnceLock::new();

/// A Tera template engine instance.
#[derive(Debug)]
pub struct TeraEngine {
    inner: tera::Tera,
    /// The glob this engine was built from, remembered so [`reload`][Self::reload]
    /// can re-read it. `None` for [`from_raw`][Self::from_raw] engines.
    glob: Option<String>,
}

impl TeraEngine {
    /// Load all templates matching `glob_pattern`, e.g. `"templates/**/*"`.
    ///
    /// The pattern is passed directly to [`tera::Tera::new`].
    pub fn from_glob(pattern: &str) -> Result<Self, String> {
        let tera = tera::Tera::new(pattern)
            .map_err(|e| format!("template engine init failed: {}", e))?;
        Ok(TeraEngine { inner: tera, glob: Some(pattern.to_string()) })
    }

    /// Load all templates from `dir`. Equivalent to `from_glob("dir/**/*")`.
    pub fn from_dir(dir: &str) -> Result<Self, String> {
        let pattern = format!("{}/**/*", dir.trim_end_matches('/'));
        Self::from_glob(&pattern)
    }

    /// Build an engine from in-memory template strings — useful for testing.
    ///
    /// `templates` is a slice of `(name, content)` pairs. Has no glob, so
    /// [`reload`][Self::reload] always returns `Err` for an engine built this way.
    pub fn from_raw(templates: &[(&str, &str)]) -> Result<Self, String> {
        let mut tera = tera::Tera::default();
        for (name, content) in templates {
            tera.add_raw_template(name, content)
                .map_err(|e| format!("failed to add template '{}': {}", name, e))?;
        }
        Ok(TeraEngine { inner: tera, glob: None })
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

    /// Re-read every template from the glob this engine was created with,
    /// replacing all currently-loaded templates.
    ///
    /// Picks up edited content in existing files *and* newly added or removed
    /// files matching the original glob — it's a full re-glob, not a diff.
    ///
    /// Builds the replacement set of templates in full before swapping it in,
    /// rather than mutating the live template set in place (which is what
    /// [`tera::Tera::full_reload`] does, and why this doesn't just call it) —
    /// so a syntax error in one edited file fails the whole reload atomically
    /// and this engine keeps serving its last-known-good templates, instead
    /// of serving a half-reloaded, partially-broken set. Returns `Err` if:
    /// - this engine was built with [`TeraEngine::from_raw`] (no glob to
    ///   re-read), or
    /// - any template now fails to parse (e.g. a syntax error mid-edit) or
    ///   an `{% extends %}`/`{% import %}` chain no longer resolves.
    pub fn reload(&mut self) -> Result<(), String> {
        let pattern = self.glob.as_ref().ok_or_else(|| {
            "cannot reload: this engine was built with TeraEngine::from_raw, \
             which has no template directory to re-read"
                .to_string()
        })?;
        let fresh = tera::Tera::new(pattern)
            .map_err(|e| format!("template reload failed, previous templates are still active: {}", e))?;
        self.inner = fresh;
        Ok(())
    }
}

// ── Global singleton ──────────────────────────────────────────────────────────

/// Initialize the global template engine from a directory on disk.
///
/// Call once at startup before any handlers run. Subsequent calls return an `Err`.
pub fn init(dir: &str) -> Result<(), String> {
    let engine = TeraEngine::from_dir(dir)?;
    ENGINE
        .set(RwLock::new(engine))
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

/// Acquire a read lock on the global template engine. Held only for the
/// duration of one render — never store the guard across `.await` points or
/// requests.
///
/// # Panics
///
/// Panics if [`init`] has not been called yet.
pub fn global() -> std::sync::RwLockReadGuard<'static, TeraEngine> {
    ENGINE
        .get()
        .expect("template engine not initialized — call template::init() at startup")
        .read()
        .unwrap_or_else(|e| e.into_inner())
}

/// Render `template_name` with `ctx` using the global engine and return a `200 OK` HTML response.
pub fn render(template_name: &str, ctx: &Context) -> Result<Response, String> {
    global().response(template_name, ctx)
}

/// Reload the global template engine's templates from disk.
///
/// See the [module-level hot reload section](self#hot-reload). Wired into
/// [`crate::config_reload::reload`] automatically, so `SIGHUP` (or your own
/// trigger calling `config_reload::reload()`) reloads templates too — you
/// don't need to call this directly unless you want reload on a different
/// trigger.
///
/// # Errors
///
/// Returns `Err` if [`init`] hasn't been called yet, or if [`TeraEngine::reload`]
/// fails (in which case the previously-loaded templates are still active and
/// still being served — see [`TeraEngine::reload`]'s docs on why a bad edit
/// can't corrupt the live template set).
pub fn reload() -> Result<(), String> {
    let lock = ENGINE
        .get()
        .ok_or_else(|| "template engine not initialized — call template::init() at startup".to_string())?;
    let mut guard = lock.write().unwrap_or_else(|e| e.into_inner());
    guard.reload()
}

/// Reload the global template engine if [`init`] has been called; silently
/// does nothing otherwise.
///
/// Used by [`crate::config_reload::reload`] to safely fold template hot
/// reload into the same `SIGHUP` hook regardless of whether the calling
/// binary uses templates at all — most `tera`-feature-enabled binaries that
/// never called `template::init` shouldn't see a reload error every time
/// something else triggers a config reload.
pub(crate) fn reload_if_initialized() {
    if let Some(lock) = ENGINE.get() {
        let mut guard = lock.write().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = guard.reload() {
            eprintln!("[template] hot reload failed: {}", e);
        }
    }
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
