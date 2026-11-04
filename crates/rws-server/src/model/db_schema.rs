//! The database's schema as data (18.30.0): [`DbPool::schema`] reads every
//! table of the connected schema — columns (type, nullability, default),
//! primary key, indexes and foreign keys — from the backend's catalog
//! (`sqlite_master` and the `pragma_*` functions on SQLite,
//! `information_schema` and `pg_catalog` on PostgreSQL,
//! `information_schema` on MySQL), and [`DbSchema::to_json`] writes it for
//! tools: `rws db schema --json` and the dev MCP server's `db_schema`.
//!
//! "The connected schema" is SQLite's main database, PostgreSQL's
//! `current_schema()` (the first of the `search_path`), MySQL's
//! `DATABASE()`. Views are left out; rws's `_schema_migrations` table is
//! listed (it is part of the schema).
//!
//! ```rust,no_run
//! # extern crate rws_server as rust_web_server;
//! # #[cfg(feature = "model-sqlite")]
//! # async fn example() -> Result<(), rust_web_server::model::DbError> {
//! use rust_web_server::model::DbPool;
//!
//! let pool = DbPool::memory().await?;
//! pool.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT NOT NULL UNIQUE)", &[]).await?;
//! let schema = pool.schema().await?;
//! let users = schema.table("users").unwrap();
//! assert_eq!(vec!["id".to_string()], users.primary_key);
//! assert!(users.indexes.iter().any(|i| i.unique && i.columns == ["email"]));
//! println!("{}", schema.to_json());
//! # Ok(())
//! # }
//! ```

use super::backend::Backend;
use super::pool::DbPool;
use super::{DbError, ModelRow, Value};

/// Every table of the connected schema, by name.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DbSchema {
    /// The backend read.
    pub backend: Backend,
    /// The tables, sorted by name.
    pub tables: Vec<TableSchema>,
}

/// One table.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TableSchema {
    pub name: String,
    /// In the table's order.
    pub columns: Vec<ColumnSchema>,
    /// The primary key's columns, in key order; empty without one.
    pub primary_key: Vec<String>,
    /// Indexes, by name — the primary key's and those made for unique
    /// constraints included where the backend lists them (SQLite has no
    /// index for an `INTEGER PRIMARY KEY`).
    pub indexes: Vec<IndexSchema>,
    /// Foreign keys, by name (SQLite: in declaration order, unnamed).
    pub foreign_keys: Vec<ForeignKeySchema>,
}

/// One column.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ColumnSchema {
    pub name: String,
    /// The type as the backend spells it (`INTEGER`, `bigint`,
    /// `varchar(255)`, …).
    pub data_type: String,
    pub nullable: bool,
    /// The default's SQL text, `None` without one.
    pub default: Option<String>,
    /// Part of the primary key.
    pub primary_key: bool,
}

/// One index.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct IndexSchema {
    pub name: String,
    /// In index order; expression parts are left out.
    pub columns: Vec<String>,
    pub unique: bool,
    /// The primary key's index.
    pub primary: bool,
}

/// One foreign key.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ForeignKeySchema {
    /// The constraint's name (`None` on SQLite, whose constraints have
    /// none).
    pub name: Option<String>,
    pub columns: Vec<String>,
    pub references_table: String,
    /// The referenced columns, matching `columns` one to one.
    pub references_columns: Vec<String>,
    /// `NO ACTION`, `RESTRICT`, `CASCADE`, `SET NULL` or `SET DEFAULT`.
    pub on_update: String,
    pub on_delete: String,
}

impl DbSchema {
    /// The table named `name` (ignoring ASCII case).
    pub fn table(&self, name: &str) -> Option<&TableSchema> {
        self.tables.iter().find(|t| t.name.eq_ignore_ascii_case(name))
    }

    /// The schema as JSON: `{"backend":…,"tables":[{"name":…,"columns":[{"name":…,"type":…,
    /// "nullable":…,"default":…,"primary_key":…}],"primary_key":[…],"indexes":[{"name":…,
    /// "columns":[…],"unique":…,"primary":…}],"foreign_keys":[{"name":…,"columns":[…],
    /// "references":{"table":…,"columns":[…]},"on_update":…,"on_delete":…}]}]}`.
    pub fn to_json(&self) -> String {
        let tables: Vec<String> = self.tables.iter().map(TableSchema::to_json).collect();
        format!(r#"{{"backend":{},"tables":[{}]}}"#, quoted(self.backend.name()), tables.join(","))
    }
}

impl TableSchema {
    /// The column named `name` (ignoring ASCII case).
    pub fn column(&self, name: &str) -> Option<&ColumnSchema> {
        self.columns.iter().find(|c| c.name.eq_ignore_ascii_case(name))
    }

    fn to_json(&self) -> String {
        let columns: Vec<String> = self
            .columns
            .iter()
            .map(|c| {
                format!(
                    r#"{{"name":{},"type":{},"nullable":{},"default":{},"primary_key":{}}}"#,
                    quoted(&c.name),
                    quoted(&c.data_type),
                    c.nullable,
                    c.default.as_deref().map(quoted).unwrap_or_else(|| "null".to_string()),
                    c.primary_key
                )
            })
            .collect();
        let indexes: Vec<String> = self
            .indexes
            .iter()
            .map(|i| format!(r#"{{"name":{},"columns":{},"unique":{},"primary":{}}}"#, quoted(&i.name), list(&i.columns), i.unique, i.primary))
            .collect();
        let foreign_keys: Vec<String> = self
            .foreign_keys
            .iter()
            .map(|f| {
                format!(
                    r#"{{"name":{},"columns":{},"references":{{"table":{},"columns":{}}},"on_update":{},"on_delete":{}}}"#,
                    f.name.as_deref().map(quoted).unwrap_or_else(|| "null".to_string()),
                    list(&f.columns),
                    quoted(&f.references_table),
                    list(&f.references_columns),
                    quoted(&f.on_update),
                    quoted(&f.on_delete)
                )
            })
            .collect();
        format!(
            r#"{{"name":{},"columns":[{}],"primary_key":{},"indexes":[{}],"foreign_keys":[{}]}}"#,
            quoted(&self.name),
            columns.join(","),
            list(&self.primary_key),
            indexes.join(","),
            foreign_keys.join(",")
        )
    }
}

fn quoted(s: &str) -> String {
    format!("\"{}\"", crate::core::json::escape(s))
}

fn list(items: &[String]) -> String {
    format!("[{}]", items.iter().map(|s| quoted(s)).collect::<Vec<_>>().join(","))
}

/// A column's value as text (`None` for NULL), whatever type the driver
/// decoded it as.
fn text(row: &ModelRow, column: &str) -> Option<String> {
    let value = row.columns.iter().find(|(name, _)| name.eq_ignore_ascii_case(column)).map(|(_, v)| v.clone())?;
    match value {
        Value::Null => None,
        Value::Bool(b) => Some(b.to_string()),
        Value::Int(n) => Some(n.to_string()),
        Value::Float(f) => Some(f.to_string()),
        Value::Text(s) => Some(s),
        Value::Bytes(b) => Some(String::from_utf8_lossy(&b).into_owned()),
    }
}

fn truthy(row: &ModelRow, column: &str) -> bool {
    matches!(text(row, column).as_deref(), Some("1" | "true" | "t" | "YES" | "yes"))
}

fn table(name: String) -> TableSchema {
    TableSchema { name, columns: Vec::new(), primary_key: Vec::new(), indexes: Vec::new(), foreign_keys: Vec::new() }
}

/// Add column `column` of index `index` to `indexes` (a new index when it
/// differs from the last one).
fn push_index(indexes: &mut Vec<IndexSchema>, index: String, unique: bool, primary: bool, column: Option<String>) {
    if indexes.last().map(|i| i.name != index).unwrap_or(true) {
        indexes.push(IndexSchema { name: index, columns: Vec::new(), unique, primary });
    }
    if let (Some(last), Some(column)) = (indexes.last_mut(), column) {
        last.columns.push(column);
    }
}

#[allow(clippy::too_many_arguments)]
fn push_foreign_key(
    keys: &mut Vec<ForeignKeySchema>,
    name: Option<String>,
    group: &str,
    column: String,
    references_table: String,
    references_column: Option<String>,
    on_update: String,
    on_delete: String,
    groups: &mut Vec<String>,
) {
    if groups.last().map(|g| g != group).unwrap_or(true) {
        groups.push(group.to_string());
        keys.push(ForeignKeySchema { name, columns: Vec::new(), references_table, references_columns: Vec::new(), on_update, on_delete });
    }
    if let Some(last) = keys.last_mut() {
        last.columns.push(column);
        if let Some(c) = references_column {
            last.references_columns.push(c);
        }
    }
}

impl DbPool {
    /// The connected schema's tables (18.30.0), read from the primary: see
    /// [`db_schema`](super::db_schema).
    pub async fn schema(&self) -> Result<DbSchema, DbError> {
        let pool = self.primary();
        let mut tables = match pool.backend() {
            #[cfg(feature = "model-sqlite")]
            Backend::Sqlite => sqlite(pool).await?,
            #[cfg(feature = "model-postgres")]
            Backend::Postgres => postgres(pool).await?,
            #[cfg(feature = "model-mysql")]
            Backend::MySql => mysql(pool).await?,
        };
        tables.sort_by(|a, b| a.name.cmp(&b.name));
        for t in &mut tables {
            let primary_key = t.primary_key.clone();
            for c in &mut t.columns {
                c.primary_key = primary_key.iter().any(|k| k.eq_ignore_ascii_case(&c.name));
            }
        }
        // A foreign key naming no columns references the primary key.
        let keys: Vec<(String, Vec<String>)> = tables.iter().map(|t| (t.name.clone(), t.primary_key.clone())).collect();
        for t in &mut tables {
            for fk in &mut t.foreign_keys {
                if fk.references_columns.is_empty() {
                    if let Some((_, pk)) = keys.iter().find(|(name, _)| name.eq_ignore_ascii_case(&fk.references_table)) {
                        fk.references_columns = pk.clone();
                    }
                }
            }
        }
        Ok(DbSchema { backend: pool.backend(), tables })
    }
}

#[cfg(feature = "model-sqlite")]
async fn sqlite(pool: &DbPool) -> Result<Vec<TableSchema>, DbError> {
    let names = pool
        .query_rows("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name", &[])
        .await?;
    let mut tables = Vec::new();
    for row in names {
        let Some(name) = text(&row, "name") else { continue };
        let arg = [Value::Text(name.clone())];
        let mut t = table(name);
        let mut key: Vec<(i64, String)> = Vec::new();
        for row in pool
            .query_rows(r#"SELECT name, type, "notnull" AS not_null, dflt_value, pk FROM pragma_table_info(?) ORDER BY cid"#, &arg)
            .await?
        {
            let column = text(&row, "name").unwrap_or_default();
            if let Some(position) = text(&row, "pk").and_then(|p| p.parse::<i64>().ok()).filter(|p| *p > 0) {
                key.push((position, column.clone()));
            }
            t.columns.push(ColumnSchema {
                name: column,
                data_type: text(&row, "type").unwrap_or_default(),
                nullable: !truthy(&row, "not_null"),
                default: text(&row, "dflt_value"),
                primary_key: false,
            });
        }
        key.sort();
        t.primary_key = key.into_iter().map(|(_, c)| c).collect();
        for row in pool.query_rows(r#"SELECT name, "unique" AS is_unique, origin FROM pragma_index_list(?) ORDER BY name"#, &arg).await? {
            let Some(index) = text(&row, "name") else { continue };
            let (unique, primary) = (truthy(&row, "is_unique"), text(&row, "origin").as_deref() == Some("pk"));
            let columns = pool.query_rows("SELECT name FROM pragma_index_info(?) ORDER BY seqno", &[Value::Text(index.clone())]).await?;
            push_index(&mut t.indexes, index.clone(), unique, primary, None);
            for c in columns {
                push_index(&mut t.indexes, index.clone(), unique, primary, text(&c, "name"));
            }
        }
        let mut groups = Vec::new();
        for row in pool
            .query_rows(
                r#"SELECT id, "table" AS ref_table, "from" AS from_column, "to" AS to_column, on_update, on_delete FROM pragma_foreign_key_list(?) ORDER BY id, seq"#,
                &arg,
            )
            .await?
        {
            push_foreign_key(
                &mut t.foreign_keys,
                None,
                &text(&row, "id").unwrap_or_default(),
                text(&row, "from_column").unwrap_or_default(),
                text(&row, "ref_table").unwrap_or_default(),
                text(&row, "to_column"),
                text(&row, "on_update").unwrap_or_else(|| "NO ACTION".to_string()),
                text(&row, "on_delete").unwrap_or_else(|| "NO ACTION".to_string()),
                &mut groups,
            );
        }
        tables.push(t);
    }
    Ok(tables)
}

/// The tables `table_sql` lists, filled with the columns, indexes and
/// foreign keys the other queries answer (each ordered by table, then the
/// object, then position).
#[cfg(any(feature = "model-postgres", feature = "model-mysql"))]
async fn catalog(pool: &DbPool, table_sql: &str, column_sql: &str, index_sql: &str, key_sql: &str) -> Result<Vec<TableSchema>, DbError> {
    let mut tables: Vec<TableSchema> = pool.query_rows(table_sql, &[]).await?.iter().filter_map(|r| text(r, "table_name")).map(table).collect();
    let find = |tables: &[TableSchema], name: &str| tables.iter().position(|t| t.name == name);
    for row in pool.query_rows(column_sql, &[]).await? {
        let Some(i) = text(&row, "table_name").and_then(|n| find(&tables, &n)) else { continue };
        tables[i].columns.push(ColumnSchema {
            name: text(&row, "column_name").unwrap_or_default(),
            data_type: text(&row, "data_type").unwrap_or_default(),
            nullable: truthy(&row, "is_nullable"),
            default: text(&row, "column_default"),
            primary_key: false,
        });
    }
    for row in pool.query_rows(index_sql, &[]).await? {
        let Some(i) = text(&row, "table_name").and_then(|n| find(&tables, &n)) else { continue };
        let (index, primary) = (text(&row, "index_name").unwrap_or_default(), truthy(&row, "is_primary"));
        let column = text(&row, "column_name");
        if primary {
            tables[i].primary_key.extend(column.clone());
        }
        push_index(&mut tables[i].indexes, index, truthy(&row, "is_unique"), primary, column);
    }
    let mut groups: Vec<Vec<String>> = vec![Vec::new(); tables.len()];
    for row in pool.query_rows(key_sql, &[]).await? {
        let Some(i) = text(&row, "table_name").and_then(|n| find(&tables, &n)) else { continue };
        let name = text(&row, "constraint_name").unwrap_or_default();
        push_foreign_key(
            &mut tables[i].foreign_keys,
            Some(name.clone()),
            &name,
            text(&row, "column_name").unwrap_or_default(),
            text(&row, "ref_table").unwrap_or_default(),
            text(&row, "ref_column"),
            text(&row, "on_update").unwrap_or_else(|| "NO ACTION".to_string()),
            text(&row, "on_delete").unwrap_or_else(|| "NO ACTION".to_string()),
            &mut groups[i],
        );
    }
    Ok(tables)
}

#[cfg(feature = "model-postgres")]
async fn postgres(pool: &DbPool) -> Result<Vec<TableSchema>, DbError> {
    catalog(
        pool,
        "SELECT table_name::text AS table_name FROM information_schema.tables \
         WHERE table_schema = current_schema() AND table_type = 'BASE TABLE' ORDER BY table_name",
        "SELECT table_name::text AS table_name, column_name::text AS column_name, data_type::text AS data_type, \
         is_nullable::text AS is_nullable, column_default::text AS column_default \
         FROM information_schema.columns WHERE table_schema = current_schema() ORDER BY table_name, ordinal_position",
        "SELECT t.relname::text AS table_name, i.relname::text AS index_name, ix.indisunique AS is_unique, \
         ix.indisprimary AS is_primary, a.attname::text AS column_name \
         FROM pg_index ix \
         JOIN pg_class t ON t.oid = ix.indrelid \
         JOIN pg_class i ON i.oid = ix.indexrelid \
         JOIN pg_namespace ns ON ns.oid = t.relnamespace \
         CROSS JOIN LATERAL unnest(ix.indkey::int2[]) WITH ORDINALITY AS k(attnum, ord) \
         JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = k.attnum \
         WHERE ns.nspname = current_schema() \
         ORDER BY t.relname, i.relname, k.ord",
        "SELECT t.relname::text AS table_name, c.conname::text AS constraint_name, a.attname::text AS column_name, \
         rt.relname::text AS ref_table, ra.attname::text AS ref_column, \
         CASE c.confupdtype WHEN 'r' THEN 'RESTRICT' WHEN 'c' THEN 'CASCADE' WHEN 'n' THEN 'SET NULL' WHEN 'd' THEN 'SET DEFAULT' ELSE 'NO ACTION' END AS on_update, \
         CASE c.confdeltype WHEN 'r' THEN 'RESTRICT' WHEN 'c' THEN 'CASCADE' WHEN 'n' THEN 'SET NULL' WHEN 'd' THEN 'SET DEFAULT' ELSE 'NO ACTION' END AS on_delete \
         FROM pg_constraint c \
         JOIN pg_class t ON t.oid = c.conrelid \
         JOIN pg_class rt ON rt.oid = c.confrelid \
         JOIN pg_namespace ns ON ns.oid = t.relnamespace \
         CROSS JOIN LATERAL unnest(c.conkey, c.confkey) WITH ORDINALITY AS k(attnum, refnum, ord) \
         JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = k.attnum \
         JOIN pg_attribute ra ON ra.attrelid = c.confrelid AND ra.attnum = k.refnum \
         WHERE c.contype = 'f' AND ns.nspname = current_schema() \
         ORDER BY t.relname, c.conname, k.ord",
    )
    .await
}

#[cfg(feature = "model-mysql")]
async fn mysql(pool: &DbPool) -> Result<Vec<TableSchema>, DbError> {
    catalog(
        pool,
        "SELECT CAST(TABLE_NAME AS CHAR) AS table_name FROM information_schema.TABLES \
         WHERE TABLE_SCHEMA = DATABASE() AND TABLE_TYPE = 'BASE TABLE' ORDER BY TABLE_NAME",
        "SELECT CAST(TABLE_NAME AS CHAR) AS table_name, CAST(COLUMN_NAME AS CHAR) AS column_name, \
         CAST(COLUMN_TYPE AS CHAR) AS data_type, CAST(IS_NULLABLE AS CHAR) AS is_nullable, \
         CAST(COLUMN_DEFAULT AS CHAR) AS column_default \
         FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = DATABASE() ORDER BY TABLE_NAME, ORDINAL_POSITION",
        "SELECT CAST(TABLE_NAME AS CHAR) AS table_name, CAST(INDEX_NAME AS CHAR) AS index_name, \
         CAST(NON_UNIQUE = 0 AS SIGNED) AS is_unique, CAST(INDEX_NAME = 'PRIMARY' AS SIGNED) AS is_primary, \
         CAST(COLUMN_NAME AS CHAR) AS column_name \
         FROM information_schema.STATISTICS WHERE TABLE_SCHEMA = DATABASE() \
         ORDER BY TABLE_NAME, INDEX_NAME = 'PRIMARY' DESC, INDEX_NAME, SEQ_IN_INDEX",
        "SELECT CAST(k.TABLE_NAME AS CHAR) AS table_name, CAST(k.CONSTRAINT_NAME AS CHAR) AS constraint_name, \
         CAST(k.COLUMN_NAME AS CHAR) AS column_name, CAST(k.REFERENCED_TABLE_NAME AS CHAR) AS ref_table, \
         CAST(k.REFERENCED_COLUMN_NAME AS CHAR) AS ref_column, CAST(r.UPDATE_RULE AS CHAR) AS on_update, \
         CAST(r.DELETE_RULE AS CHAR) AS on_delete \
         FROM information_schema.KEY_COLUMN_USAGE k \
         JOIN information_schema.REFERENTIAL_CONSTRAINTS r \
           ON r.CONSTRAINT_SCHEMA = k.CONSTRAINT_SCHEMA AND r.CONSTRAINT_NAME = k.CONSTRAINT_NAME AND r.TABLE_NAME = k.TABLE_NAME \
         WHERE k.TABLE_SCHEMA = DATABASE() AND k.REFERENCED_TABLE_NAME IS NOT NULL \
         ORDER BY k.TABLE_NAME, k.CONSTRAINT_NAME, k.ORDINAL_POSITION",
    )
    .await
}
