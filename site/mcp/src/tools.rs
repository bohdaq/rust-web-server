//! The tools, resources and prompt. Every tool is read-only; results are
//! Markdown (what an agent reads best), with links to the page on rws8.tech.

use std::sync::Arc;

use rust_web_server::mcp::{McpContent, McpServer, PromptArgDef, PromptMessage, ToolAnnotations};
use serde::Deserialize;

use crate::index::{excerpt, Docs, Release};

/// Largest `rws.config.toml` check_config accepts.
pub const MAX_CONFIG_BYTES: usize = 64 * 1024;

const VERSION: &str = r#""version":{"type":"string","description":"rws release the project uses, e.g. \"18.7\" or \"18.7.2\"; default: the latest"}"#;

fn read_only() -> ToolAnnotations {
    ToolAnnotations { read_only_hint: Some(true), destructive_hint: Some(false), idempotent_hint: Some(true), open_world_hint: Some(false) }
}

fn args<T: for<'de> Deserialize<'de>>(raw: &str) -> Result<T, String> {
    let raw = if raw.trim().is_empty() { "{}" } else { raw };
    serde_json::from_str(raw).map_err(|e| format!("Invalid arguments: {e}"))
}

#[derive(Deserialize)]
struct VersionArg {
    version: Option<String>,
}

#[derive(Deserialize)]
struct SearchArgs {
    query: String,
    version: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct PathArgs {
    path: String,
    version: Option<String>,
}

#[derive(Deserialize)]
struct NameArgs {
    name: String,
    version: Option<String>,
}

#[derive(Deserialize)]
struct ChangesArgs {
    since: String,
    until: Option<String>,
}

#[derive(Deserialize)]
struct ConfigArgs {
    toml: String,
}

fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let mut parts = v.trim().trim_start_matches('v').split('.').map(|p| p.parse::<u64>().ok());
    Some((parts.next()??, parts.next().flatten().unwrap_or(0), parts.next().flatten().unwrap_or(0)))
}

/// A page path as agents may write it: "/features/acme/", "features/acme",
/// "docs/18.7/features/acme", or the full URL.
fn normalize_path(path: &str) -> String {
    let p = path.trim();
    let p = p.split_once("rws8.tech").map_or(p, |(_, rest)| rest);
    let p = p.split('#').next().unwrap_or("").trim_matches('/');
    let p = p.strip_prefix("docs/").unwrap_or(p);
    let p = match p.split_once('/') {
        Some((first, rest)) if parse_version(first).is_some() && first.contains('.') => rest,
        _ => p,
    };
    p.trim_end_matches(".md").to_string()
}

pub fn search(docs: &Docs, raw: &str) -> Result<McpContent, String> {
    let a: SearchArgs = args(raw)?;
    let (v, search) = docs.version(a.version.as_deref())?;
    let limit = a.limit.unwrap_or(8).clamp(1, 20);
    let hits = search.query(&a.query, limit);
    if hits.is_empty() {
        return Ok(McpContent::text(format!(
            "No sections of the rws {} docs match \"{}\". Try other words, or `get_page` with a path from `versions`.",
            v.version, a.query
        )));
    }
    let mut out = format!("rws {} docs, best matches for \"{}\":\n", v.version, a.query);
    for h in hits {
        let page = &v.pages[h.page];
        let section = &page.sections[h.section];
        let (title, anchor) = if section.heading.is_empty() {
            (page.title.clone(), String::new())
        } else {
            (format!("{} › {}", page.title, section.heading), format!("#{}", section.id))
        };
        out.push_str(&format!("\n## {title}\npath: `{}` · {}{anchor}\n\n{}\n", page.path, page.url, excerpt(&section.text, 400)));
    }
    Ok(McpContent::text(out))
}

pub fn get_page(docs: &Docs, raw: &str) -> Result<McpContent, String> {
    let a: PathArgs = args(raw)?;
    let (v, _) = docs.version(a.version.as_deref())?;
    let path = normalize_path(&a.path);
    let page = v.pages.iter().find(|p| p.path == path).ok_or_else(|| {
        let close: Vec<&str> = v.pages.iter().filter(|p| p.path.contains(path.rsplit('/').next().unwrap_or(&path))).map(|p| p.path.as_str()).take(5).collect();
        if close.is_empty() {
            format!("No page `{path}` in the rws {} docs; `search_docs` finds pages by topic.", v.version)
        } else {
            format!("No page `{path}` in the rws {} docs. Did you mean: {}?", v.version, close.join(", "))
        }
    })?;
    Ok(McpContent::text(format!("# {}\n\n{}\n\nrws {} · {}\n\n{}", page.title, page.description, v.version, page.url, page.markdown)))
}

pub fn setting(docs: &Docs, raw: &str) -> Result<McpContent, String> {
    let a: NameArgs = args(raw)?;
    let (v, _) = docs.version(a.version.as_deref())?;
    let wanted = a.name.trim().trim_matches('`').trim_start_matches("--").replace('-', "_").to_ascii_lowercase();
    // "[acme] domains", "acme.domains" and "acme_domains" are one key.
    let wanted = wanted.replace(['[', ']'], "").split(|c: char| c == '.' || c.is_whitespace()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join("_");
    let wanted = wanted.strip_prefix("rws_config_").unwrap_or(&wanted).to_string();
    let exact: Vec<_> = v.settings.iter().filter(|s| s.key == wanted).collect();
    let found: Vec<_> = if exact.is_empty() { v.settings.iter().filter(|s| s.key.contains(&wanted)).take(8).collect() } else { exact };
    if found.is_empty() {
        return Err(format!("No setting like `{}` in rws {}; `search_docs` covers config sections.", a.name, v.version));
    }
    let mut out = String::new();
    for s in found {
        out.push_str(&format!(
            "## {}\nfile key: `{}` (a `[table] key` maps to `<table>_<key>`) · default: {} · type: {}\n\n{}\n\n",
            s.env,
            s.key,
            if s.default.is_empty() { "none".to_string() } else { format!("`{}`", s.default) },
            if s.kind.is_empty() { "—" } else { &s.kind },
            s.description
        ));
    }
    out.push_str(&format!("rws {} · https://rws8.tech/docs/configuration/env-vars/", v.version));
    Ok(McpContent::text(out))
}

pub fn api_item(docs: &Docs, raw: &str) -> Result<McpContent, String> {
    let a: NameArgs = args(raw)?;
    let (v, _) = docs.version(a.version.as_deref())?;
    let wanted = a.name.trim().trim_matches('`').rsplit("::").next().unwrap_or("").to_ascii_lowercase();
    let matches = |name: &str| name.split([',', '/', ' ']).any(|n| n.trim().trim_matches('`').to_ascii_lowercase() == wanted);
    let exact: Vec<_> = v.api.iter().filter(|i| matches(&i.name)).collect();
    let found: Vec<_> = if exact.is_empty() { v.api.iter().filter(|i| i.name.to_ascii_lowercase().contains(&wanted)).take(8).collect() } else { exact };
    if found.is_empty() {
        return Err(format!("No API item like `{}` in rws {}'s reference; `search_docs` searches the guides.", a.name, v.version));
    }
    let mut out = String::new();
    for i in found {
        let first = i.name.split([',', '/', ' ']).next().unwrap_or(&i.name).trim();
        out.push_str(&format!(
            "## {} ({})\nmodule: `{}` · https://docs.rs/rust-web-server/{}/rust_web_server/?search={first}\n\n{}\n\n",
            i.name, i.group, i.module, v.tag, i.description
        ));
    }
    Ok(McpContent::text(out.trim_end().to_string()))
}

pub fn get_example(docs: &Docs, raw: &str) -> Result<McpContent, String> {
    let a: NameArgs = args(raw)?;
    let (v, _) = docs.version(a.version.as_deref())?;
    let wanted = a.name.trim().trim_start_matches("examples/").trim_end_matches(".rs").to_string();
    let Some(e) = v.examples.iter().find(|e| e.name == wanted) else {
        let names: Vec<&str> = v.examples.iter().map(|e| e.name.as_str()).collect();
        return Err(format!("No example `{wanted}` in rws {}. Examples: {}.", v.version, names.join(", ")));
    };
    Ok(McpContent::text(format!(
        "# {}\n\n{}\n\nRun it from a clone of the {} tag: `cargo run --example {}` (features: see the file's header).\n\n```rust\n{}\n```",
        e.file, e.description, v.tag, e.name, e.source.trim_end()
    )))
}

pub fn changes(docs: &Docs, raw: &str) -> Result<McpContent, String> {
    let a: ChangesArgs = args(raw)?;
    let since = parse_version(&a.since).ok_or_else(|| format!("`since` = {:?} is not a version like 18.4.0", a.since))?;
    let until = match a.until.as_deref() {
        Some(u) => parse_version(u).ok_or_else(|| format!("`until` = {u:?} is not a version like 18.8.3"))?,
        None => (u64::MAX, 0, 0),
    };
    let picked: Vec<&Release> = docs
        .changelog
        .iter()
        .filter(|r| parse_version(&r.version).is_some_and(|v| v > since && v <= until))
        .collect();
    if picked.is_empty() {
        return Ok(McpContent::text(format!("No releases after {} in the changelog.", a.since)));
    }
    // Breaking changes first: they are what an upgrade has to act on.
    let mut breaking = String::new();
    for r in &picked {
        for part in r.markdown.split("\n### ") {
            let part = part.trim_start_matches("### ");
            if part.to_ascii_lowercase().starts_with("breaking") {
                breaking.push_str(&format!("\n#### {}\n{}\n", r.version, part.split_once('\n').map_or("", |(_, b)| b).trim()));
            }
        }
    }
    let mut out = format!("rws changes after {} ({} releases).\n", a.since, picked.len());
    if !breaking.is_empty() {
        out.push_str(&format!("\n## Breaking changes\n{breaking}\nMigration guides: https://rws8.tech/docs/reference/migrating-18/\n"));
    }
    for r in picked {
        out.push_str(&format!("\n## {}{}\n\n{}\n", r.version, if r.title.is_empty() { String::new() } else { format!(" — {}", r.title) }, r.markdown));
    }
    Ok(McpContent::text(out))
}

pub fn check_config(raw: &str) -> Result<McpContent, String> {
    let a: ConfigArgs = args(raw)?;
    if a.toml.len() > MAX_CONFIG_BYTES {
        return Err(format!("The file is {} bytes; check_config takes up to {MAX_CONFIG_BYTES}.", a.toml.len()));
    }
    let report = rust_web_server::config_check::check_str_offline(&a.toml);
    let mut out = format!(
        "{}: {} error(s), {} warning(s).\n",
        if report.is_ok() { "OK" } else { "Not OK" },
        report.errors.len(),
        report.warnings.len()
    );
    for e in &report.errors {
        out.push_str(&format!("\nerror: {e}"));
    }
    for w in &report.warnings {
        out.push_str(&format!("\nwarning: {w}"));
    }
    out.push_str(&format!(
        "\n\nChecked with rws {}'s `rws --check`, offline: file paths (certificates, htpasswd, static roots) and secrets in the environment aren't checked here. Run `rws --check` where the server runs for those.",
        env!("RWS_VERSION")
    ));
    Ok(McpContent::text(out))
}

pub fn versions(docs: &Docs) -> String {
    let mut out = format!("Docs for rws {} (latest is {}).\n", docs.order.join(", "), docs.latest);
    if let Ok((v, _)) = docs.version(None) {
        out.push_str(&format!("\nPages in {} (use the path with `get_page`):\n", v.version));
        let mut group = "";
        for p in &v.pages {
            if p.group != group {
                group = &p.group;
                out.push_str(&format!("\n{group}\n"));
            }
            out.push_str(&format!("- `{}`: {}\n", p.path, p.title));
        }
    }
    out
}

pub fn server(docs: Arc<Docs>) -> McpServer {
    let d = |f: fn(&Docs, &str) -> Result<McpContent, String>| {
        let docs = docs.clone();
        move |raw: &str| f(&docs, raw)
    };
    let versions_docs = docs.clone();
    let resource_docs = docs.clone();
    let prompt_docs = docs.clone();
    McpServer::new("rws-docs", env!("CARGO_PKG_VERSION"))
        .tool_annotated(
            "versions",
            "The rws releases these docs cover, the latest one, and every page path of the latest docs.",
            r#"{"type":"object","properties":{}}"#,
            read_only(),
            move |_| Ok(McpContent::text(versions(&versions_docs))),
        )
        .tool_annotated(
            "search_docs",
            "Search the rws docs (configuration, proxy, building apps with the rust-web-server crate, MCP, database, deployment). Returns the best-matching sections with their page path and link.",
            &format!(r#"{{"type":"object","properties":{{"query":{{"type":"string","description":"what to look for, e.g. \"route with a path parameter\""}},{VERSION},"limit":{{"type":"integer","minimum":1,"maximum":20,"description":"results (default 8)"}}}},"required":["query"]}}"#),
            read_only(),
            d(search),
        )
        .tool_annotated(
            "get_page",
            "One rws docs page as Markdown, by path (e.g. \"building-apps/routing\"; `versions` lists them).",
            &format!(r#"{{"type":"object","properties":{{"path":{{"type":"string"}},{VERSION}}},"required":["path"]}}"#),
            read_only(),
            d(get_page),
        )
        .tool_annotated(
            "get_example",
            "An example program from the rws repository (examples/<name>.rs) with how to run it.",
            &format!(r#"{{"type":"object","properties":{{"name":{{"type":"string","description":"e.g. \"rest_api\""}},{VERSION}}},"required":["name"]}}"#),
            read_only(),
            d(get_example),
        )
        .tool_annotated(
            "api_item",
            "The API reference entry for a type, trait or function of the rust-web-server crate, with its module and a docs.rs link.",
            &format!(r#"{{"type":"object","properties":{{"name":{{"type":"string","description":"e.g. \"AppWithState\" or \"RateLimitLayer\""}},{VERSION}}},"required":["name"]}}"#),
            read_only(),
            d(api_item),
        )
        .tool_annotated(
            "setting",
            "An rws setting by RWS_CONFIG_* name, rws.config.toml key or --flag: meaning, default and type.",
            &format!(r#"{{"type":"object","properties":{{"name":{{"type":"string","description":"e.g. \"RWS_CONFIG_PORT\", \"port\" or \"acme_domains\""}},{VERSION}}},"required":["name"]}}"#),
            read_only(),
            d(setting),
        )
        .tool_annotated(
            "check_config",
            "Check an rws.config.toml the way `rws --check` does (unknown keys, wrong types, routes to undeclared upstreams, ...), without starting anything. Files and secrets it names aren't checked here.",
            &format!(r#"{{"type":"object","properties":{{"toml":{{"type":"string","maxLength":{MAX_CONFIG_BYTES},"description":"the file's contents"}}}},"required":["toml"]}}"#),
            read_only(),
            check_config,
        )
        .tool_annotated(
            "changes",
            "rws changelog entries after one version (and up to another), breaking changes first: what to do when upgrading.",
            r#"{"type":"object","properties":{"since":{"type":"string","description":"the version the project uses now, e.g. \"18.4.0\""},"until":{"type":"string","description":"default: the latest release"}},"required":["since"]}"#,
            read_only(),
            d(changes),
        )
        .resource("rws-docs://{version}/{path}", "rws docs page", "One rws docs page as Markdown, e.g. rws-docs://18.8/building-apps/routing", move |uri| {
            let rest = uri.strip_prefix("rws-docs://").ok_or("expected rws-docs://<version>/<path>")?;
            let (version, path) = rest.split_once('/').ok_or("expected rws-docs://<version>/<path>")?;
            get_page(&resource_docs, &serde_json::json!({ "path": path, "version": version }).to_string())
        })
        .prompt_with_args(
            "start_rws_app",
            "How to lay out and write an app on the rust-web-server crate, for one release (the project's agent guidance).",
            vec![PromptArgDef::optional("version", "rws release, e.g. 18.8; default: the latest")],
            move |raw| {
                let a: VersionArg = args(raw)?;
                let (v, _) = prompt_docs.version(a.version.as_deref())?;
                if v.skill.is_empty() {
                    return Err(format!("rws {} has no agent guidance; use 18.7 or later.", v.version));
                }
                Ok(vec![PromptMessage::user(format!(
                    "You are writing code that uses the rust-web-server crate {} (rws). Follow this guidance, and use the rws-docs tools (search_docs, get_page, api_item, get_example) for anything it doesn't cover.\n\n{}",
                    v.tag, v.skill
                ))])
            },
        )
}

#[cfg(test)]
mod tests;

// The fixture index the tests use; also shows the shape the site build writes.
#[cfg(test)]
pub(crate) fn fixture() -> Docs {
    crate::index::Docs::load(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixture"))).expect("fixture index")
}
