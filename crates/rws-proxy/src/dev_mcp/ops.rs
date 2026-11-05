//! The operations tools: what the `rws` binary's `/mcp` endpoint has had
//! since 17.x (moved here from `src/main.rs` in 18.28.0, unchanged), each a
//! thin wrapper over [`crate::admin::ops`] so a tool and its admin API
//! endpoint answer alike.

use crate::cache::CacheLayer;
use crate::mcp::{extract_arg, McpContent, McpServer, ToolAnnotations};

/// The operations tools that change the running server. They are what
/// [`crate::admin::access::McpAuditLayer`] records, and what
/// [`super::DevTools::read_only`] leaves out.
pub const OPS_WRITE_TOOLS: [&str; 8] =
    ["cache_clear", "reload_config", "set_maintenance", "block_ip", "unblock_ip", "feature_set", "authz_write", "authz_delete"];

/// The operations tools that only read.
const OPS_READ_TOOLS: [&str; 17] = [
    "server_config", "feature_flags", "server_metrics", "list_routes", "cache_stats", "rate_limit_config",
    "check_rate_limit", "cors_config", "list_static_files", "maintenance_status", "list_blocked_ips", "feature_list",
    "recent_requests", "recent_errors", "authz_check", "authz_tuples", "traffic_summary",
];

/// Register the 25 operations tools on `mcp` — the `rws` binary's set:
/// configuration, metrics, routes (`routes` as `(method, pattern)`), the
/// response cache (`cache`'s stats and purge; both tools are left out
/// without one), rate limits, CORS, static files, config reload,
/// maintenance mode, the IP blocklist, runtime feature flags, the request
/// log, authorization tuples and checks, and the traffic summary. Each
/// carries [`ToolAnnotations`]: read-only, or changing (destructive except
/// `unblock_ip`).
///
/// No gating here: put it behind a token
/// ([`McpServer::require_bearer`]), or use [`super::DevTools::with_ops`],
/// which mounts it only in development or with a token.
///
/// ```rust,no_run
/// # extern crate rws_proxy as rust_web_server;
/// use rust_web_server::app::App;
/// use rust_web_server::cache::CacheLayer;
/// use rust_web_server::dev_mcp::ops_tools;
///
/// let app = App::with_state(());
/// let routes: Vec<(String, String)> = app.route_entries().into_iter().map(|r| (r.method, r.pattern)).collect();
/// let cache = CacheLayer::memory(500).ttl(60);
/// let mcp = ops_tools(app.wrap(cache.clone()).mcp("rws", "1.0").require_bearer("secret"), routes, Some(cache));
/// ```
pub fn ops_tools(mcp: McpServer, routes: Vec<(String, String)>, cache: Option<CacheLayer>) -> McpServer {
    install(mcp, routes, cache, true)
}

/// [`ops_tools`], without the changing ones when `writes` is false.
pub(super) fn install(mcp: McpServer, routes: Vec<(String, String)>, cache: Option<CacheLayer>, writes: bool) -> McpServer {
    let mcp = mcp
        // ── server introspection ─────────────────────────────────────────────
        .tool(
            "server_config",
            "Active RWS_CONFIG_* environment variables and their current values",
            r#"{"type":"object"}"#,
            |_| Ok(McpContent::json(crate::admin::ops::config_json())),
        )
        .tool(
            "feature_flags",
            "Compile-time feature flags active in this binary",
            r#"{"type":"object"}"#,
            |_| Ok(McpContent::json(crate::admin::ops::build_features_json())),
        )
        .tool(
            "server_metrics",
            "Live server counters: requests, errors, active connections, thread pool queue depth",
            r#"{"type":"object"}"#,
            |_| Ok(McpContent::json(crate::admin::ops::metrics_json())),
        )
        .tool(
            "list_routes",
            "All registered HTTP routes (method + pattern)",
            r#"{"type":"object"}"#,
            move |_| Ok(McpContent::json(crate::admin::ops::routes_json(&routes))),
        );
    // ── cache (only with one) ────────────────────────────────────────────────
    let mcp = match cache {
        Some(cache) => {
            let stats = cache.clone();
            mcp
                .tool(
                    "cache_stats",
                    "Response cache hit/miss counts and current entry count",
                    r#"{"type":"object"}"#,
                    move |_| Ok(McpContent::json(crate::admin::ops::cache_stats_json(&stats))),
                )
                .tool(
                    "cache_clear",
                    "Evict entries from the response cache: all of them, one request target (`path`, e.g. /products/42) or every target under a `prefix`",
                    r#"{"type":"object","properties":{"path":{"type":"string","description":"One request target to purge"},"prefix":{"type":"string","description":"Purge every target starting with this"}}}"#,
                    move |args| {
                        let path = extract_arg(args, "path").filter(|p| !p.is_empty());
                        let prefix = extract_arg(args, "prefix").filter(|p| !p.is_empty());
                        if path.is_none() && prefix.is_none() {
                            cache.clear();
                            return Ok(McpContent::text("cache cleared"));
                        }
                        let target = match path {
                            Some(p) => crate::cache::Purge::Exact(p),
                            None => crate::cache::Purge::Prefix(prefix.unwrap_or_default()),
                        };
                        Ok(McpContent::text(format!("{} entries purged", cache.purge(&target))))
                    },
                )
        }
        None => mcp,
    };
    let mcp = mcp
        // ── rate limiting ────────────────────────────────────────────────────
        .tool(
            "rate_limit_config",
            "Current rate limit settings (max requests per window and window duration)",
            r#"{"type":"object"}"#,
            |_| Ok(McpContent::json(crate::admin::ops::rate_limit_json())),
        )
        .tool(
            "check_rate_limit",
            "Check remaining rate limit quota for a client IP address",
            r#"{"type":"object","properties":{"ip":{"type":"string","description":"Client IP address to check"}},"required":["ip"]}"#,
            |args| Ok(McpContent::json(crate::admin::ops::check_rate_limit_json(&extract_arg(args, "ip").unwrap_or_default()))),
        )
        // ── CORS / config ────────────────────────────────────────────────────
        .tool(
            "cors_config",
            "Current CORS configuration snapshot",
            r#"{"type":"object"}"#,
            |_| Ok(McpContent::json(crate::admin::ops::cors_json())),
        )
        .tool(
            "list_static_files",
            "List files the static file controller can serve from the working directory",
            r#"{"type":"object"}"#,
            |_| {
                let files: Vec<String> = std::fs::read_dir(".")
                    .map_err(|e| e.to_string())?
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
                    .filter_map(|e| e.file_name().into_string().ok())
                    .map(|n| format!(r#""{}""#, n))
                    .collect();
                Ok(McpContent::json(format!("[{}]", files.join(","))))
            },
        )
        .tool(
            "reload_config",
            "Trigger a hot config reload from environment variables and rws.config.toml",
            r#"{"type":"object"}"#,
            |_| {
                crate::config_reload::request_reload();
                Ok(McpContent::text("config reload requested"))
            },
        )
        // ── runtime controls ─────────────────────────────────────────────────
        .tool(
            "maintenance_status",
            "Check whether maintenance mode is currently active",
            r#"{"type":"object"}"#,
            |_| Ok(McpContent::json(crate::admin::ops::maintenance_json())),
        )
        .tool(
            "set_maintenance",
            "Enable or disable maintenance mode (returns 503 for all non-health paths when active)",
            r#"{"type":"object","properties":{"enabled":{"type":"boolean"}},"required":["enabled"]}"#,
            |args| Ok(McpContent::json(crate::admin::ops::set_maintenance(extract_arg(args, "enabled").is_some_and(|v| v == "true")))),
        )
        .tool(
            "block_ip",
            "Add an IP address to the runtime blocklist (returns 403 for future requests)",
            r#"{"type":"object","properties":{"ip":{"type":"string"}},"required":["ip"]}"#,
            |args| Ok(McpContent::json(crate::admin::ops::block(&extract_arg(args, "ip").unwrap_or_default()))),
        )
        .tool(
            "unblock_ip",
            "Remove an IP address from the runtime blocklist",
            r#"{"type":"object","properties":{"ip":{"type":"string"}},"required":["ip"]}"#,
            |args| Ok(McpContent::json(crate::admin::ops::unblock(&extract_arg(args, "ip").unwrap_or_default()))),
        )
        .tool(
            "list_blocked_ips",
            "List all currently blocked IP addresses",
            r#"{"type":"object"}"#,
            |_| Ok(McpContent::json(crate::admin::ops::blocklist_json())),
        )
        .tool(
            "feature_list",
            "List all runtime feature flags and their current enabled state",
            r#"{"type":"object"}"#,
            |_| Ok(McpContent::json(crate::admin::ops::flags_json())),
        )
        .tool(
            "feature_set",
            "Enable or disable a named runtime feature flag",
            r#"{"type":"object","properties":{"name":{"type":"string"},"enabled":{"type":"boolean"}},"required":["name","enabled"]}"#,
            |args| {
                let name = extract_arg(args, "name").unwrap_or_default();
                Ok(McpContent::json(crate::admin::ops::set_flag(&name, extract_arg(args, "enabled").is_some_and(|v| v == "true"))))
            },
        );
    // ── request log ──────────────────────────────────────────────────────────
    let mcp = super::recent_tools(mcp)
        .tool(
            "traffic_summary",
            "Who visited the site: people, AI agents (by name and operator) and bots (by kind), with top pages, referrers, 404s, sessions and bounce rate, sources (channels, UTM) and audience (countries, browsers, devices), for the last N hours; with live=true, who is on the site now instead",
            r#"{"type":"object","properties":{"hours":{"type":"integer","default":24,"description":"24 = a day, 168 = a week, 720 = 30 days"},"host":{"type":"string","description":"only this site (Host header)"},"live":{"type":"boolean","default":false,"description":"people in the last 5 minutes, views per minute for the last 30, current pages"}}}"#,
            |args| {
                let host = extract_arg(args, "host");
                if extract_arg(args, "live").is_some_and(|v| v == "true") {
                    return crate::admin::ops::traffic_live_json(host.as_deref()).map(McpContent::json);
                }
                let hours = extract_arg(args, "hours").and_then(|h| h.parse().ok()).unwrap_or(24);
                crate::admin::ops::traffic_json(hours, host.as_deref()).map(McpContent::json)
            },
        )
        .tool(
            "authz_check",
            "Whether a subject has a relation on an object under the authorization schema, with the path that allowed it",
            r#"{"type":"object","properties":{"subject":{"type":"string","description":"e.g. user:alice"},"relation":{"type":"string","description":"e.g. viewer"},"object":{"type":"string","description":"e.g. doc:42"}},"required":["subject","relation","object"]}"#,
            |args| {
                let arg = |key| extract_arg(args, key).unwrap_or_default();
                crate::admin::ops::authz_check(&arg("subject"), &arg("relation"), &arg("object"))
                    .map(McpContent::json)
                    .map_err(|e| e.to_string())
            },
        )
        .tool(
            "authz_tuples",
            "List authorization relationship tuples, filtered by object (doc:42, or a type like doc), relation and subject",
            r#"{"type":"object","properties":{"object":{"type":"string"},"relation":{"type":"string"},"subject":{"type":"string"},"limit":{"type":"integer","default":100}}}"#,
            |args| {
                let arg = |key| extract_arg(args, key).filter(|v| !v.is_empty());
                let limit = arg("limit").and_then(|n| n.parse().ok()).unwrap_or(100);
                crate::admin::ops::authz_tuples_json(arg("object").as_deref(), arg("relation").as_deref(), arg("subject").as_deref(), limit)
                    .map(McpContent::json)
                    .map_err(|e| e.to_string())
            },
        )
        .tool(
            "authz_write",
            "Add an authorization relationship tuple, e.g. doc:42#viewer@user:alice (checked against the schema)",
            r#"{"type":"object","properties":{"tuple":{"type":"string"}},"required":["tuple"]}"#,
            |args| crate::admin::ops::authz_write(&extract_arg(args, "tuple").unwrap_or_default()).map(McpContent::json),
        )
        .tool(
            "authz_delete",
            "Remove an authorization relationship tuple, e.g. doc:42#viewer@user:alice",
            r#"{"type":"object","properties":{"tuple":{"type":"string"}},"required":["tuple"]}"#,
            |args| crate::admin::ops::authz_delete(&extract_arg(args, "tuple").unwrap_or_default()).map(McpContent::json),
        );
    if !writes {
        for name in OPS_WRITE_TOOLS {
            mcp.remove_tool(name);
        }
    }
    // Behaviour hints, so an AI client can tell looking from changing — and
    // ask before the latter. (No effect for a tool left out above.)
    let mut mcp = mcp;
    for name in OPS_READ_TOOLS {
        mcp = mcp.tool_annotations(name, super::read_only_hint());
    }
    for name in OPS_WRITE_TOOLS {
        mcp = mcp.tool_annotations(name, changes(name != "unblock_ip"));
    }
    mcp
}

/// The hints of a tool that changes something, repeatably.
pub(super) fn changes(destructive: bool) -> ToolAnnotations {
    ToolAnnotations {
        read_only_hint: Some(false),
        destructive_hint: Some(destructive),
        idempotent_hint: Some(true),
        open_world_hint: Some(false),
    }
}
