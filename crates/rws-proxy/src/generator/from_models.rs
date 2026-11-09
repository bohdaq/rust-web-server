//! `rws gen migration <name> --from-models` (18.35.0): the migration that
//! brings a database up to the project's `#[derive(Model)]` structs —
//! `CREATE TABLE` for a model whose table isn't there, `ALTER TABLE … ADD
//! COLUMN` for a field whose column isn't. Additive only: nothing is ever
//! dropped or changed; columns the models no longer have, and fields whose
//! type it can't map, are listed in the plan's notes for a person to write.

use std::collections::BTreeMap;
use std::path::Path;

use super::names::check_snake;
use super::{join, migration_paths, Builder, Db, Disk, Files, GenError, Plan};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The database as `--from-models` sees it: each table's column names.
pub type ExistingColumns = BTreeMap<String, Vec<String>>;

/// A model field as its column.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Column {
    name: String,
    /// The SQL type with `NOT NULL` when required; `None`: a type this
    /// doesn't map.
    sql: Option<String>,
    rust: String,
    optional: bool,
    primary_key: bool,
    auto_increment: bool,
}

/// A `#[derive(Model)]` struct as found in a file.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ModelInfo {
    type_name: String,
    table: String,
    file: String,
    columns: Vec<Column>,
}

/// The SQL type of a Rust field type on `db`, `NOT NULL` unless optional.
fn sql_for(rust: &str, db: Db) -> (Option<String>, bool) {
    let rust = rust.trim();
    let (inner, optional) = match rust.strip_prefix("Option<").and_then(|r| r.strip_suffix('>')) {
        Some(inner) => (inner.trim(), true),
        None => (rust, false),
    };
    let last = inner.rsplit("::").next().unwrap_or(inner);
    let ty = match (last, db) {
        ("i64" | "u64", Db::Sqlite) | ("i32" | "u32" | "i16", Db::Sqlite) => "INTEGER",
        ("i64" | "u64", _) => "BIGINT",
        ("i32" | "u32", Db::Postgres) => "INTEGER",
        ("i32" | "u32", Db::Mysql) => "INT",
        ("i16", _) => "SMALLINT",
        ("f64" | "f32", Db::Sqlite) => "REAL",
        ("f64" | "f32", Db::Postgres) => "DOUBLE PRECISION",
        ("f64" | "f32", Db::Mysql) => "DOUBLE",
        ("bool", Db::Sqlite) => "INTEGER",
        ("bool", _) => "BOOLEAN",
        ("String", _) => "TEXT",
        ("Timestamp", Db::Sqlite) => "TEXT",
        ("Timestamp", Db::Postgres) => "TIMESTAMPTZ",
        ("Timestamp", Db::Mysql) => "DATETIME(6)",
        ("Uuid", Db::Postgres) => "UUID",
        ("Uuid", _) => "CHAR(36)",
        // A file in storage: one text column (`rust_web_server::attachment`).
        ("Attachment", _) => "TEXT",
        _ => return (None, optional),
    };
    (Some(if optional { ty.to_string() } else { format!("{ty} NOT NULL") }), optional)
}

/// A literal to fill a new `NOT NULL` column of existing rows with.
fn default_for(rust: &str, db: Db) -> Option<&'static str> {
    let last = rust.trim().rsplit("::").next().unwrap_or(rust);
    Some(match last {
        "i64" | "u64" | "i32" | "u32" | "i16" | "f64" | "f32" => "0",
        "bool" if db == Db::Sqlite => "0",
        "bool" => "FALSE",
        "String" | "Attachment" => "''",
        _ => return None,
    })
}

/// The value of `name = "…"` inside an attribute line.
fn attr_value(line: &str, name: &str) -> Option<String> {
    let at = line.find(name)?;
    let rest = line[at + name.len()..].trim_start().strip_prefix('=')?.trim_start().strip_prefix('"')?;
    Some(rest[..rest.find('"')?].to_string())
}

/// Every `#[derive(… Model …)]` struct in `text`. Line based, one pass.
fn models_in(text: &str, file: &str, db: Db) -> Vec<ModelInfo> {
    let mut out = Vec::new();
    let mut attrs: Vec<&str> = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let t = line.trim();
        if t.starts_with("#[") {
            attrs.push(t);
            continue;
        }
        if t.is_empty() || t.starts_with("//") {
            continue;
        }
        let is_model = attrs.iter().any(|a| a.starts_with("#[derive(") && a.split(|c: char| !c.is_alphanumeric() && c != '_').any(|w| w == "Model"));
        let header = t.strip_prefix("pub ").unwrap_or(t).strip_prefix("struct ").map(|r| r.trim_end_matches('{').trim());
        match header {
            Some(name) if is_model && t.ends_with('{') => {
                let table = attrs.iter().find(|a| a.starts_with("#[table(")).and_then(|a| attr_value(a, "name")).unwrap_or_else(|| name.to_lowercase());
                let mut columns = Vec::new();
                let mut field_attrs: Vec<&str> = Vec::new();
                for field in lines.by_ref() {
                    let f = field.trim();
                    if f == "}" {
                        break;
                    }
                    if f.starts_with("#[") {
                        field_attrs.push(f);
                        continue;
                    }
                    if f.is_empty() || f.starts_with("//") {
                        continue;
                    }
                    let decl = f.strip_prefix("pub(crate) ").or_else(|| f.strip_prefix("pub ")).unwrap_or(f).trim_end_matches(',');
                    if let Some((fname, ty)) = decl.split_once(':') {
                        let ignored = field_attrs.iter().any(|a| a.starts_with("#[ignore"));
                        if !ignored {
                            let column = field_attrs.iter().find(|a| a.starts_with("#[column(")).and_then(|a| attr_value(a, "name")).unwrap_or_else(|| fname.trim().to_string());
                            let pk = field_attrs.iter().find(|a| a.starts_with("#[primary_key"));
                            let (sql, optional) = sql_for(ty, db);
                            columns.push(Column {
                                name: column,
                                sql,
                                rust: ty.trim().to_string(),
                                optional,
                                primary_key: pk.is_some(),
                                auto_increment: pk.is_some_and(|a| a.contains("auto_increment")),
                            });
                        }
                    }
                    field_attrs.clear();
                }
                out.push(ModelInfo { type_name: name.to_string(), table, file: file.to_string(), columns });
            }
            _ => {}
        }
        attrs.clear();
    }
    out
}

fn auto_increment_column(db: Db) -> &'static str {
    match db {
        Db::Sqlite => "INTEGER PRIMARY KEY AUTOINCREMENT",
        Db::Postgres => "BIGSERIAL PRIMARY KEY",
        Db::Mysql => "BIGINT AUTO_INCREMENT PRIMARY KEY",
    }
}

/// What the database lacks, as up and down SQL, and the notes.
fn diff(models: &[ModelInfo], existing: &ExistingColumns, db: Db) -> (Vec<String>, Vec<String>, Vec<String>) {
    let (mut up, mut down, mut notes) = (Vec::new(), Vec::new(), Vec::new());
    let lower: BTreeMap<String, Vec<String>> = existing.iter().map(|(t, cs)| (t.to_lowercase(), cs.iter().map(|c| c.to_lowercase()).collect())).collect();
    for model in models {
        let unmapped: Vec<&Column> = model.columns.iter().filter(|c| c.sql.is_none()).collect();
        match lower.get(&model.table.to_lowercase()) {
            None => {
                let mut lines = Vec::new();
                for c in &model.columns {
                    let Some(sql) = &c.sql else { continue };
                    let ty = if c.auto_increment {
                        auto_increment_column(db).to_string()
                    } else if c.primary_key {
                        format!("{sql} PRIMARY KEY")
                    } else {
                        sql.clone()
                    };
                    lines.push(format!("    {} {ty}", c.name));
                }
                up.push(format!("-- {} ({})\nCREATE TABLE {} (\n{}\n);", model.type_name, model.file, model.table, lines.join(",\n")));
                down.push(format!("DROP TABLE {};", model.table));
            }
            Some(columns) => {
                for c in &model.columns {
                    if columns.contains(&c.name.to_lowercase()) {
                        continue;
                    }
                    let Some(sql) = &c.sql else { continue };
                    if c.primary_key {
                        notes.push(format!("{}.{}: the table exists without its primary key column; change the key by hand", model.table, c.name));
                        continue;
                    }
                    // Existing rows need a value for a NOT NULL column.
                    let sql = match (c.optional, default_for(&c.rust, db)) {
                        (false, Some(value)) => format!("{sql} DEFAULT {value}"),
                        (false, None) => {
                            notes.push(format!("{}.{} is NOT NULL with no default: fill existing rows by hand, or make the field an Option", model.table, c.name));
                            sql.clone()
                        }
                        (true, _) => sql.clone(),
                    };
                    up.push(format!("ALTER TABLE {} ADD COLUMN {} {sql};", model.table, c.name));
                    down.push(format!("ALTER TABLE {} DROP COLUMN {};", model.table, c.name));
                }
                let model_columns: Vec<String> = model.columns.iter().map(|c| c.name.to_lowercase()).collect();
                for extra in columns.iter().filter(|c| !model_columns.contains(c)) {
                    notes.push(format!("{}.{} is in the database but not in {}: drop it by hand if it's gone (never generated)", model.table, extra, model.type_name));
                }
            }
        }
        for c in unmapped {
            notes.push(format!("{}.{}: no SQL type for `{}`: add the column by hand", model.table, c.name, c.rust));
        }
    }
    down.reverse();
    (up, down, notes)
}

/// The migration `name` that adds what the models in the project at `dir`
/// have and `existing` (the database's tables and columns) lacks. Reads
/// only; a plan without changes and a note when the database already
/// matches. `db: None` takes the database from the `model-*` feature in
/// `Cargo.toml`, else SQLite.
pub fn plan_model_migration(dir: impl AsRef<Path>, name: &str, existing: &ExistingColumns, db: Option<Db>) -> Result<Plan, GenError> {
    let dir = dir.as_ref();
    if !dir.join("Cargo.toml").is_file() {
        return Err(GenError::refused(format!("{} has no Cargo.toml: run rws gen in the project's root (or pass --dir)", dir.display())));
    }
    plan_with(&Disk(dir.to_path_buf()), name, existing, db)
}

pub(crate) fn plan_with(files: &dyn Files, name: &str, existing: &ExistingColumns, db: Option<Db>) -> Result<Plan, GenError> {
    check_snake(name, "migration")?;
    let manifest = files.manifest()?;
    let db = super::choose_db(&super::cargo(files)?, db)?;
    let command = format!("rws gen migration {name} --from-models");
    let mut b = Builder { files, changes: Vec::new(), notes: Vec::new() };
    let models_dir = manifest.paths.models.clone();
    let mut models = Vec::new();
    let mut names = files.list(&models_dir);
    names.sort();
    for file in names.iter().filter(|n| n.ends_with(".rs")) {
        let path = join(&models_dir, file);
        if let Some(text) = files.read(&path) {
            models.extend(models_in(&text, &path, db));
        }
    }
    if models.is_empty() {
        return Err(GenError::refused(format!("no #[derive(Model)] struct in {models_dir}")));
    }
    let (up, down, notes) = diff(&models, existing, db);
    b.notes.extend(notes);
    if up.is_empty() {
        b.note("the database already has every model's table and columns: no migration");
        return Ok(Plan { command, changes: b.changes, notes: b.notes });
    }
    let dir = manifest.paths.migrations.clone();
    let (up_path, down_path, stem) = migration_paths(files, &dir, name);
    let header = format!("-- {stem}: generated by `{command}` (rws {VERSION}) from the models\n-- and the database it saw ({}). Additive only: review before applying.\n", db.as_str());
    b.create(&up_path, format!("{header}\n{}\n", up.join("\n\n")));
    b.create(&down_path, format!("-- Undoes {stem}.\n{}\n", down.join("\n")));
    Ok(Plan { command, changes: b.changes, notes: b.notes })
}

/// The model scanner on arbitrary text (the generator's fuzz target).
pub(crate) fn fuzz_models(input: &str) {
    let _ = models_in(input, "fuzz.rs", Db::Postgres);
    let _ = attr_value(input, "name");
}

#[cfg(test)]
mod tests;
