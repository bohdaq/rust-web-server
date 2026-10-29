//! `rws_llm_*` series for the [LLM proxy](crate::llm) (18.9.0).
//!
//! The `model` label is what clients asked for, so it is bounded: past
//! [`MAX_MODELS`] distinct names, or for a name that isn't a plain model
//! identifier, it reads `_other`. `identity` comes from configured virtual
//! keys, `provider` from configured providers.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// Distinct `model` label values kept.
pub const MAX_MODELS: usize = 64;

/// Time-to-first-token bucket bounds, in seconds.
const BUCKETS: [f64; 11] = [0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0];

#[derive(Default)]
struct Histogram {
    counts: [u64; BUCKETS.len()],
    count: u64,
    sum: f64,
}

impl Histogram {
    fn observe(&mut self, seconds: f64) {
        for (bound, count) in BUCKETS.iter().zip(self.counts.iter_mut()) {
            if seconds <= *bound {
                *count += 1;
            }
        }
        self.count += 1;
        self.sum += seconds;
    }
}

#[derive(Default)]
struct Stats {
    models: HashSet<String>,
    /// (provider, model, status)
    requests: HashMap<(String, String, String), u64>,
    /// (provider, model, identity, "input" | "output")
    tokens: HashMap<(String, String, String, &'static str), u64>,
    /// (provider, model)
    ttft: HashMap<(String, String), Histogram>,
    /// provider that failed over to the next
    fallbacks: HashMap<String, u64>,
    /// identity
    budget_rejections: HashMap<String, u64>,
}

fn stats() -> &'static Mutex<Stats> {
    static STATS: OnceLock<Mutex<Stats>> = OnceLock::new();
    STATS.get_or_init(Mutex::default)
}

fn with<T>(f: impl FnOnce(&mut Stats) -> T) -> T {
    f(&mut stats().lock().unwrap_or_else(|e| e.into_inner()))
}

fn model_label(stats: &mut Stats, model: &str) -> String {
    if model.is_empty() {
        return "unknown".to_string();
    }
    let plain = model.len() <= 64 && model.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.:/@".contains(&b));
    if !plain {
        return "_other".to_string();
    }
    if stats.models.contains(model) {
        return model.to_string();
    }
    if stats.models.len() < MAX_MODELS {
        stats.models.insert(model.to_string());
        return model.to_string();
    }
    "_other".to_string()
}

/// One finished request: its final status (`"error"` when no provider
/// answered), tokens and time to first byte.
pub(crate) fn observe(provider: &str, model: &str, identity: &str, status: &str, usage: Option<crate::llm::Usage>, ttft: Option<Duration>) {
    with(|stats| {
        let model = model_label(stats, model);
        *stats.requests.entry((provider.to_string(), model.clone(), status.to_string())).or_default() += 1;
        if let Some(usage) = usage {
            for (kind, n) in [("input", usage.input_tokens), ("output", usage.output_tokens)] {
                *stats.tokens.entry((provider.to_string(), model.clone(), identity.to_string(), kind)).or_default() += n;
            }
        }
        if let Some(ttft) = ttft {
            stats.ttft.entry((provider.to_string(), model)).or_default().observe(ttft.as_secs_f64());
        }
    });
}

pub(crate) fn observe_fallback(provider: &str) {
    with(|stats| *stats.fallbacks.entry(provider.to_string()).or_default() += 1);
}

pub(crate) fn observe_budget_rejection(identity: &str) {
    with(|stats| *stats.budget_rejections.entry(identity.to_string()).or_default() += 1);
}

fn label(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

fn sorted<K: Ord, V>(map: &HashMap<K, V>) -> Vec<(&K, &V)> {
    let mut entries: Vec<(&K, &V)> = map.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    entries
}

/// The `rws_llm_*` series for `/metrics`; empty until the proxy served a
/// request.
pub(crate) fn prometheus_text() -> String {
    with(|stats| {
        let mut out = String::new();
        if !stats.requests.is_empty() {
            out.push_str("# HELP rws_llm_requests_total Requests through the LLM proxy, by provider, model and final status (\"error\": no provider answered)\n");
            out.push_str("# TYPE rws_llm_requests_total counter\n");
            for ((provider, model, status), n) in sorted(&stats.requests) {
                out.push_str(&format!("rws_llm_requests_total{{provider=\"{}\",model=\"{}\",status=\"{}\"}} {n}\n", label(provider), label(model), label(status)));
            }
        }
        if !stats.tokens.is_empty() {
            out.push_str("# HELP rws_llm_tokens_total Tokens providers reported, by provider, model, caller identity and type (input includes cache reads and writes)\n");
            out.push_str("# TYPE rws_llm_tokens_total counter\n");
            for ((provider, model, identity, kind), n) in sorted(&stats.tokens) {
                out.push_str(&format!(
                    "rws_llm_tokens_total{{provider=\"{}\",model=\"{}\",identity=\"{}\",type=\"{kind}\"}} {n}\n",
                    label(provider), label(model), label(identity)
                ));
            }
        }
        if !stats.ttft.is_empty() {
            out.push_str("# HELP rws_llm_time_to_first_token_seconds Time from sending a request to a provider to the first byte of its response body\n");
            out.push_str("# TYPE rws_llm_time_to_first_token_seconds histogram\n");
            for ((provider, model), h) in sorted(&stats.ttft) {
                let labels = format!("provider=\"{}\",model=\"{}\"", label(provider), label(model));
                for (bound, count) in BUCKETS.iter().zip(h.counts.iter()) {
                    out.push_str(&format!("rws_llm_time_to_first_token_seconds_bucket{{{labels},le=\"{bound}\"}} {count}\n"));
                }
                out.push_str(&format!("rws_llm_time_to_first_token_seconds_bucket{{{labels},le=\"+Inf\"}} {}\n", h.count));
                out.push_str(&format!("rws_llm_time_to_first_token_seconds_sum{{{labels}}} {}\n", h.sum));
                out.push_str(&format!("rws_llm_time_to_first_token_seconds_count{{{labels}}} {}\n", h.count));
            }
        }
        if !stats.fallbacks.is_empty() {
            out.push_str("# HELP rws_llm_fallbacks_total Requests a provider failed (no response, 429 or 5xx) and the proxy sent to the next one\n");
            out.push_str("# TYPE rws_llm_fallbacks_total counter\n");
            for (provider, n) in sorted(&stats.fallbacks) {
                out.push_str(&format!("rws_llm_fallbacks_total{{provider=\"{}\"}} {n}\n", label(provider)));
            }
        }
        if !stats.budget_rejections.is_empty() {
            out.push_str("# HELP rws_llm_budget_rejections_total Requests refused because the caller's token budget was spent\n");
            out.push_str("# TYPE rws_llm_budget_rejections_total counter\n");
            for (identity, n) in sorted(&stats.budget_rejections) {
                out.push_str(&format!("rws_llm_budget_rejections_total{{identity=\"{}\"}} {n}\n", label(identity)));
            }
        }
        out
    })
}
