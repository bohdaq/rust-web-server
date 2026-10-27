//! `http.server.request.duration` — the OpenTelemetry HTTP semantic-convention
//! server metric, recorded by the server core for every request on every
//! protocol (no layer needed).
//!
//! Attributes follow the conventions: `http.request.method` (the known methods,
//! anything else `_OTHER`), `http.response.status_code`, `url.scheme` and
//! `network.protocol.version` — no path, so the series count stays bounded.
//! The duration runs from the moment the request is handed to the app until
//! its response is ready to write (a streamed body's transfer isn't included).
//!
//! Each series keeps the convention's explicit buckets (Prometheus text format,
//! OTLP) and an exponential, schema-3 copy for Prometheus native histograms
//! (the protobuf format, [`super::exposition`]). Observations go to one of
//! [`SHARDS`] locks picked per thread, so threads rarely meet on one.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// The semantic convention's advised bucket boundaries, in seconds.
pub(crate) const BOUNDS: [f64; 14] = [0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5, 10.0];

/// Native histogram resolution: 8 buckets per power of two (growth 2^(1/8) ≈ 9%),
/// client_golang's default.
pub(crate) const SCHEMA: i32 = 3;

/// Observations at or below this count in the zero bucket (client_golang's default).
pub(crate) const ZERO_THRESHOLD: f64 = 2.938735877055719e-39; // 2^-128

/// Native bucket indexes are kept within [-200, 160]: about 30 ns to 1 000 000 s.
const MIN_INDEX: i32 = -200;
const MAX_INDEX: i32 = 160;

const METHODS: [&str; 9] = ["CONNECT", "DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT", "TRACE"];
const OTHER: &str = "_OTHER";

/// The series a request belongs to.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct Series {
    /// Index into `METHODS`, or `METHODS.len()` for `_OTHER`.
    method: u8,
    pub(crate) status: i16,
    pub(crate) https: bool,
    /// `network.protocol.version`: 0 = 1.0, 1 = 1.1, 2 = 2, 3 = 3.
    version: u8,
}

impl Series {
    pub(crate) fn new(method: &str, status: i16, https: bool, http_version: &str) -> Series {
        let method = METHODS.iter().position(|m| *m == method).unwrap_or(METHODS.len()) as u8;
        let version = match http_version {
            "HTTP/1.0" => 0,
            "HTTP/2" | "HTTP/2.0" => 2,
            "HTTP/3" | "HTTP/3.0" => 3,
            _ => 1,
        };
        Series { method, status, https, version }
    }

    /// All four fields in one number (what the store hashes).
    fn key(&self) -> u32 {
        (self.method as u32) << 24 | (self.status as u16 as u32) << 8 | (self.https as u32) << 4 | self.version as u32
    }

    pub(crate) fn method(&self) -> &'static str {
        METHODS.get(self.method as usize).copied().unwrap_or(OTHER)
    }

    pub(crate) fn scheme(&self) -> &'static str {
        if self.https { "https" } else { "http" }
    }

    pub(crate) fn protocol_version(&self) -> &'static str {
        ["1.0", "1.1", "2", "3"][self.version as usize]
    }
}

/// One series' observations.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Histogram {
    pub(crate) count: u64,
    pub(crate) sum: f64,
    /// Per explicit bucket (not cumulative): `counts[i]` is `(BOUNDS[i-1], BOUNDS[i]]`,
    /// the last one `(10, +Inf)`.
    pub(crate) counts: [u64; BOUNDS.len() + 1],
    pub(crate) zero_count: u64,
    /// Native buckets `native_offset ..`, each `(2^((i-1)/8), 2^(i/8)]`.
    pub(crate) native_offset: i32,
    pub(crate) native: Vec<u64>,
}

impl Histogram {
    pub(crate) fn observe(&mut self, secs: f64) {
        self.count += 1;
        self.sum += secs;
        let slot = BOUNDS.iter().position(|b| secs <= *b).unwrap_or(BOUNDS.len());
        self.counts[slot] += 1;
        if secs <= ZERO_THRESHOLD {
            self.zero_count += 1;
            return;
        }
        self.add_native(native_index(secs), 1);
    }

    fn add_native(&mut self, index: i32, n: u64) {
        if self.native.is_empty() {
            self.native_offset = index;
            self.native.push(0);
        } else if index < self.native_offset {
            let grow = (self.native_offset - index) as usize;
            self.native.splice(0..0, std::iter::repeat_n(0, grow));
            self.native_offset = index;
        } else if index >= self.native_offset + self.native.len() as i32 {
            self.native.resize((index - self.native_offset + 1) as usize, 0);
        }
        self.native[(index - self.native_offset) as usize] += n;
    }

    fn merge(&mut self, other: &Histogram) {
        self.count += other.count;
        self.sum += other.sum;
        for (mine, theirs) in self.counts.iter_mut().zip(other.counts.iter()) {
            *mine += theirs;
        }
        self.zero_count += other.zero_count;
        for (i, n) in other.native.iter().enumerate() {
            if *n == 0 {
                continue;
            }
            self.add_native(other.native_offset + i as i32, *n);
        }
    }

    /// The native buckets as Prometheus spans (offset, length) and count deltas:
    /// runs of empty buckets longer than two start a new span.
    pub(crate) fn native_spans(&self) -> (Vec<(i32, u32)>, Vec<i64>) {
        let mut spans: Vec<(i32, u32)> = Vec::new();
        let mut deltas = Vec::new();
        let mut previous_count = 0i64;
        let mut last_index: Option<i32> = None;
        for (i, n) in self.native.iter().enumerate() {
            if *n == 0 {
                continue;
            }
            let index = self.native_offset + i as i32;
            match last_index {
                Some(last) if index - last <= 3 => {
                    // Fill a short gap with zero buckets inside the span.
                    for _ in last + 1..index {
                        deltas.push(-previous_count);
                        previous_count = 0;
                    }
                    if let Some(span) = spans.last_mut() {
                        span.1 += (index - last) as u32;
                    }
                }
                Some(last) => spans.push((index - last - 1, 1)),
                None => spans.push((index, 1)),
            }
            deltas.push(*n as i64 - previous_count);
            previous_count = *n as i64;
            last_index = Some(index);
        }
        (spans, deltas)
    }
}

/// Upper bounds of the eight schema-3 buckets within one power of two:
/// 2^(k/8) for k = 0..=8.
const MANTISSA_BOUNDS: [f64; 9] = [
    1.0,
    1.0905077326652577,
    1.189207115002721,
    1.2968395546510096,
    1.4142135623730951,
    1.5422108254079407,
    1.681792830507429,
    1.8340080864093424,
    2.0,
];

/// The schema-3 bucket holding `secs` (> 0): `ceil(log2(secs) · 8)`, clamped —
/// from the float's exponent and a table lookup on its mantissa (no `log2`).
pub(crate) fn native_index(secs: f64) -> i32 {
    let bits = secs.to_bits();
    let raw_exp = ((bits >> 52) & 0x7ff) as i32;
    if raw_exp == 0 {
        return MIN_INDEX; // subnormal: far below anything a request takes
    }
    // secs = mantissa · 2^exp, mantissa in [1, 2).
    let exp = raw_exp - 1023;
    let mantissa = f64::from_bits((bits & !(0x7ff << 52)) | (1023u64 << 52));
    let within = MANTISSA_BOUNDS.iter().position(|bound| mantissa <= *bound).unwrap_or(8) as i32;
    ((exp << SCHEMA) + within).clamp(MIN_INDEX, MAX_INDEX)
}

// ── store ─────────────────────────────────────────────────────────────────────

const SHARDS: usize = 16;

impl std::hash::Hash for Series {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u32(self.key());
    }
}

/// Hashes a [`Series`] key with one multiplication (SipHash cost more than
/// the rest of an observation).
#[derive(Default)]
struct SeriesHasher(u64);

impl std::hash::Hasher for SeriesHasher {
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 << 8 | *b as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }
    }
    fn write_u32(&mut self, n: u32) {
        self.0 = (n as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

type Store = HashMap<Series, Histogram, std::hash::BuildHasherDefault<SeriesHasher>>;

fn shards() -> &'static [Mutex<Store>] {
    static STORE: OnceLock<Vec<Mutex<Store>>> = OnceLock::new();
    STORE.get_or_init(|| (0..SHARDS).map(|_| Mutex::new(Store::default())).collect())
}

thread_local! {
    static SHARD: usize = {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        NEXT.fetch_add(1, Ordering::Relaxed) % SHARDS
    };
}

/// Record a finished request: `started` is when it was handed to the app.
pub fn observe(method: &str, status: i16, https: bool, http_version: &str, started: Instant) {
    let secs = started.elapsed().as_secs_f64();
    record(Series::new(method, status, https, http_version), secs);
}

pub(crate) fn record(series: Series, secs: f64) {
    let shard = SHARD.with(|s| *s);
    let mut map = shards()[shard].lock().unwrap_or_else(|e| e.into_inner());
    map.entry(series).or_default().observe(secs);
}

/// Every series, merged across shards, in a stable order.
pub(crate) fn snapshot() -> Vec<(Series, Histogram)> {
    let mut merged: HashMap<Series, Histogram> = HashMap::new();
    for shard in shards() {
        let map = shard.lock().unwrap_or_else(|e| e.into_inner());
        for (series, histogram) in map.iter() {
            merged.entry(*series).or_default().merge(histogram);
        }
    }
    let mut all: Vec<_> = merged.into_iter().collect();
    all.sort_by_key(|(s, _)| *s);
    all
}

pub(crate) const NAME: &str = "http_server_request_duration_seconds";
pub(crate) const HELP: &str = "Duration of HTTP server requests (OpenTelemetry http.server.request.duration)";

/// The series' labels, Prometheus-style (dots become underscores).
pub(crate) fn labels(series: &Series) -> Vec<(String, String)> {
    vec![
        ("http_request_method".to_string(), series.method().to_string()),
        ("http_response_status_code".to_string(), series.status.to_string()),
        ("network_protocol_version".to_string(), series.protocol_version().to_string()),
        ("url_scheme".to_string(), series.scheme().to_string()),
    ]
}

/// The classic-histogram text form.
pub(crate) fn prometheus_text() -> String {
    let all = snapshot();
    if all.is_empty() {
        return String::new();
    }
    let mut out = format!("# HELP {NAME} {HELP}\n# TYPE {NAME} histogram\n");
    for (series, histogram) in &all {
        let labels: String = labels(series).iter().map(|(k, v)| format!("{k}=\"{v}\",")).collect();
        let mut cumulative = 0;
        for (i, bound) in BOUNDS.iter().enumerate() {
            cumulative += histogram.counts[i];
            out.push_str(&format!("{NAME}_bucket{{{labels}le=\"{bound}\"}} {cumulative}\n"));
        }
        out.push_str(&format!("{NAME}_bucket{{{labels}le=\"+Inf\"}} {}\n", histogram.count));
        let labels = labels.trim_end_matches(',');
        out.push_str(&format!("{NAME}_sum{{{labels}}} {}\n", histogram.sum));
        out.push_str(&format!("{NAME}_count{{{labels}}} {}\n", histogram.count));
    }
    out
}

#[cfg(test)]
mod tests;
