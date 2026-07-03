//! Path-pattern parsing and segment matching shared by [`super::Router`] and
//! `AsyncAppWithState` (`crate::async_state`).
//!
//! `AsyncAppWithState`'s handlers return `Future`s, which `Router`'s
//! `HandlerFn` type doesn't support, so it can't reuse `Router` itself — but
//! the underlying pattern-matching algorithm is identical, so it lives here
//! once instead of being duplicated.

use std::collections::HashMap;

#[derive(Clone)]
pub(crate) enum Segment {
    Literal(String),
    Param(String),
    Wildcard(String),
}

pub(crate) fn parse_pattern(pattern: &str) -> Vec<Segment> {
    if pattern == "/" {
        return vec![];
    }
    pattern
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|seg| {
            if let Some(name) = seg.strip_prefix(':') {
                Segment::Param(name.to_string())
            } else if let Some(name) = seg.strip_prefix('*') {
                Segment::Wildcard(name.to_string())
            } else {
                Segment::Literal(seg.to_string())
            }
        })
        .collect()
}

/// Matches `path` (already split on `/`, empty segments filtered out)
/// against `pattern`. Returns the extracted named/wildcard values on a match.
pub(crate) fn try_match(pattern: &[Segment], path: &[&str]) -> Option<HashMap<String, String>> {
    let mut params = HashMap::new();
    let mut pi = 0;

    for (si, seg) in pattern.iter().enumerate() {
        match seg {
            Segment::Literal(lit) => {
                if pi >= path.len() || path[pi] != lit.as_str() {
                    return None;
                }
                pi += 1;
            }
            Segment::Param(name) => {
                if pi >= path.len() {
                    return None;
                }
                params.insert(name.clone(), path[pi].to_string());
                pi += 1;
            }
            Segment::Wildcard(name) => {
                if si != pattern.len() - 1 {
                    return None; // wildcard must be the last segment
                }
                params.insert(name.clone(), path[pi..].join("/"));
                pi = path.len();
            }
        }
    }

    if pi == path.len() { Some(params) } else { None }
}
