//! `rws gen model|route|migration|test` (18.29.0, roadmap #88): code
//! generators for a project in the conventional layout
//! ([`crate::app_manifest`]), as a library — the `rws gen` command and the
//! dev MCP endpoint's `gen_*` tools ([`crate::dev_mcp`]) are front ends.
//!
//! | Generator | Writes | Adds to |
//! |---|---|---|
//! | [`Generate::Model`] | `src/models/<name>.rs` (`#[derive(Model)]`, `id` auto-increment, a test), `migrations/NNNN_create_<table>.sql` + `.down.sql` | `src/models/mod.rs`, `src/lib.rs` (`pub mod models;`), `Cargo.toml` (the `model-<db>` feature) |
//! | [`Generate::Route`] | `src/routes/<resource>.rs` (typed handlers, `register(app)`) | `src/routes/mod.rs`, `app()` in `src/lib.rs` |
//! | [`Generate::Migration`] | `migrations/NNNN_<name>.sql` + `.down.sql` | — |
//! | [`Generate::Test`] | `tests/<resource>.rs` (`TestClient` against `app()`) | — |
//!
//! The paths come from `rws.app.toml`. The output is pinned to this
//! release's API: it compiles against the `rust-web-server` it came with
//! (CI generates into every starter and runs the generated tests).
//!
//! [`plan`] only reads; it answers a [`Plan`] — each file created, edited
//! or left alone, with a unified diff. [`Plan::apply`] writes it. Nothing
//! is ever overwritten: a file that exists with other content is a
//! [`ChangeKind::Conflict`] (the diff shows what the generator would have
//! written) and the whole plan is refused, so running a generator twice
//! changes nothing the second time. Edits to existing files only add a
//! line (`pub mod posts;`, a feature, a `register` call) and are skipped
//! when it is there; what can't be added safely is a [`Plan::notes`] line
//! saying what to add by hand.
//!
//! ```rust,no_run
//! # extern crate rws_proxy as rust_web_server;
//! use rust_web_server::generator::{plan, Field, Generate};
//!
//! let request = Generate::Model {
//!     name: "Post".into(),
//!     fields: vec![Field::parse("title:String").unwrap(), Field::parse("views:i64").unwrap()],
//!     db: None, // from Cargo.toml's model-* feature, else SQLite
//!     timestamps: false,
//!     soft_delete: false,
//! };
//! let plan = plan(".", &request).unwrap();
//! println!("{}", plan.render(true)); // what would change, with diffs
//! if !plan.has_conflicts() {
//!     plan.apply(".").unwrap();
//! }
//! ```
//!
//! Names are identifiers (letters, digits, `_`), so no path leaves the
//! project; field types are a fixed list ([`FieldType`]).

use std::fmt;
use std::path::{Path, PathBuf};

use crate::app_manifest::AppManifest;
use crate::core::json::escape as json_escape;

mod cli;
mod deploy;
mod diff;
mod from_models;
mod names;
mod scan;
mod templates;
#[cfg(test)]
mod tests;

pub use cli::{requested, run, EXIT_CONFLICT, EXIT_OK, EXIT_USAGE};
pub use deploy::{k8s_name, plan_deploy, DeploySpec};
pub use from_models::{plan_model_migration, ExistingColumns};
pub use names::{parse_fields, BaseType, Field, FieldType, FIELD_TYPES, MAX_NAME_LEN};

use names::{check_snake, check_type_name, pluralize, singularize, to_camel, to_snake};

/// The database a model's migration is written for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Db {
    Sqlite,
    Postgres,
    Mysql,
}

impl Db {
    /// `sqlite`, `postgres` (or `postgresql`), `mysql`.
    pub fn parse(text: &str) -> Option<Db> {
        match text.trim().to_ascii_lowercase().as_str() {
            "sqlite" => Some(Db::Sqlite),
            "postgres" | "postgresql" => Some(Db::Postgres),
            "mysql" => Some(Db::Mysql),
            _ => None,
        }
    }

    /// The `rust-web-server` feature for it (`model-sqlite`, …).
    pub fn feature(self) -> &'static str {
        match self {
            Db::Sqlite => "model-sqlite",
            Db::Postgres => "model-postgres",
            Db::Mysql => "model-mysql",
        }
    }

    /// `sqlite`, `postgres`, `mysql`.
    pub fn as_str(self) -> &'static str {
        match self {
            Db::Sqlite => "sqlite",
            Db::Postgres => "postgres",
            Db::Mysql => "mysql",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Db::Sqlite => "SQLite",
            Db::Postgres => "PostgreSQL",
            Db::Mysql => "MySQL",
        }
    }
}

/// One of the routes `rws gen route` writes for a resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteAction {
    /// `GET /posts`
    List,
    /// `GET /posts/{id}`
    Show,
    /// `POST /posts`
    Create,
    /// `PUT /posts/{id}`
    Update,
    /// `DELETE /posts/{id}`
    Remove,
}

impl RouteAction {
    /// Every action, in the order the routes are written.
    pub const ALL: [RouteAction; 5] = [RouteAction::List, RouteAction::Show, RouteAction::Create, RouteAction::Update, RouteAction::Remove];

    /// `list`, `show`, `create`, `update`, `remove` (also `delete`).
    pub fn parse(text: &str) -> Option<RouteAction> {
        match text.trim() {
            "list" | "index" => Some(RouteAction::List),
            "show" | "get" => Some(RouteAction::Show),
            "create" => Some(RouteAction::Create),
            "update" => Some(RouteAction::Update),
            "remove" | "delete" | "destroy" => Some(RouteAction::Remove),
            _ => None,
        }
    }

    /// `list,show` → the actions in [`Self::ALL`] order, once each.
    pub fn parse_list(text: &str) -> Result<Vec<RouteAction>, GenError> {
        let mut wanted = Vec::new();
        for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let action = RouteAction::parse(part)
                .ok_or_else(|| GenError::usage(format!("unknown method `{part}`: use list, show, create, update, remove")))?;
            wanted.push(action);
        }
        if wanted.is_empty() {
            return Err(GenError::usage("--methods is empty: name at least one of list, show, create, update, remove"));
        }
        Ok(RouteAction::ALL.into_iter().filter(|a| wanted.contains(a)).collect())
    }

    /// The handler's name, also the action's name.
    pub fn handler(self) -> &'static str {
        match self {
            RouteAction::List => "list",
            RouteAction::Show => "show",
            RouteAction::Create => "create",
            RouteAction::Update => "update",
            RouteAction::Remove => "remove",
        }
    }

    fn verb(self) -> &'static str {
        self.handler()
    }

    fn macro_name(self) -> &'static str {
        match self {
            RouteAction::List | RouteAction::Show => "get",
            RouteAction::Create => "post",
            RouteAction::Update => "put",
            RouteAction::Remove => "delete",
        }
    }

    fn method(self) -> &'static str {
        match self {
            RouteAction::List | RouteAction::Show => "GET",
            RouteAction::Create => "POST",
            RouteAction::Update => "PUT",
            RouteAction::Remove => "DELETE",
        }
    }

    fn with_id(self) -> bool {
        matches!(self, RouteAction::Show | RouteAction::Update | RouteAction::Remove)
    }
}

/// What to generate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Generate {
    /// `rws gen model <Name> field:type…`: a `#[derive(Model)]` type and
    /// the migration creating its table. `db: None` takes the database
    /// from the `model-*` feature in `Cargo.toml`, else SQLite.
    /// `timestamps` (`--timestamps`, 18.30.0) adds `#[timestamps]` with
    /// `created_at` / `updated_at` columns, `soft_delete`
    /// (`--soft-delete`) `#[soft_delete]` with a nullable `deleted_at`.
    Model { name: String, fields: Vec<Field>, db: Option<Db>, timestamps: bool, soft_delete: bool },
    /// `rws gen route <resource> [field:type…]`: typed handlers for
    /// `actions` under `prefix` + `/<resource>`, and `register(app)`.
    /// Without fields: the fields of `src/models/<singular>.rs` when
    /// there is one, else `name: String`.
    Route { resource: String, fields: Vec<Field>, actions: Vec<RouteAction>, prefix: String },
    /// `rws gen migration <name>`: an up/down pair to fill in.
    Migration { name: String },
    /// `rws gen test <resource>`: `TestClient` tests for the routes in
    /// `src/routes/<resource>.rs`. `app_expr` is how a test builds the
    /// app (default `app()`, or what the existing tests pass to
    /// `TestClient::new` when `app` takes arguments).
    Test { resource: String, app_expr: Option<String> },
}

impl Generate {
    /// `model`, `route`, `migration`, `test`.
    pub fn kind(&self) -> &'static str {
        match self {
            Generate::Model { .. } => "model",
            Generate::Route { .. } => "route",
            Generate::Migration { .. } => "migration",
            Generate::Test { .. } => "test",
        }
    }

    /// The command line that asks for this (written into the files).
    pub fn command_line(&self) -> String {
        let mut words = vec!["rws".to_string(), "gen".to_string(), self.kind().to_string()];
        match self {
            Generate::Model { name, fields, db, timestamps, soft_delete } => {
                words.push(name.clone());
                words.extend(fields.iter().map(Field::spec));
                if let Some(db) = db {
                    words.push(format!("--db {}", db.as_str()));
                }
                if *timestamps {
                    words.push("--timestamps".to_string());
                }
                if *soft_delete {
                    words.push("--soft-delete".to_string());
                }
            }
            Generate::Route { resource, fields, actions, prefix } => {
                words.push(resource.clone());
                words.extend(fields.iter().map(Field::spec));
                if actions.as_slice() != RouteAction::ALL {
                    let names: Vec<&str> = actions.iter().map(|a| a.handler()).collect();
                    words.push(format!("--methods {}", names.join(",")));
                }
                let prefix = prefix.trim().trim_end_matches('/');
                if !prefix.is_empty() {
                    words.push(format!("--prefix {prefix}"));
                }
            }
            Generate::Migration { name } => words.push(name.clone()),
            Generate::Test { resource, app_expr } => {
                words.push(resource.clone());
                if let Some(expr) = app_expr {
                    words.push(format!("--app-expr '{expr}'"));
                }
            }
        }
        words.join(" ")
    }
}

/// Why a generator refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenError {
    /// `true`: the request itself is wrong (a bad name or type, a missing
    /// argument) — exit code 2; `false`: the project isn't in a state the
    /// generator can work with — exit code 1.
    pub usage: bool,
    pub message: String,
}

impl GenError {
    fn usage(message: impl Into<String>) -> Self {
        GenError { usage: true, message: message.into() }
    }

    fn refused(message: impl Into<String>) -> Self {
        GenError { usage: false, message: message.into() }
    }

    /// `{"status":"refused"|"usage","error":…}`.
    pub fn to_json(&self) -> String {
        format!(r#"{{"status":"{}","error":"{}"}}"#, if self.usage { "usage" } else { "refused" }, json_escape(&self.message))
    }
}

impl fmt::Display for GenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for GenError {}

/// What happens to one file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    /// A new file.
    Create,
    /// An existing file gets a line.
    Edit,
    /// The file is already what the generator would write.
    Unchanged,
    /// The file exists with other content: not overwritten.
    Conflict,
}

impl ChangeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ChangeKind::Create => "create",
            ChangeKind::Edit => "edit",
            ChangeKind::Unchanged => "unchanged",
            ChangeKind::Conflict => "conflict",
        }
    }
}

/// One file of a [`Plan`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChange {
    /// Relative to the project root, `/`-separated.
    pub path: String,
    pub kind: ChangeKind,
    /// The file's content after the change (for a conflict: what the
    /// generator would have written).
    pub content: String,
    /// The file's content now (`None`: it doesn't exist).
    pub original: Option<String>,
    /// A unified diff from `original` to `content` (empty when
    /// unchanged).
    pub diff: String,
}

/// What a generator would do: the files, and what to add by hand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// The command line it answers (`rws gen model Post title:String`).
    pub command: String,
    pub changes: Vec<FileChange>,
    /// Steps the generator leaves to the person (a line it couldn't add
    /// safely, a model to register with the dev MCP endpoint).
    pub notes: Vec<String>,
}

impl Plan {
    /// Any file in the way ([`ChangeKind::Conflict`]).
    pub fn has_conflicts(&self) -> bool {
        self.changes.iter().any(|c| c.kind == ChangeKind::Conflict)
    }

    /// Nothing to write: every file is already as generated.
    pub fn is_unchanged(&self) -> bool {
        self.changes.iter().all(|c| c.kind == ChangeKind::Unchanged)
    }

    /// `conflict`, `unchanged`, or `planned` (something to write).
    pub fn status(&self) -> &'static str {
        if self.has_conflicts() {
            "conflict"
        } else if self.is_unchanged() {
            "unchanged"
        } else {
            "planned"
        }
    }

    /// Write the plan under `dir`: new files are created (never replacing
    /// one that appeared since [`plan`]), edited files rewritten only if
    /// they still hold what was read. Refused whole when there is a
    /// conflict. Answers the paths written.
    pub fn apply(&self, dir: impl AsRef<Path>) -> Result<Vec<String>, GenError> {
        let dir = dir.as_ref();
        if self.has_conflicts() {
            let paths: Vec<&str> = self.changes.iter().filter(|c| c.kind == ChangeKind::Conflict).map(|c| c.path.as_str()).collect();
            return Err(GenError::refused(format!("not written: {} already exist(s) with other content", paths.join(", "))));
        }
        // Check every edit before writing anything.
        for change in self.changes.iter().filter(|c| c.kind == ChangeKind::Edit) {
            let now = std::fs::read_to_string(dir.join(&change.path)).ok();
            if now != change.original {
                return Err(GenError::refused(format!("{} changed since the plan was made; run the generator again", change.path)));
            }
        }
        let mut written = Vec::new();
        for change in &self.changes {
            let path = dir.join(&change.path);
            match change.kind {
                ChangeKind::Create => {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| GenError::refused(format!("{}: {e}", parent.display())))?;
                    }
                    use std::io::Write;
                    let mut file = std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&path)
                        .map_err(|e| GenError::refused(format!("{}: {e}", change.path)))?;
                    file.write_all(change.content.as_bytes()).map_err(|e| GenError::refused(format!("{}: {e}", change.path)))?;
                }
                ChangeKind::Edit => {
                    std::fs::write(&path, &change.content).map_err(|e| GenError::refused(format!("{}: {e}", change.path)))?;
                }
                ChangeKind::Unchanged | ChangeKind::Conflict => continue,
            }
            written.push(change.path.clone());
        }
        Ok(written)
    }

    /// For people: one line per file (`create  src/models/post.rs`), the
    /// diffs of conflicts — and of every change with `diffs` — then the
    /// notes.
    pub fn render(&self, diffs: bool) -> String {
        let mut out = String::new();
        for change in &self.changes {
            out.push_str(&format!("{:<10} {}\n", change.kind.as_str(), change.path));
        }
        for change in &self.changes {
            let show = change.kind == ChangeKind::Conflict || (diffs && change.kind != ChangeKind::Unchanged);
            if show && !change.diff.is_empty() {
                out.push('\n');
                out.push_str(&change.diff);
            }
        }
        if !self.notes.is_empty() {
            out.push('\n');
            for note in &self.notes {
                out.push_str(&format!("note: {note}\n"));
            }
        }
        out
    }

    /// For agents: `{"command":…,"status":…,"written":…,"changes":[{"path","action","diff"}],"notes":[…]}`.
    /// `written`: whether [`Plan::apply`] ran.
    pub fn to_json(&self, written: bool) -> String {
        let changes: Vec<String> = self
            .changes
            .iter()
            .map(|c| format!(r#"{{"path":"{}","action":"{}","diff":"{}"}}"#, json_escape(&c.path), c.kind.as_str(), json_escape(&c.diff)))
            .collect();
        let notes: Vec<String> = self.notes.iter().map(|n| format!("\"{}\"", json_escape(n))).collect();
        let status = if written && !self.has_conflicts() && !self.is_unchanged() { "written" } else { self.status() };
        format!(
            r#"{{"command":"{}","status":"{status}","written":{written},"changes":[{}],"notes":[{}]}}"#,
            json_escape(&self.command),
            changes.join(","),
            notes.join(",")
        )
    }
}

/// What a generator reads from the project. [`Disk`] is the real one.
pub(crate) trait Files {
    fn read(&self, path: &str) -> Option<String>;
    /// File names in a directory (not recursive).
    fn list(&self, dir: &str) -> Vec<String>;
    fn manifest(&self) -> Result<AppManifest, GenError>;
}

/// The project on disk.
pub(crate) struct Disk(pub PathBuf);

/// The largest project file a generator reads (1 MiB).
const MAX_READ: u64 = 1024 * 1024;

impl Files for Disk {
    fn read(&self, path: &str) -> Option<String> {
        let full = self.0.join(path);
        let meta = std::fs::metadata(&full).ok()?;
        if !meta.is_file() || meta.len() > MAX_READ {
            return None;
        }
        std::fs::read_to_string(full).ok()
    }

    fn list(&self, dir: &str) -> Vec<String> {
        let Ok(read) = std::fs::read_dir(self.0.join(dir)) else { return Vec::new() };
        let mut names: Vec<String> = read.filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().into_owned()).take(10_000).collect();
        names.sort();
        names
    }

    fn manifest(&self) -> Result<AppManifest, GenError> {
        AppManifest::load(&self.0).map_err(|e| GenError::refused(e.to_string()))
    }
}

/// What `request` would change in the project at `dir` (reads only).
pub fn plan(dir: impl AsRef<Path>, request: &Generate) -> Result<Plan, GenError> {
    let dir = dir.as_ref();
    if !dir.join("Cargo.toml").is_file() {
        return Err(GenError::refused(format!("{} has no Cargo.toml: run rws gen in the project's root (or pass --dir)", dir.display())));
    }
    plan_with(&Disk(dir.to_path_buf()), request)
}

/// Collects the changes of one plan.
struct Builder<'a> {
    files: &'a dyn Files,
    changes: Vec<FileChange>,
    notes: Vec<String>,
}

impl<'a> Builder<'a> {
    /// A file the generator owns: created, unchanged, or a conflict.
    fn create(&mut self, path: &str, content: String) {
        let original = self.files.read(path);
        let (kind, diff) = match &original {
            None => (ChangeKind::Create, diff::unified(path, None, &content)),
            Some(now) if *now == content => (ChangeKind::Unchanged, String::new()),
            Some(now) => (ChangeKind::Conflict, diff::unified(path, Some(now), &content)),
        };
        self.changes.push(FileChange { path: path.to_string(), kind, content, original, diff });
    }

    /// An existing file's next content: `None` = unchanged. Later edits of
    /// the same path build on earlier ones.
    fn current(&self, path: &str) -> Option<String> {
        match self.changes.iter().rev().find(|c| c.path == path) {
            Some(change) => Some(change.content.clone()),
            None => self.files.read(path),
        }
    }

    fn edit(&mut self, path: &str, new: Option<String>) {
        let Some(new) = new else {
            if !self.changes.iter().any(|c| c.path == path) {
                if let Some(now) = self.files.read(path) {
                    self.changes.push(FileChange { path: path.to_string(), kind: ChangeKind::Unchanged, content: now.clone(), original: Some(now), diff: String::new() });
                }
            }
            return;
        };
        if let Some(existing) = self.changes.iter_mut().find(|c| c.path == path) {
            existing.diff = diff::unified(path, existing.original.as_deref(), &new);
            existing.content = new;
            if existing.kind == ChangeKind::Unchanged {
                existing.kind = ChangeKind::Edit;
            }
            return;
        }
        let original = self.files.read(path);
        let kind = if original.is_some() { ChangeKind::Edit } else { ChangeKind::Create };
        let diff = diff::unified(path, original.as_deref(), &new);
        self.changes.push(FileChange { path: path.to_string(), kind, content: new, original, diff });
    }

    fn note(&mut self, note: impl Into<String>) {
        self.notes.push(note.into());
    }
}

/// `src/models` relative to `src/lib.rs` → the module name `models`, when
/// the directory sits beside `lib.rs`.
fn module_beside(lib: &str, dir: &str) -> Option<String> {
    let lib_dir = lib.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let (parent, name) = dir.trim_end_matches('/').rsplit_once('/').unwrap_or(("", dir));
    (parent == lib_dir && check_snake(name, "module").is_ok()).then(|| name.to_string())
}

fn join(dir: &str, file: &str) -> String {
    let dir = dir.trim_end_matches('/');
    if dir.is_empty() || dir == "." {
        file.to_string()
    } else {
        format!("{dir}/{file}")
    }
}

/// `pub mod <module>;` in `<dir>/mod.rs` (created when missing, with
/// `fresh`), and `pub mod <dir module>;` in `lib.rs` when the directory is
/// new to it.
fn add_module(b: &mut Builder<'_>, manifest: &AppManifest, dir: &str, module: &str, fresh: String) {
    let mod_rs = join(dir, "mod.rs");
    match b.current(&mod_rs) {
        Some(text) => {
            let edited = scan::add_mod_line(&text, module);
            b.edit(&mod_rs, edited);
        }
        None => b.edit(&mod_rs, Some(fresh)),
    }
    let lib = manifest.paths.lib.clone();
    let Some(parent) = module_beside(&lib, dir) else {
        b.note(format!("{dir} isn't beside {lib}: declare the module where it belongs (`mod …;`)"));
        return;
    };
    match b.current(&lib) {
        Some(text) => {
            let edited = scan::add_mod_line(&text, &parent);
            b.edit(&lib, edited);
        }
        None => b.note(format!("there is no {lib}: add `pub mod {parent};` to the crate root")),
    }
}

/// The next migration number in `dir` (after every `NNNN_*.sql`) and its
/// width (at least 4).
fn next_migration(files: &dyn Files, dir: &str) -> (u64, usize) {
    let mut max = 0;
    let mut width = 4;
    for name in files.list(dir) {
        if let Some((n, w)) = scan::migration_number(&name) {
            max = max.max(n);
            width = width.max(w);
        }
    }
    (max + 1, width)
}

/// An existing `NNNN_<suffix>.sql` in `dir` (so a second run compares
/// with it instead of numbering a new one).
fn existing_migration(files: &dyn Files, dir: &str, suffix: &str) -> Option<String> {
    let wanted = format!("_{suffix}.sql");
    files.list(dir).into_iter().find(|name| scan::migration_number(name).is_some() && name.ends_with(&wanted) && !name.ends_with(".down.sql"))
}

/// The migration pair for `suffix`: `(up path, down path, stem)`.
fn migration_paths(files: &dyn Files, dir: &str, suffix: &str) -> (String, String, String) {
    let stem = match existing_migration(files, dir, suffix) {
        Some(name) => name.trim_end_matches(".sql").to_string(),
        None => {
            let (n, width) = next_migration(files, dir);
            format!("{n:0width$}_{suffix}")
        }
    };
    (join(dir, &format!("{stem}.sql")), join(dir, &format!("{stem}.down.sql")), stem)
}

fn cargo(files: &dyn Files) -> Result<String, GenError> {
    files.read("Cargo.toml").ok_or_else(|| GenError::refused("there is no Cargo.toml in the project's root"))
}

/// The database from `Cargo.toml`'s `model-*` features when `db` isn't
/// given: one of them, else SQLite.
fn choose_db(cargo: &str, db: Option<Db>) -> Result<Db, GenError> {
    if let Some(db) = db {
        return Ok(db);
    }
    let features = scan::rws_features(cargo).unwrap_or_default();
    let enabled: Vec<Db> = [Db::Sqlite, Db::Postgres, Db::Mysql].into_iter().filter(|d| features.iter().any(|f| f == d.feature())).collect();
    match enabled.as_slice() {
        [] => Ok(Db::Sqlite),
        [one] => Ok(*one),
        _ => Err(GenError::usage("Cargo.toml enables more than one model-* feature: say which database with --db sqlite|postgres|mysql")),
    }
}

pub(crate) fn plan_with(files: &dyn Files, request: &Generate) -> Result<Plan, GenError> {
    let manifest = files.manifest()?;
    let mut b = Builder { files, changes: Vec::new(), notes: Vec::new() };
    let command = request.command_line();
    match request {
        Generate::Model { name, fields, db, timestamps, soft_delete } => {
            let conventions = templates::Conventions { timestamps: *timestamps, soft_delete: *soft_delete };
            plan_model(&mut b, &manifest, &command, name, fields, *db, conventions)?
        }
        Generate::Route { resource, fields, actions, prefix } => plan_route(&mut b, &manifest, &command, resource, fields, actions, prefix)?,
        Generate::Migration { name } => {
            check_snake(name, "migration")?;
            let dir = manifest.paths.migrations.clone();
            let (up, down, stem) = migration_paths(files, &dir, name);
            b.create(&up, templates::empty_up(&command, &stem));
            b.create(&down, templates::empty_down(&stem));
        }
        Generate::Test { resource, app_expr } => plan_test(&mut b, &manifest, &command, resource, app_expr.as_deref())?,
    }
    Ok(Plan { command, changes: b.changes, notes: b.notes })
}

fn plan_model(
    b: &mut Builder<'_>,
    manifest: &AppManifest,
    command: &str,
    name: &str,
    fields: &[Field],
    db: Option<Db>,
    conventions: templates::Conventions,
) -> Result<(), GenError> {
    let module = to_snake(name, "model")?;
    let type_name = to_camel(&module);
    check_type_name(&type_name, "model")?;
    if fields.is_empty() {
        return Err(GenError::usage("a model needs at least one field besides `id`, e.g. `rws gen model Post title:String`"));
    }
    if let Some(f) = fields.iter().find(|f| conventions.columns().contains(&f.name.as_str())) {
        return Err(GenError::usage(format!(
            "`{}` is written by --timestamps / --soft-delete: leave it out of the fields",
            f.name
        )));
    }
    let table = pluralize(&module);
    let cargo = cargo(b.files)?;
    let db = choose_db(&cargo, db)?;
    let migrations = manifest.paths.migrations.clone();
    let (up, down, _) = migration_paths(b.files, &migrations, &format!("create_{table}"));
    let sqlite_test = (db == Db::Sqlite && scan::dependency_has(&cargo, "tokio", "macros")).then_some(migrations.as_str());
    if db == Db::Sqlite && sqlite_test.is_none() {
        b.note("no tokio dependency with the `macros` feature: the model's test checks the mapping only, not a round trip through SQLite");
    }
    let file = join(&manifest.paths.models, &format!("{module}.rs"));
    let text = templates::ModelText { command, type_name: &type_name, table: &table, fields, migration: &up, sqlite_test, conventions };
    b.create(&file, templates::model(&text));
    let models_dir = manifest.paths.models.clone();
    add_module(b, manifest, &models_dir, &module, templates::models_mod(&module, &type_name));
    let mod_rs = join(&models_dir, "mod.rs");
    if let Some(text) = b.current(&mod_rs) {
        let edited = scan::add_use_line(&text, &format!("pub use {module}::{type_name};"));
        b.edit(&mod_rs, edited);
    }
    match scan::add_rws_feature(&cargo, db.feature()) {
        Ok(edited) => b.edit("Cargo.toml", edited),
        Err(()) => b.note(format!("add the `{}` feature to the rust-web-server dependency in Cargo.toml", db.feature())),
    }
    if conventions != templates::Conventions::default() {
        // The model derives Serialize/Deserialize; Timestamp has them with `serde`.
        let now = b.current("Cargo.toml").unwrap_or_else(|| cargo.clone());
        match scan::add_rws_feature(&now, "serde") {
            Ok(edited) => b.edit("Cargo.toml", edited),
            Err(()) => b.note("add the `serde` feature to the rust-web-server dependency in Cargo.toml (Timestamp's Serialize and Deserialize)"),
        }
    }
    b.create(&up, templates::create_table_up_with(command, &table, fields, db, conventions));
    b.create(&down, templates::create_table_down(&table));
    let models_mod = module_beside(&manifest.paths.lib, &models_dir).unwrap_or_else(|| "models".to_string());
    b.note(format!(
        "to list it in the dev MCP endpoint's `models` tool, add `.model::<{models_mod}::{type_name}>()` to the DevTools in {}",
        manifest.paths.lib
    ));
    Ok(())
}

/// The resource's fields when none are given: the model's, else `name`.
fn default_fields(b: &mut Builder<'_>, manifest: &AppManifest, singular: &str, type_name: &str) -> Vec<Field> {
    let model = join(&manifest.paths.models, &format!("{singular}.rs"));
    if let Some(text) = b.files.read(&model) {
        if let Some(found) = scan::struct_fields(&text, type_name) {
            let fields: Option<Vec<Field>> = found
                .into_iter()
                // The key and the columns the repository writes (--timestamps, --soft-delete).
                .filter(|f| !matches!(f, Ok((name, _)) if ["id", "created_at", "updated_at", "deleted_at"].contains(&name.as_str())))
                .map(|f| f.ok().and_then(|(name, ty)| Field::parse(&format!("{name}:{ty}")).ok()))
                .collect();
            if let Some(fields) = fields.filter(|f| !f.is_empty()) {
                b.note(format!("fields taken from {model}"));
                return fields;
            }
        }
    }
    vec![Field { name: "name".to_string(), ty: FieldType { base: BaseType::String, optional: false } }]
}

fn check_prefix(prefix: &str) -> Result<String, GenError> {
    let prefix = prefix.trim().trim_end_matches('/');
    if prefix.is_empty() {
        return Ok(String::new());
    }
    let ok = prefix.starts_with('/')
        && prefix.len() <= 128
        && prefix[1..].split('/').all(|seg| !seg.is_empty() && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    if ok {
        Ok(prefix.to_string())
    } else {
        Err(GenError::usage(format!("--prefix `{prefix}`: a path like /api or /api/v1 (letters, digits, - and _ per segment)")))
    }
}

/// `app()` in `lib.rs`, or why `gen route`/`gen test` can't use it.
fn app_fn(b: &Builder<'_>, manifest: &AppManifest) -> Result<(String, scan::AppFn), GenError> {
    let lib = manifest.paths.lib.clone();
    let text = b.current(&lib).ok_or_else(|| GenError::refused(format!("there is no {lib}: the conventional layout builds the app in `pub fn app()` there")))?;
    let app = scan::find_app_fn(&text).ok_or_else(|| {
        GenError::refused(format!(
            "{lib} has no `pub fn app(…) -> AppWithState<…>` or `-> AsyncAppWithState<…>`: rws gen route and rws gen test write HTTP routes for such an app (an MCP server's tools are written by hand)"
        ))
    })?;
    Ok((text, app))
}

fn plan_route(
    b: &mut Builder<'_>,
    manifest: &AppManifest,
    command: &str,
    resource: &str,
    fields: &[Field],
    actions: &[RouteAction],
    prefix: &str,
) -> Result<(), GenError> {
    check_snake(resource, "resource")?;
    let singular = singularize(resource);
    let type_name = to_camel(&singular);
    check_type_name(&type_name, "resource")?;
    if actions.is_empty() {
        return Err(GenError::usage("no routes asked for: --methods takes list, show, create, update, remove"));
    }
    let prefix = check_prefix(prefix)?;
    let (lib_text, app) = app_fn(b, manifest)?;
    let fields = if fields.is_empty() { default_fields(b, manifest, &singular, &type_name) } else { fields.to_vec() };
    let base = format!("{prefix}/{resource}");
    let routes_dir = manifest.paths.routes.clone();
    let text = templates::RouteText {
        command,
        resource,
        base: &base,
        type_name: &type_name,
        singular: &singular,
        fields: &fields,
        actions,
        kind: app.kind,
        state: &app.state,
    };
    b.create(&join(&routes_dir, &format!("{resource}.rs")), templates::route(&text));
    add_module(b, manifest, &routes_dir, resource, templates::routes_mod(resource));
    let lib = manifest.paths.lib.clone();
    let routes_mod = module_beside(&lib, &routes_dir);
    let lib_now = b.current(&lib).unwrap_or(lib_text);
    // Re-find app(): a `pub mod` line added above it moved its body.
    let app = scan::find_app_fn(&lib_now).unwrap_or(app);
    match routes_mod.as_deref().map(|m| scan::wire_register(&lib_now, &app, m, resource)) {
        Some(Ok(edited)) => b.edit(&lib, edited),
        _ => b.note(format!(
            "register the routes in app() ({lib}): `let app = {}::{resource}::register(app);`",
            routes_mod.as_deref().unwrap_or("routes")
        )),
    }
    Ok(())
}

fn plan_test(b: &mut Builder<'_>, manifest: &AppManifest, command: &str, resource: &str, app_expr: Option<&str>) -> Result<(), GenError> {
    check_snake(resource, "resource")?;
    let (_, app) = app_fn(b, manifest)?;
    let route_file = join(&manifest.paths.routes, &format!("{resource}.rs"));
    let route_text = b
        .files
        .read(&route_file)
        .ok_or_else(|| GenError::refused(format!("there is no {route_file}: generate the routes first (rws gen route {resource})")))?;
    let cargo = cargo(b.files)?;
    let crate_name = scan::crate_name(&cargo).ok_or_else(|| GenError::refused("Cargo.toml has no [package] name"))?;
    let app_expr = match app_expr.map(str::trim) {
        Some(expr) => {
            let ok = expr.starts_with("app(") && expr.ends_with(')') && expr.len() <= 200 && !expr.contains(['\n', ';', '{', '}']);
            if !ok {
                return Err(GenError::usage("--app-expr: a call of app, e.g. 'app(Store::memory())' (one line, no `;` or braces)"));
            }
            expr.to_string()
        }
        None if app.no_params => "app()".to_string(),
        None => {
            let tests = manifest.paths.tests.clone();
            let found = b.files.list(&tests).into_iter().filter(|n| n.ends_with(".rs")).find_map(|n| b.files.read(&join(&tests, &n)).and_then(|t| scan::test_client_app_expr(&t)));
            found.ok_or_else(|| {
                GenError::usage(format!("app() takes arguments and no test in {tests}/ shows how to call it: pass --app-expr 'app(…)'"))
            })?
        }
    };
    // The routes the module declares, as `rws gen route` writes them.
    let routes = scan::declared_routes(&route_text);
    let collection = routes.iter().find(|r| !r.pattern.contains('{')).map(|r| r.pattern.clone()).or_else(|| {
        routes.iter().find_map(|r| r.pattern.strip_suffix("/{id}").map(str::to_string))
    });
    let Some(base) = collection else {
        return Err(GenError::refused(format!("{route_file} declares no `#[get(\"/…\")]`-style routes: write the test by hand")));
    };
    let actions: Vec<RouteAction> = RouteAction::ALL
        .into_iter()
        .filter(|a| {
            let pattern = if a.with_id() { format!("{base}/{{id}}") } else { base.clone() };
            routes.iter().any(|r| r.method == a.method() && r.pattern == pattern)
        })
        .collect();
    if actions.is_empty() {
        return Err(GenError::refused(format!("{route_file} has none of the routes rws gen route writes under {base}: write the test by hand")));
    }
    let needs_body = actions.contains(&RouteAction::Create) || actions.contains(&RouteAction::Update);
    let mut fields = Vec::new();
    if needs_body {
        let type_name = format!("New{}", to_camel(&singularize(resource)));
        let found = scan::struct_fields(&route_text, &type_name)
            .ok_or_else(|| GenError::refused(format!("{route_file} has no `pub struct {type_name} {{ … }}` to build a request body from")))?;
        for entry in found {
            let (name, ty) = entry.map_err(|line| GenError::refused(format!("{route_file}: can't read the field `{line}` of {type_name}")))?;
            let field = Field::parse(&format!("{name}:{ty}"))
                .map_err(|e| GenError::refused(format!("{route_file}: {type_name}.{name}: {}", e.message)))?;
            fields.push(field);
        }
    }
    let text = templates::TestText { command, crate_name: &crate_name, resource, base: &base, app_expr: &app_expr, actions: &actions, fields: &fields };
    b.create(&join(&manifest.paths.tests, &format!("{resource}.rs")), templates::test(&text));
    Ok(())
}

/// Every scanner of project text on `input` as each kind of file — the
/// fuzz target `generator` and the stack-depth test. Not an API.
#[doc(hidden)]
pub fn fuzz(input: &str) {
    if let Some(app) = scan::find_app_fn(input) {
        let _ = scan::wire_register(input, &app, "routes", "posts");
    }
    let _ = scan::add_mod_line(input, "posts");
    let _ = scan::add_use_line(input, "pub use post::Post;");
    let _ = scan::add_rws_feature(input, "model-sqlite");
    let _ = scan::rws_features(input);
    let _ = scan::crate_name(input);
    let _ = scan::dependency_has(input, "tokio", "macros");
    let _ = scan::test_client_app_expr(input);
    let _ = scan::declared_routes(input);
    let _ = scan::struct_fields(input, "NewPost");
    from_models::fuzz_models(input);
    for line in input.lines().take(64) {
        let _ = Field::parse(line);
        let _ = scan::migration_number(line);
        let _ = names::to_snake(line, "name");
        let _ = RouteAction::parse_list(line);
    }
    let _ = diff::unified("fuzz.rs", Some(input), "pub mod posts;\n");
}

