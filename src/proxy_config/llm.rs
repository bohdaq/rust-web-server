//! `[route.action] type = "llm"`: the [LLM proxy](crate::llm) from the
//! config file (18.9.0).
//!
//! ```toml
//! [[route]]
//!   [route.match]
//!   path = "/v1/*"
//!
//!   [route.action]
//!   type = "llm"
//!
//!   [route.action.llm]
//!   audit_log          = true
//!   redis              = "127.0.0.1:6379"   # share budgets between replicas
//!   redis_password_env = "REDIS_PASSWORD"
//!   redis_prefix       = "rws:llm:"
//!   connect_timeout_ms = 10000
//!   read_timeout_ms    = 300000
//!
//!   [[route.action.llm.provider]]         # tried in this order, per API
//!   name    = "openai"
//!   api     = "openai"                    # or "anthropic"
//!   url     = "https://api.openai.com"
//!   key_env = "OPENAI_API_KEY"
//!
//!   [[route.action.llm.key]]              # a virtual key clients send
//!   identity      = "alice"
//!   key_env       = "ALICE_LLM_KEY"
//!   budget_tokens = 2000000               # optional, with window_secs
//!   window_secs   = 86400
//! ```
//!
//! Secrets never sit in the file: provider keys, virtual keys and the Redis
//! password are read from the environment variables named.

use std::time::Duration;

use crate::config_check::ConfigReport;
use crate::llm::{Api, LlmProxy, Provider};

use super::parser::{get_str, get_u64, section_exists, SectionMap};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LlmConfig {
    pub audit_log: bool,
    pub allow_anonymous: bool,
    pub redis: String,
    pub redis_password_env: String,
    pub redis_prefix: String,
    pub connect_timeout_ms: u64,
    pub read_timeout_ms: u64,
    pub providers: Vec<LlmProviderConfig>,
    pub keys: Vec<LlmKeyConfig>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LlmProviderConfig {
    pub name: String,
    pub api: String,
    pub url: String,
    pub key_env: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LlmKeyConfig {
    pub identity: String,
    pub key_env: String,
    pub budget_tokens: u64,
    pub window_secs: u64,
}

/// The keys `check_unknown_keys` accepts in each part.
pub(crate) const LLM_KEYS: &[&str] =
    &["audit_log", "allow_anonymous", "redis", "redis_password_env", "redis_prefix", "connect_timeout_ms", "read_timeout_ms"];
pub(crate) const PROVIDER_KEYS: &[&str] = &["name", "api", "url", "key_env"];
pub(crate) const KEY_KEYS: &[&str] = &["identity", "key_env", "budget_tokens", "window_secs"];

/// `section` is the action's, e.g. `route[0].action`.
pub(crate) fn parse(map: &SectionMap, section: &str) -> LlmConfig {
    let base = format!("{section}.llm");
    let indexed = |kind: &str| -> Vec<String> {
        (0..).map(|i| format!("{base}.{kind}[{i}]")).take_while(|s| section_exists(map, s)).collect()
    };
    let prefix = get_str(map, &base, "redis_prefix");
    LlmConfig {
        audit_log: get_str(map, &base, "audit_log").eq_ignore_ascii_case("true"),
        allow_anonymous: get_str(map, &base, "allow_anonymous").eq_ignore_ascii_case("true"),
        redis: get_str(map, &base, "redis"),
        redis_password_env: get_str(map, &base, "redis_password_env"),
        redis_prefix: if prefix.is_empty() { "rws:llm:".to_string() } else { prefix },
        connect_timeout_ms: get_u64(map, &base, "connect_timeout_ms", 10_000),
        read_timeout_ms: get_u64(map, &base, "read_timeout_ms", 300_000),
        providers: indexed("provider")
            .into_iter()
            .map(|s| LlmProviderConfig {
                name: get_str(map, &s, "name"),
                api: get_str(map, &s, "api"),
                url: get_str(map, &s, "url"),
                key_env: get_str(map, &s, "key_env"),
            })
            .collect(),
        keys: indexed("key")
            .into_iter()
            .map(|s| LlmKeyConfig {
                identity: get_str(map, &s, "identity"),
                key_env: get_str(map, &s, "key_env"),
                budget_tokens: get_u64(map, &s, "budget_tokens", 0),
                window_secs: get_u64(map, &s, "window_secs", 0),
            })
            .collect(),
    }
}

fn env_set(name: &str) -> bool {
    !std::env::var(name).unwrap_or_default().trim().is_empty()
}

/// Semantic checks for `rws --check` and startup.
pub(crate) fn validate(config: &LlmConfig, label: &str, report: &mut ConfigReport) {
    let offline = crate::config_check::offline();
    if config.providers.is_empty() {
        report.error(format!("{label}: [route.action.llm] has no [[route.action.llm.provider]]"));
    }
    let mut names = Vec::new();
    for (i, p) in config.providers.iter().enumerate() {
        let what = format!("{label}: provider #{}", i + 1);
        if p.name.is_empty() {
            report.error(format!("{what}: needs a name"));
        } else if names.contains(&p.name) {
            report.error(format!("{what}: name {:?} is used twice", p.name));
        } else {
            names.push(p.name.clone());
        }
        let Some(api) = Api::parse(&p.api) else {
            report.error(format!("{what}: api {:?} must be \"openai\" or \"anthropic\"", p.api));
            continue;
        };
        if let Err(e) = Provider::new(p.name.clone(), api, &p.url) {
            report.error(format!("{what}: {e}"));
        }
        if !p.key_env.is_empty() && !offline && !env_set(&p.key_env) {
            report.error(format!("{what}: key_env {} is unset or empty", p.key_env));
        }
    }
    if config.keys.is_empty() && !config.allow_anonymous {
        report.error(format!("{label}: no [[route.action.llm.key]] and allow_anonymous is off: nobody can call the proxy"));
    }
    for k in &config.keys {
        if k.identity.is_empty() || k.key_env.is_empty() {
            report.error(format!("{label}: a key needs identity and key_env"));
        } else if !offline && !env_set(&k.key_env) {
            report.error(format!("{label}: key for {} reads {}, which is unset or empty", k.identity, k.key_env));
        }
        if (k.budget_tokens > 0) != (k.window_secs > 0) {
            report.error(format!("{label}: key for {}: budget_tokens and window_secs go together", k.identity));
        }
    }
    if !config.redis_password_env.is_empty() && config.redis.is_empty() {
        report.warn(format!("{label}: redis_password_env without redis: budgets stay in memory"));
    }
    if !config.redis.is_empty() && !config.keys.iter().any(|k| k.budget_tokens > 0) {
        report.warn(format!("{label}: redis is set but no key has a budget"));
    }
}

/// The proxy `config` describes, secrets read from the environment.
pub(crate) fn build(config: &LlmConfig) -> LlmProxy {
    let mut proxy = LlmProxy::new()
        .audit_log(config.audit_log)
        .timeouts(Duration::from_millis(config.connect_timeout_ms), Duration::from_millis(config.read_timeout_ms));
    for p in &config.providers {
        let Some(api) = Api::parse(&p.api) else { continue };
        match Provider::new(p.name.clone(), api, &p.url) {
            Ok(provider) => {
                let key = if p.key_env.is_empty() { String::new() } else { std::env::var(&p.key_env).unwrap_or_default() };
                proxy = proxy.provider(provider.api_key(key.trim()));
            }
            Err(e) => eprintln!("llm proxy: provider {} skipped: {e}", p.name),
        }
    }
    for k in &config.keys {
        let Ok(key) = std::env::var(&k.key_env) else { continue };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        proxy = proxy.virtual_key(key, k.identity.clone());
        if k.budget_tokens > 0 && k.window_secs > 0 {
            proxy = proxy.budget(k.identity.clone(), k.budget_tokens, k.window_secs);
        }
    }
    if config.allow_anonymous {
        proxy = proxy.allow_anonymous();
    }
    if !config.redis.is_empty() {
        let password = (!config.redis_password_env.is_empty()).then(|| std::env::var(&config.redis_password_env).ok()).flatten();
        proxy = proxy.redis(config.redis.clone(), password, config.redis_prefix.clone());
    }
    proxy
}

#[cfg(test)]
mod tests;
