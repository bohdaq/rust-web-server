//! The last access-log lines in memory, and live followers of new ones,
//! for the admin API (18.10.0). Off until [`enable`]: then each batch the
//! access-log sink writes out (every 100 ms, off the request path) is also
//! split into lines here.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::sse::{SseEvent, SseSender};

/// Lines kept for [`recent`].
pub const CAPACITY: usize = 500;
/// Followers at once; more are refused.
pub const MAX_FOLLOWERS: usize = 8;
/// A longer line is cut to this many bytes (on a character boundary).
pub const MAX_LINE: usize = 8 * 1024;

static ENABLED: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
struct Tail {
    lines: VecDeque<String>,
    followers: Vec<SseSender>,
}

fn tail() -> &'static Mutex<Tail> {
    static TAIL: OnceLock<Mutex<Tail>> = OnceLock::new();
    TAIL.get_or_init(Mutex::default)
}

/// Start keeping lines (the admin listener does, at start).
pub fn enable() {
    ENABLED.store(true, Ordering::SeqCst);
}

pub(crate) fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

fn cut(line: &str) -> String {
    if line.len() <= MAX_LINE {
        return line.to_string();
    }
    let mut end = MAX_LINE;
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    line[..end].to_string()
}

/// Lines the sink wrote out, newline-separated.
pub(crate) fn record(bytes: &[u8]) {
    if !enabled() {
        return;
    }
    let text = String::from_utf8_lossy(bytes);
    let mut tail = tail().lock().unwrap_or_else(|e| e.into_inner());
    for line in text.split('\n').filter(|l| !l.is_empty()) {
        let line = cut(line);
        // A follower that's gone is dropped; one that's behind misses lines.
        tail.followers.retain(|f| f.try_send(SseEvent::data(line.clone())).is_ok());
        if tail.lines.len() == CAPACITY {
            tail.lines.pop_front();
        }
        tail.lines.push_back(line);
    }
}

/// The last `n` lines, oldest first.
pub fn recent(n: usize) -> Vec<String> {
    let tail = tail().lock().unwrap_or_else(|e| e.into_inner());
    tail.lines.iter().skip(tail.lines.len().saturating_sub(n)).cloned().collect()
}

/// Add a follower: `false` when [`MAX_FOLLOWERS`] already follow.
pub fn follow(sender: SseSender) -> bool {
    let mut tail = tail().lock().unwrap_or_else(|e| e.into_inner());
    tail.followers.retain(|f| f.try_send(SseEvent::data(String::new()).event_type("ping")).is_ok());
    if tail.followers.len() >= MAX_FOLLOWERS {
        return false;
    }
    tail.followers.push(sender);
    true
}
