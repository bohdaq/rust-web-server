//! `models`: the `#[derive(Model)]` types registered with
//! [`super::DevTools::model`] — table, columns, primary key — and, with a
//! database, how each compares with its table.

use crate::mcp::{McpContent, McpServer};
use crate::model::{DbPool, ModelInfo};

use super::db::{self, Column};
use super::{quoted, read_only_hint};

pub(super) fn install(mcp: McpServer, models: Vec<ModelInfo>, database: Option<DbPool>) -> McpServer {
    mcp.tool(
        "models",
        "The app's models (#[derive(Model)] types registered with DevTools::model): type, table, columns and primary key; with a database also each column's type there, whether the table exists, model columns missing from it and table columns no model field maps",
        r#"{"type":"object"}"#,
        move |_| match &database {
            None => Ok(McpContent::json(models_json(&models, None))),
            Some(pool) => {
                let (pool, models) = (pool.clone(), models.clone());
                db::run(async move {
                    let columns = db::columns(&pool).await?;
                    Ok(models_json(&models, Some(&columns)))
                })
                .map(McpContent::json)
            }
        },
    )
    .tool_annotations("models", read_only_hint())
}

/// What `models` answers; `columns` is the database's, when there is one.
pub(crate) fn models_json(models: &[ModelInfo], columns: Option<&[Column]>) -> String {
    let entries: Vec<String> = models.iter().map(|m| model_json(m, columns)).collect();
    let hint = if models.is_empty() {
        quoted("no models registered: add DevTools::new().model::<YourModel>() for each #[derive(Model)] type")
    } else {
        "null".to_string()
    };
    format!(r#"{{"database":{},"models":[{}],"hint":{}}}"#, columns.is_some(), entries.join(","), hint)
}

fn model_json(model: &ModelInfo, columns: Option<&[Column]>) -> String {
    let table: Option<Vec<&Column>> =
        columns.map(|all| all.iter().filter(|c| c.table.eq_ignore_ascii_case(model.table)).collect());
    let mapped: Vec<String> = model
        .columns
        .iter()
        .map(|name| {
            let db = match &table {
                None => String::new(),
                Some(cols) => match cols.iter().find(|c| c.name.eq_ignore_ascii_case(name)) {
                    Some(c) => format!(
                        r#","db":{{"type":{},"nullable":{},"primary_key":{}}}"#,
                        quoted(&c.data_type),
                        c.nullable,
                        c.primary_key
                    ),
                    None => r#","db":null"#.to_string(),
                },
            };
            format!(r#"{{"name":{},"primary_key":{}{}}}"#, quoted(name), *name == model.primary_key, db)
        })
        .collect();
    let comparison = match &table {
        None => String::new(),
        Some(cols) => {
            let missing: Vec<String> = model
                .columns
                .iter()
                .filter(|name| !cols.iter().any(|c| c.name.eq_ignore_ascii_case(name)))
                .map(|name| quoted(name))
                .collect();
            let unmapped: Vec<String> = cols
                .iter()
                .filter(|c| !model.columns.iter().any(|name| c.name.eq_ignore_ascii_case(name)))
                .map(|c| quoted(&c.name))
                .collect();
            format!(
                r#","table_exists":{},"missing_columns":[{}],"unmapped_columns":[{}]"#,
                !cols.is_empty(),
                missing.join(","),
                unmapped.join(",")
            )
        }
    };
    format!(
        r#"{{"type":{},"name":{},"table":{},"primary_key":{},"auto_increment":{},"columns":[{}]{}}}"#,
        quoted(model.type_name),
        quoted(model.short_name()),
        quoted(model.table),
        quoted(model.primary_key),
        model.auto_increment,
        mapped.join(","),
        comparison
    )
}
