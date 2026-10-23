//! Unit tests for `rws.config.toml` → `RWS_CONFIG_*` mapping.

use std::env;
use std::io::Cursor;

use super::{parse_value, read_config_file, strip_comment, SETTINGS_WITHOUT_FLAG};
use crate::entry_point::command_line_args::CommandLineArgument;

/// Runs `toml` through `read_config_file`, returns the requested variables,
/// and restores every variable it may have touched. Caller holds the lock.
fn load_and_read(toml: &str, vars: &[&str]) -> Vec<Option<String>> {
    let saved: Vec<(&str, Option<String>)> = vars.iter().map(|v| (*v, env::var(v).ok())).collect();
    for v in vars {
        env::remove_var(v);
    }
    read_config_file(Cursor::new(toml.as_bytes()), String::new()).unwrap();
    let values = vars.iter().map(|v| env::var(v).ok()).collect();
    for (v, old) in saved {
        match old {
            Some(old) => env::set_var(v, old),
            None => env::remove_var(v),
        }
    }
    values
}

#[test]
fn top_level_keys_and_flag_tables_map_to_their_variables() {
    let _g = crate::test_env::lock();
    let got = load_and_read(
        "port = 8443\ntls_cert_file = \"/etc/rws/cert.pem\"\n\n[cors]\nallow_all = false\nallow_origins = [\"https://a.example\", \"https://b.example\"]\n",
        &["RWS_CONFIG_PORT", "RWS_CONFIG_TLS_CERT_FILE", "RWS_CONFIG_CORS_ALLOW_ALL", "RWS_CONFIG_CORS_ALLOW_ORIGINS"],
    );
    assert_eq!(
        vec![Some("8443".into()), Some("/etc/rws/cert.pem".into()), Some("false".into()), Some("https://a.example,https://b.example".into())],
        got
    );
}

#[test]
fn tables_without_a_flag_map_to_table_and_key() {
    let _g = crate::test_env::lock();
    let got = load_and_read(
        "log_format = \"json\"\nmax_body_size_in_bytes = 1048576\n\n[rate_limit]\nmax_requests = 500\nwindow_secs = 30\n\n[circuit_breaker]\nfailure_threshold = 7\n",
        &["RWS_CONFIG_LOG_FORMAT", "RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES", "RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS",
          "RWS_CONFIG_RATE_LIMIT_WINDOW_SECS", "RWS_CONFIG_CIRCUIT_BREAKER_FAILURE_THRESHOLD"],
    );
    assert_eq!(
        vec![Some("json".into()), Some("1048576".into()), Some("500".into()), Some("30".into()), Some("7".into())],
        got
    );
}

#[test]
fn virtual_host_blocks_are_numbered_in_order() {
    let _g = crate::test_env::lock();
    let got = load_and_read(
        "[[virtual_host]]\ndomain = \"a.example\"\ncert_file = \"a.pem\"\nkey_file = \"a.key\"\n\n[[virtual_host]]\ndomain = \"b.example\"\ncert_file = \"b.pem\"\nkey_file = \"b.key\"\n",
        &["RWS_CONFIG_VIRTUAL_HOST_0_DOMAIN", "RWS_CONFIG_VIRTUAL_HOST_0_KEY_FILE",
          "RWS_CONFIG_VIRTUAL_HOST_1_DOMAIN", "RWS_CONFIG_VIRTUAL_HOST_1_CERT_FILE", "RWS_CONFIG_VIRTUAL_HOST_2_DOMAIN"],
    );
    assert_eq!(
        vec![Some("a.example".into()), Some("a.key".into()), Some("b.example".into()), Some("b.pem".into()), None],
        got
    );
}

#[test]
fn string_values_keep_spaces_quotes_and_hashes() {
    let _g = crate::test_env::lock();
    let got = load_and_read(
        "csp = \"default-src 'self'; img-src * data:\"  # trailing comment\ntemplate_dir = 'tem#plates'\n",
        &["RWS_CONFIG_CSP", "RWS_CONFIG_TEMPLATE_DIR"],
    );
    assert_eq!(vec![Some("default-src 'self'; img-src * data:".into()), Some("tem#plates".into())], got);
}

#[test]
fn proxy_tables_and_unknown_keys_set_nothing() {
    let _g = crate::test_env::lock();
    let got = load_and_read(
        "[server]\nport = 9999\n\n[[upstream]]\nname = \"api\"\n\n[[route]]\n[route.match]\npath = \"/*\"\n[route.action]\ntype = \"proxy\"\n",
        &["RWS_CONFIG_SERVER_PORT", "RWS_CONFIG_PORT", "RWS_CONFIG_UPSTREAM_NAME", "RWS_CONFIG_ROUTE_MATCH_PATH"],
    );
    assert_eq!(vec![None, None, None, None], got);
}

#[test]
fn rws_config_file_selects_the_file_when_no_path_is_given() {
    let _g = crate::test_env::lock();
    let path = env::temp_dir().join(format!("rws_config_file_test_{}.toml", std::process::id()));
    std::fs::write(&path, "log_format = \"json\"\n").unwrap();
    let saved = (env::var("RWS_CONFIG_FILE").ok(), env::var("RWS_CONFIG_LOG_FORMAT").ok());
    env::set_var("RWS_CONFIG_FILE", &path);
    env::remove_var("RWS_CONFIG_LOG_FORMAT");
    super::override_environment_variables_from_config(None);
    let got = env::var("RWS_CONFIG_LOG_FORMAT").ok();
    match saved.0 { Some(v) => env::set_var("RWS_CONFIG_FILE", v), None => env::remove_var("RWS_CONFIG_FILE") }
    match saved.1 { Some(v) => env::set_var("RWS_CONFIG_LOG_FORMAT", v), None => env::remove_var("RWS_CONFIG_LOG_FORMAT") }
    assert_eq!(Some("json".to_string()), got);
}

#[test]
fn parse_value_handles_strings_arrays_and_scalars() {
    assert_eq!("a b", parse_value(" \"a b\" "));
    assert_eq!("it's", parse_value("\"it's\""));
    assert_eq!("x,y", parse_value("[\"x\", 'y']"));
    assert_eq!("a, b,c", parse_value("[\"a, b\", \"c\"]"));
    assert_eq!("", parse_value("[]"));
    assert_eq!("42", parse_value(" 42 "));
    assert_eq!("true", parse_value("true"));
}

#[test]
fn strip_comment_ignores_hash_inside_quotes() {
    assert_eq!("color = \"#fff\" ", strip_comment("color = \"#fff\" # hex"));
    assert_eq!("", strip_comment("# whole line"));
}

/// Every RWS_CONFIG_* variable the code reads must be settable from the
/// config file — via a command-line flag or SETTINGS_WITHOUT_FLAG.
#[test]
fn every_setting_in_the_source_is_reachable_from_the_config_file() {
    let flags: Vec<String> = CommandLineArgument::get_command_line_arg_list()
        .into_iter()
        .map(|a| a.environment_variable)
        .collect();
    let mut missing = vec![];
    let mut stack = vec![std::path::PathBuf::from("src")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().map_or(true, |e| e != "rs") || path.to_string_lossy().contains("tests") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            for (i, _) in source.match_indices("\"RWS_CONFIG_") {
                let name: String = source[i + 1..]
                    .chars()
                    .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
                    .collect();
                if name.ends_with('_') {
                    continue; // a format! template like "RWS_CONFIG_{}", not a setting
                }
                let reachable = flags.contains(&name)
                    || SETTINGS_WITHOUT_FLAG.contains(&name.as_str())
                    || name == "RWS_CONFIG_FILE"
                    || name.starts_with("RWS_CONFIG_VIRTUAL_HOST_");
                if !reachable && !missing.contains(&name) {
                    missing.push(name);
                }
            }
        }
    }
    assert!(missing.is_empty(), "not settable from rws.config.toml: {:?} — add to SETTINGS_WITHOUT_FLAG", missing);
}
