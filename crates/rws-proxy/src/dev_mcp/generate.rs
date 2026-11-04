//! `gen_model`, `gen_route`, `gen_migration`, `gen_test`: the generators
//! ([`crate::generator`]) as tools. They write into the project, so they
//! are listed only when the tools may change things (not
//! [`super::DevTools::read_only`]); `dry_run: true` answers the plan and
//! its diffs without writing. The answer is the plan as JSON
//! ([`crate::generator::Plan::to_json`]); a conflict or a refusal is a
//! tool error carrying the same JSON.

use std::path::PathBuf;

use crate::generator::{plan, Db, Field, GenError, Generate, RouteAction};
use crate::mcp::json_rpc;
use crate::mcp::{extract_arg, McpContent, McpServer};

/// The tools this module adds.
pub const GEN_TOOLS: [&str; 4] = ["gen_model", "gen_route", "gen_migration", "gen_test"];

const DRY_RUN: &str = r#""dry_run":{"type":"boolean","description":"Answer the plan and diffs without writing anything"}"#;
const FIELDS: &str = r#""fields":{"type":"array","items":{"type":"string"},"description":"name:type, the type one of i64, i32, f64, bool, String, Option<T>"}"#;

pub(super) fn install(mcp: McpServer, dir: PathBuf) -> McpServer {
    let (model_dir, route_dir, migration_dir, test_dir) = (dir.clone(), dir.clone(), dir.clone(), dir);
    mcp.tool(
        "gen_model",
        "Generate a #[derive(Model)] type in src/models/ (id auto-increment) and the migration creating its table, as `rws gen model` does: never overwrites; a conflict answers the diff",
        &format!(
            r#"{{"type":"object","properties":{{"name":{{"type":"string","description":"The model, e.g. Post"}},{FIELDS},"db":{{"type":"string","enum":["sqlite","postgres","mysql"],"description":"Default: the model-* feature in Cargo.toml, else sqlite"}},"timestamps":{{"type":"boolean","description":"Add #[timestamps]: created_at and updated_at, set by save"}},"soft_delete":{{"type":"boolean","description":"Add #[soft_delete]: deleted_at, set by delete; queries skip deleted rows"}},{DRY_RUN}}},"required":["name","fields"]}}"#
        ),
        move |args| {
            let request = (|| {
                let db = match extract_arg(args, "db") {
                    Some(v) => Some(Db::parse(&v).ok_or_else(|| usage(format!("db `{v}`: use sqlite, postgres or mysql")))?),
                    None => None,
                };
                let flag = |name: &str| extract_arg(args, name).is_some_and(|v| v == "true");
                Ok(Generate::Model { name: required(args, "name")?, fields: fields(args)?, db, timestamps: flag("timestamps"), soft_delete: flag("soft_delete") })
            })();
            answer(&model_dir, args, request)
        },
    )
    .tool(
        "gen_route",
        "Generate typed handlers for a resource in src/routes/<resource>.rs with register(app), and register them in app(), as `rws gen route` does",
        &format!(
            r#"{{"type":"object","properties":{{"resource":{{"type":"string","description":"The resource, snake_case plural, e.g. posts"}},{FIELDS},"methods":{{"type":"array","items":{{"type":"string","enum":["list","show","create","update","remove"]}},"description":"Default: all five"}},"prefix":{{"type":"string","description":"A path the routes go under, e.g. /api"}},{DRY_RUN}}},"required":["resource"]}}"#
        ),
        move |args| {
            let request = (|| {
                let actions = match list(args, "methods") {
                    Some(methods) => RouteAction::parse_list(&methods.join(","))?,
                    None => RouteAction::ALL.to_vec(),
                };
                Ok(Generate::Route { resource: required(args, "resource")?, fields: fields(args)?, actions, prefix: extract_arg(args, "prefix").unwrap_or_default() })
            })();
            answer(&route_dir, args, request)
        },
    )
    .tool(
        "gen_migration",
        "Generate an empty migration pair, migrations/NNNN_<name>.sql and .down.sql, numbered after the existing ones",
        &format!(r#"{{"type":"object","properties":{{"name":{{"type":"string","description":"snake_case, e.g. add_index_to_posts"}},{DRY_RUN}}},"required":["name"]}}"#),
        move |args| {
            let request = required(args, "name").map(|name| Generate::Migration { name });
            answer(&migration_dir, args, request)
        },
    )
    .tool(
        "gen_test",
        "Generate tests/<resource>.rs: TestClient tests against app() for the routes rws gen route wrote in src/routes/<resource>.rs",
        &format!(
            r#"{{"type":"object","properties":{{"resource":{{"type":"string"}},"app_expr":{{"type":"string","description":"How a test builds the app when app() takes arguments, e.g. app(Store::memory())"}},{DRY_RUN}}},"required":["resource"]}}"#
        ),
        move |args| {
            let request = required(args, "resource").map(|resource| Generate::Test { resource, app_expr: extract_arg(args, "app_expr") });
            answer(&test_dir, args, request)
        },
    )
    .tool_annotations("gen_model", super::ops::changes(false))
    .tool_annotations("gen_route", super::ops::changes(false))
    .tool_annotations("gen_migration", super::ops::changes(false))
    .tool_annotations("gen_test", super::ops::changes(false))
}

fn usage(message: String) -> GenError {
    GenError { usage: true, message }
}

fn required(args: &str, name: &str) -> Result<String, GenError> {
    extract_arg(args, name).filter(|v| !v.trim().is_empty()).ok_or_else(|| usage(format!("`{name}` is required")))
}

/// A string-array argument (the schema has the server refuse anything
/// else before the tool runs).
fn list(args: &str, name: &str) -> Option<Vec<String>> {
    let members = json_rpc::object_members(args)?;
    let (_, raw) = members.into_iter().find(|(key, _)| key == name)?;
    let raw = raw.trim();
    raw.starts_with('[').then(|| {
        json_rpc::split_array_elements(raw).iter().filter_map(|e| json_rpc::extract_str(&format!(r#"{{"v":{e}}}"#), "v")).collect()
    })
}

fn fields(args: &str) -> Result<Vec<Field>, GenError> {
    crate::generator::parse_fields(&list(args, "fields").unwrap_or_default())
}

fn answer(dir: &std::path::Path, args: &str, request: Result<Generate, GenError>) -> Result<McpContent, String> {
    let dry_run = extract_arg(args, "dry_run").is_some_and(|v| v == "true");
    let plan = request.and_then(|r| plan(dir, &r)).map_err(|e| e.to_json())?;
    if plan.has_conflicts() {
        return Err(plan.to_json(false));
    }
    if dry_run || plan.is_unchanged() {
        return Ok(McpContent::json(plan.to_json(false)));
    }
    plan.apply(dir).map_err(|e| e.to_json())?;
    Ok(McpContent::json(plan.to_json(true)))
}
