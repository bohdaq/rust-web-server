//! `rws gen …`: the command line of the generators.

use std::io::Write;
use std::path::PathBuf;

use super::names::parse_fields;
use super::{plan, Db, GenError, Generate, RouteAction};

/// Written (or nothing to write: every file already as generated).
pub const EXIT_OK: i32 = 0;
/// A conflict (a file exists with other content) or the project refused
/// (no `app()`, no route module, …): nothing written.
pub const EXIT_CONFLICT: i32 = 1;
/// Bad arguments.
pub const EXIT_USAGE: i32 = 2;

const USAGE: &str = "\
Usage: rws gen <generator> <name> [args] [--dir <project>] [--dry-run] [--json]

Generators:
  model <Name> field:type...      src/models/<name>.rs (#[derive(Model)], id auto-increment)
        [--db sqlite|postgres|mysql]  and migrations/NNNN_create_<table>.sql + .down.sql
        [--timestamps] [--soft-delete]  created_at/updated_at, deleted_at kept by save/delete
  route <resource> [field:type...]  src/routes/<resource>.rs: typed handlers and register(app)
        [--methods list,show,create,update,remove] [--prefix /api]
  migration <name>                migrations/NNNN_<name>.sql + .down.sql to fill in
  test <resource> [--app-expr 'app(...)']  tests/<resource>.rs: TestClient against app()

Field types: i64, i32, f64, bool, String, Option<T> (T one of those).
Paths come from rws.app.toml (the conventional layout without one). --db
defaults to the model-* feature in Cargo.toml, else sqlite.

Nothing is overwritten: a file that exists with other content is a conflict,
shown as a diff, and nothing is written. Running a generator twice changes
nothing the second time. --dry-run prints the plan and the diffs without
writing; --json prints it for agents.
Exit codes: 0 written (or already as generated), 1 conflict or refused, 2 usage.
";

/// Run `rws gen <args>` (the arguments after `gen`), writing to `out` and
/// `err`; answers the exit code ([`EXIT_OK`], [`EXIT_CONFLICT`],
/// [`EXIT_USAGE`]).
pub fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let parsed = match parse(args) {
        Ok(Parsed::Help) => {
            let _ = write!(out, "{USAGE}");
            return EXIT_OK;
        }
        Ok(Parsed::Run(cli)) => cli,
        Err(e) => return fail(&e, args.iter().any(|a| a == "--json"), out, err),
    };
    let plan = match plan(&parsed.dir, &parsed.request) {
        Ok(plan) => plan,
        Err(e) => return fail(&e, parsed.json, out, err),
    };
    if plan.has_conflicts() || parsed.dry_run || plan.is_unchanged() {
        if parsed.json {
            let _ = writeln!(out, "{}", plan.to_json(false));
        } else {
            let _ = write!(out, "{}", plan.render(parsed.dry_run));
            if plan.has_conflicts() {
                let _ = writeln!(err, "rws gen: not written — a file above exists with other content (the diff is what the generator would write). Move it aside or merge by hand.");
            } else if plan.is_unchanged() {
                let _ = writeln!(out, "nothing to do: every file is already as generated");
            }
        }
        return if plan.has_conflicts() { EXIT_CONFLICT } else { EXIT_OK };
    }
    match plan.apply(&parsed.dir) {
        Ok(_) => {
            if parsed.json {
                let _ = writeln!(out, "{}", plan.to_json(true));
            } else {
                let _ = write!(out, "{}", plan.render(false));
            }
            EXIT_OK
        }
        Err(e) => fail(&e, parsed.json, out, err),
    }
}

fn fail(e: &GenError, json: bool, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    if json {
        let _ = writeln!(out, "{}", e.to_json());
    } else if e.usage {
        let _ = writeln!(err, "rws gen: {}\n(rws gen --help lists the generators and their arguments)", e.message);
    } else {
        let _ = writeln!(err, "rws gen: {}", e.message);
    }
    if e.usage {
        EXIT_USAGE
    } else {
        EXIT_CONFLICT
    }
}

struct Cli {
    request: Generate,
    dir: PathBuf,
    dry_run: bool,
    json: bool,
}

enum Parsed {
    Help,
    Run(Cli),
}

fn parse(args: &[String]) -> Result<Parsed, GenError> {
    let mut positional: Vec<String> = Vec::new();
    let mut dir = PathBuf::from(".");
    let (mut dry_run, mut json) = (false, false);
    let (mut db, mut methods, mut prefix, mut app_expr) = (None, None, None, None);
    let (mut timestamps, mut soft_delete) = (false, false);
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        i += 1;
        if arg == "-h" || arg == "--help" || (arg == "help" && positional.is_empty()) {
            return Ok(Parsed::Help);
        }
        if !arg.starts_with("--") {
            positional.push(arg.to_string());
            continue;
        }
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) => (f, Some(v.to_string())),
            None => (arg, None),
        };
        let mut value = |name: &str| -> Result<String, GenError> {
            if let Some(v) = inline.clone() {
                return Ok(v);
            }
            let v = args.get(i).cloned().ok_or_else(|| GenError::usage(format!("{name} needs a value")))?;
            i += 1;
            Ok(v)
        };
        match flag {
            "--dry-run" => dry_run = true,
            "--timestamps" => timestamps = true,
            "--soft-delete" => soft_delete = true,
            "--json" => json = true,
            "--dir" => dir = PathBuf::from(value("--dir")?),
            "--db" => {
                let v = value("--db")?;
                db = Some(Db::parse(&v).ok_or_else(|| GenError::usage(format!("--db {v}: use sqlite, postgres or mysql")))?);
            }
            "--methods" => methods = Some(value("--methods")?),
            "--prefix" => prefix = Some(value("--prefix")?),
            "--app-expr" => app_expr = Some(value("--app-expr")?),
            other => return Err(GenError::usage(format!("unknown option {other}"))),
        }
    }
    let Some(kind) = positional.first().cloned() else {
        return Err(GenError::usage("name a generator: model, route, migration or test"));
    };
    let name = positional.get(1).cloned();
    let rest = positional.get(2..).unwrap_or(&[]);
    let only = |flag: &str, set: bool, kinds: &str| -> Result<(), GenError> {
        if set {
            Err(GenError::usage(format!("{flag} applies to rws gen {kinds} only")))
        } else {
            Ok(())
        }
    };
    let need_name = |what: &str| name.clone().ok_or_else(|| GenError::usage(format!("rws gen {kind}: give the {what}")));
    let request = match kind.as_str() {
        "model" => {
            only("--methods", methods.is_some(), "route")?;
            only("--prefix", prefix.is_some(), "route")?;
            only("--app-expr", app_expr.is_some(), "test")?;
            Generate::Model { name: need_name("model's name (e.g. Post)")?, fields: parse_fields(rest)?, db, timestamps, soft_delete }
        }
        "route" => {
            only("--db", db.is_some(), "model")?;
            only("--timestamps", timestamps, "model")?;
            only("--soft-delete", soft_delete, "model")?;
            only("--app-expr", app_expr.is_some(), "test")?;
            let actions = match &methods {
                Some(m) => RouteAction::parse_list(m)?,
                None => RouteAction::ALL.to_vec(),
            };
            Generate::Route { resource: need_name("resource (e.g. posts)")?, fields: parse_fields(rest)?, actions, prefix: prefix.unwrap_or_default() }
        }
        "migration" | "test" => {
            only("--db", db.is_some(), "model")?;
            only("--timestamps", timestamps, "model")?;
            only("--soft-delete", soft_delete, "model")?;
            only("--methods", methods.is_some(), "route")?;
            only("--prefix", prefix.is_some(), "route")?;
            if let Some(extra) = rest.first() {
                return Err(GenError::usage(format!("rws gen {kind} takes one name; `{extra}` is extra")));
            }
            if kind == "migration" {
                only("--app-expr", app_expr.is_some(), "test")?;
                Generate::Migration { name: need_name("migration's name (e.g. add_index_to_posts)")? }
            } else {
                Generate::Test { resource: need_name("resource (e.g. posts)")?, app_expr }
            }
        }
        other => return Err(GenError::usage(format!("unknown generator `{other}`: use model, route, migration or test"))),
    };
    Ok(Parsed::Run(Cli { request, dir, dry_run, json }))
}

/// `rws gen …` asked for: the arguments after `gen`.
pub fn requested(args: &[String]) -> Option<&[String]> {
    (args.get(1).map(String::as_str) == Some("gen")).then(|| &args[2..])
}
