use std::collections::HashMap;
use std::env;
use std::io::{BufRead, Cursor};
use crate::entry_point::command_line_args::CommandLineArgument;
use file_ext::FileExt;

#[cfg(test)]
mod tests;

/// Settings that have an `RWS_CONFIG_*` variable but no command-line flag.
/// In `rws.config.toml` they're written like any other key — the variable
/// name is the table path plus key, upper-cased: `[rate_limit] max_requests`
/// → `RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS`. Settings that do have a flag are
/// taken from `CommandLineArgument::get_command_line_arg_list()` instead.
/// `tests.rs` checks this list against every `RWS_CONFIG_*` name in `src/`.
pub const SETTINGS_WITHOUT_FLAG: &[&str] = &[
    "RWS_CONFIG_ACME_ACCOUNT_KEY_PATH",
    "RWS_CONFIG_ACME_RENEW_BEFORE_DAYS",
    "RWS_CONFIG_ACME_RENEW_CHECK_INTERVAL_SECS",
    "RWS_CONFIG_CIRCUIT_BREAKER_FAILURE_THRESHOLD",
    "RWS_CONFIG_CIRCUIT_BREAKER_MAX_HALF_OPEN_PROBES",
    "RWS_CONFIG_CIRCUIT_BREAKER_RECOVERY_SECS",
    "RWS_CONFIG_CSP",
    "RWS_CONFIG_LOG_FORMAT",
    "RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES",
    "RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS",
    "RWS_CONFIG_RATE_LIMIT_WINDOW_SECS",
    "RWS_CONFIG_SPA_FALLBACK",
    "RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES",
    "RWS_CONFIG_TEMPLATE_DIR",
    "RWS_CONFIG_TLS_CLIENT_CA_FILE",
];

/// Tables (and their sub-tables) read by `proxy_config`'s own parser, not
/// mapped to `RWS_CONFIG_*` variables.
const PROXY_TABLES: &[&str] = &["route", "upstream", "tcp_proxy", "udp_proxy", "ws_proxy"];

/// `[[virtual_host]]` fields, set as `RWS_CONFIG_VIRTUAL_HOST_<n>_<FIELD>`.
const VIRTUAL_HOST_FIELDS: &[&str] = &["DOMAIN", "CERT_FILE", "KEY_FILE"];

/// Apply every setting in a `rws.config.toml` document to the process
/// environment. `prefix` is the table the document starts in (normally empty).
///
/// Top-level keys and keys in plain `[tables]` map to `RWS_CONFIG_<TABLE>_<KEY>`
/// (`[cors] allow_all` → `RWS_CONFIG_CORS_ALLOW_ALL`); each `[[virtual_host]]`
/// block maps to `RWS_CONFIG_VIRTUAL_HOST_<n>_*`. Strings keep their exact
/// contents (spaces, quotes inside them); arrays become comma-separated lists.
/// Unknown keys are reported on stderr instead of being ignored silently.
pub fn read_config_file(
    cursor: Cursor<&[u8]>,
    prefix: String) -> Result<bool, String> {

    let mut table = prefix;
    // Index of the current [[array-table]] entry, per array-table name.
    let mut array_table_counters: HashMap<String, usize> = HashMap::new();
    let mut array_index: Option<usize> = None;
    let mut flag_arguments: Vec<String> = vec![];
    let flags = CommandLineArgument::get_command_line_arg_list();

    for line in cursor.lines() {
        let line = line.map_err(|e| e.to_string())?;
        let line = strip_comment(&line).trim().to_string();
        if line.is_empty() {
            continue;
        }

        if let Some(name) = line.strip_prefix("[[").and_then(|l| l.strip_suffix("]]")) {
            let name = name.trim().to_string();
            let count = array_table_counters.entry(name.clone()).or_insert(0);
            array_index = Some(*count);
            *count += 1;
            table = name;
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            table = name.trim().to_string();
            array_index = None;
            continue;
        }

        let Some((raw_key, raw_value)) = line.split_once('=') else { continue };
        let key = raw_key.trim();
        let value = parse_value(raw_value);

        let root = table.split('.').next().unwrap_or("");
        if PROXY_TABLES.contains(&root) {
            continue;
        }

        if table == "virtual_host" {
            let field = key.to_ascii_uppercase();
            if VIRTUAL_HOST_FIELDS.contains(&field.as_str()) {
                let name = format!("RWS_CONFIG_VIRTUAL_HOST_{}_{}", array_index.unwrap_or(0), field);
                set_setting(&name, &value);
            } else {
                warn_unknown(&table, key);
            }
            continue;
        }

        let path = if table.is_empty() { key.to_string() } else { format!("{}_{}", table.replace('.', "_"), key) };
        let long_form = path.replace('_', "-");
        if flags.iter().any(|f| f.long_form == long_form) {
            flag_arguments.push(format!("--{}={}", long_form, value));
            continue;
        }
        let name = format!("RWS_CONFIG_{}", path.to_ascii_uppercase());
        if SETTINGS_WITHOUT_FLAG.contains(&name.as_str()) {
            set_setting(&name, &value);
        } else {
            warn_unknown(&table, key);
        }
    }
    CommandLineArgument::_parse(flag_arguments, flags);

    Ok(true)
}

fn set_setting(name: &str, value: &str) {
    env::set_var(name, value);
    println!("    Set env variable '{}' to value '{}'", name, value);
}

fn warn_unknown(table: &str, key: &str) {
    let qualified = if table.is_empty() { key.to_string() } else { format!("[{}] {}", table, key) };
    eprintln!("    Warning: unknown setting '{}' in rws.config.toml — ignored", qualified);
}

/// Drop a trailing `# comment`, ignoring `#` inside quoted strings.
fn strip_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    for (i, c) in line.char_indices() {
        match (quote, c) {
            (None, '"') | (None, '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, '#') => return &line[..i],
            _ => {}
        }
    }
    line
}

/// A TOML value as a plain string: quoted strings lose only their quotes,
/// arrays become `a,b,c`, anything else (numbers, booleans) is trimmed.
fn parse_value(raw: &str) -> String {
    let raw = raw.trim();
    if let Some(inner) = raw.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        return split_array(inner)
            .iter()
            .map(|item| unquote(item.trim()))
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join(",");
    }
    unquote(raw)
}

fn unquote(s: &str) -> String {
    for q in ['"', '\''] {
        if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
            return s[1..s.len() - 1].to_string();
        }
    }
    s.to_string()
}

/// Split array items on commas outside quotes.
fn split_array(inner: &str) -> Vec<&str> {
    let mut items = vec![];
    let mut quote: Option<char> = None;
    let mut start = 0;
    for (i, c) in inner.char_indices() {
        match (quote, c) {
            (None, '"') | (None, '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, ',') => {
                items.push(&inner[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    items.push(&inner[start..]);
    items
}

/// Load `rws.config.toml` into the environment: `filepath` (relative to the
/// working directory) when given, else the file named by `RWS_CONFIG_FILE`
/// (absolute, or relative to the working directory) — the same variable
/// `proxy_config` reads routes from — else `rws.config.toml` in the working
/// directory.
pub fn override_environment_variables_from_config(filepath: Option<&str>) {
    println!("\n  Start of Config Section");

    let from_env = env::var("RWS_CONFIG_FILE").ok().filter(|p| !p.trim().is_empty());
    let resolved = match (filepath, from_env) {
        (Some(path), _) => FileExt::get_static_filepath(path),
        (None, Some(path)) => Ok(path),
        (None, None) => FileExt::get_static_filepath("/rws.config.toml"),
    };
    let path = match resolved {
        Ok(path) => path,
        Err(e) => {
            eprintln!("{}", e);
            println!("  End of Config Section");
            return;
        }
    };

    match std::fs::read_to_string(&path) {
        Ok(content) => {
            if let Err(e) = read_config_file(Cursor::new(content.as_bytes()), String::new()) {
                eprintln!("    Unable to parse {}: {}", path, e);
            }
        }
        Err(e) => eprintln!("    Unable to read {}: {}", path, e),
    }

    println!("  End of Config Section");
}
