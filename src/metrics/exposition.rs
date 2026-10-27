//! The Prometheus protobuf exposition format (`io.prometheus.client.MetricFamily`,
//! length-delimited), served by `GET /metrics` to a scraper that asks for it.
//! It is the only format that carries native histograms.
//!
//! Families come from the text output of every other producer
//! ([`parse_text`]: counters, gauges and classic histograms) plus
//! `http_server_request_duration_seconds` built directly, with its classic
//! buckets and its native (schema 3) ones side by side. Field numbers are
//! those of prometheus/client_model `metrics.proto`.

use super::http;

/// The `Content-Type` of the protobuf format, also what a scraper's `Accept` names.
pub const PROTOBUF_CONTENT_TYPE: &str = "application/vnd.google.protobuf; proto=io.prometheus.client.MetricFamily; encoding=delimited";

/// Whether an `Accept` header asks for the protobuf format (with a `q` above 0).
pub fn accepts_protobuf(accept: &str) -> bool {
    accept.split(',').any(|range| {
        let mut params = range.split(';').map(str::trim);
        if !params.next().is_some_and(|t| t.eq_ignore_ascii_case("application/vnd.google.protobuf")) {
            return false;
        }
        let mut message = false;
        let mut q = 1.0;
        for param in params {
            match param.split_once('=') {
                Some(("proto", v)) => message = v == "io.prometheus.client.MetricFamily",
                Some(("q", v)) => q = v.parse().unwrap_or(0.0),
                _ => {}
            }
        }
        message && q > 0.0
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Kind {
    Counter = 0,
    Gauge = 1,
    Untyped = 3,
    Histogram = 4,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct ClassicHistogram {
    pub(crate) count: u64,
    pub(crate) sum: f64,
    /// `(upper bound, cumulative count)`, increasing.
    pub(crate) buckets: Vec<(f64, u64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Value {
    Number(f64),
    Histogram(ClassicHistogram, Option<http::Histogram>),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Sample {
    pub(crate) labels: Vec<(String, String)>,
    pub(crate) value: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Family {
    pub(crate) name: String,
    pub(crate) help: String,
    pub(crate) kind: Kind,
    pub(crate) samples: Vec<Sample>,
}

/// Every metric, protobuf-encoded.
pub fn protobuf() -> Vec<u8> {
    let mut families = parse_text(&super::prometheus_text_without_http());
    families.push(http_family());
    families.retain(|f| !f.samples.is_empty());
    encode(&families)
}

fn http_family() -> Family {
    let samples = http::snapshot()
        .into_iter()
        .map(|(series, histogram)| {
            let mut cumulative = 0;
            let buckets = http::BOUNDS
                .iter()
                .enumerate()
                .map(|(i, bound)| {
                    cumulative += histogram.counts[i];
                    (*bound, cumulative)
                })
                .collect();
            let classic = ClassicHistogram { count: histogram.count, sum: histogram.sum, buckets };
            Sample { labels: http::labels(&series), value: Value::Histogram(classic, Some(histogram)) }
        })
        .collect();
    Family { name: http::NAME.to_string(), help: http::HELP.to_string(), kind: Kind::Histogram, samples }
}

// ── text → families ───────────────────────────────────────────────────────────

/// Parse rws's own text output (one `# HELP`/`# TYPE` per family, samples
/// after). Lines it doesn't understand are skipped.
pub(crate) fn parse_text(text: &str) -> Vec<Family> {
    let mut families: Vec<Family> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("# HELP ") {
            let (name, help) = rest.split_once(' ').unwrap_or((rest, ""));
            family_named(&mut families, name).help = help.replace("\\n", "\n").replace("\\\\", "\\");
            continue;
        }
        if let Some(rest) = line.strip_prefix("# TYPE ") {
            let (name, kind) = rest.split_once(' ').unwrap_or((rest, ""));
            family_named(&mut families, name).kind = match kind.trim() {
                "counter" => Kind::Counter,
                "gauge" => Kind::Gauge,
                "histogram" => Kind::Histogram,
                _ => Kind::Untyped,
            };
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let Some((name, labels, value)) = parse_sample(line) else { continue };
        add_sample(&mut families, name, labels, value);
    }
    families
}

fn family_named<'a>(families: &'a mut Vec<Family>, name: &str) -> &'a mut Family {
    match families.iter().position(|f| f.name == name) {
        Some(i) => &mut families[i],
        None => {
            families.push(Family { name: name.to_string(), help: String::new(), kind: Kind::Untyped, samples: Vec::new() });
            families.last_mut().unwrap()
        }
    }
}

fn add_sample(families: &mut Vec<Family>, name: &str, mut labels: Vec<(String, String)>, value: f64) {
    // A histogram's `_bucket`/`_sum`/`_count` lines belong to its family.
    for suffix in ["_bucket", "_sum", "_count"] {
        let Some(base) = name.strip_suffix(suffix) else { continue };
        let Some(family) = families.iter_mut().find(|f| f.name == base && f.kind == Kind::Histogram) else { continue };
        let le = labels.iter().position(|(k, _)| k == "le").map(|i| labels.remove(i).1);
        let sample = match family.samples.iter_mut().position(|s| s.labels == labels) {
            Some(i) => &mut family.samples[i],
            None => {
                family.samples.push(Sample { labels, value: Value::Histogram(ClassicHistogram::default(), None) });
                family.samples.last_mut().unwrap()
            }
        };
        let Value::Histogram(classic, _) = &mut sample.value else { return };
        match suffix {
            "_sum" => classic.sum = value,
            "_count" => classic.count = value as u64,
            _ => {
                let bound = match le.as_deref() {
                    Some("+Inf") | None => return, // implied by the count
                    Some(le) => le.parse().unwrap_or(f64::INFINITY),
                };
                classic.buckets.push((bound, value as u64));
            }
        }
        return;
    }
    let family = family_named(families, name);
    family.samples.push(Sample { labels, value: Value::Number(value) });
}

/// `name{a="x",b="y"} value` → its parts; label values unescaped.
fn parse_sample(line: &str) -> Option<(&str, Vec<(String, String)>, f64)> {
    let name_end = line.find(['{', ' '])?;
    let name = &line[..name_end];
    let mut labels = Vec::new();
    let mut rest = &line[name_end..];
    if let Some(after) = rest.strip_prefix('{') {
        let mut chars = after.char_indices();
        let mut key = String::new();
        let close;
        loop {
            let (i, c) = chars.next()?;
            match c {
                '}' => {
                    close = i;
                    break;
                }
                ',' | ' ' => {}
                '=' => {
                    if chars.next()?.1 != '"' {
                        return None;
                    }
                    let mut value = String::new();
                    loop {
                        match chars.next()?.1 {
                            '"' => break,
                            '\\' => match chars.next()?.1 {
                                'n' => value.push('\n'),
                                other => value.push(other),
                            },
                            other => value.push(other),
                        }
                    }
                    labels.push((std::mem::take(&mut key), value));
                }
                other => key.push(other),
            }
        }
        rest = &after[close + 1..];
    }
    let value = rest.split_whitespace().next()?;
    let value = match value {
        "+Inf" => f64::INFINITY,
        "-Inf" => f64::NEG_INFINITY,
        "NaN" => f64::NAN,
        v => v.parse().ok()?,
    };
    Some((name, labels, value))
}

// ── protobuf encoding ─────────────────────────────────────────────────────────

pub(crate) fn encode(families: &[Family]) -> Vec<u8> {
    let mut out = Vec::new();
    for family in families {
        let mut message = Vec::new();
        string_field(&mut message, 1, &family.name);
        if !family.help.is_empty() {
            string_field(&mut message, 2, &family.help);
        }
        varint_field(&mut message, 3, family.kind as u64);
        for sample in &family.samples {
            bytes_field(&mut message, 4, &metric(family.kind, sample));
        }
        varint(&mut out, message.len() as u64);
        out.extend_from_slice(&message);
    }
    out
}

fn metric(kind: Kind, sample: &Sample) -> Vec<u8> {
    let mut m = Vec::new();
    for (name, value) in &sample.labels {
        let mut pair = Vec::new();
        string_field(&mut pair, 1, name);
        string_field(&mut pair, 2, value);
        bytes_field(&mut m, 1, &pair);
    }
    match &sample.value {
        Value::Number(v) => {
            let mut inner = Vec::new();
            double_field(&mut inner, 1, *v);
            let field = match kind {
                Kind::Counter => 3,
                Kind::Gauge => 2,
                _ => 5, // untyped
            };
            bytes_field(&mut m, field, &inner);
        }
        Value::Histogram(classic, native) => bytes_field(&mut m, 7, &histogram(classic, native.as_ref())),
    }
    m
}

fn histogram(classic: &ClassicHistogram, native: Option<&http::Histogram>) -> Vec<u8> {
    let mut h = Vec::new();
    varint_field(&mut h, 1, classic.count);
    double_field(&mut h, 2, classic.sum);
    for (bound, cumulative) in &classic.buckets {
        let mut bucket = Vec::new();
        varint_field(&mut bucket, 1, *cumulative);
        double_field(&mut bucket, 2, *bound);
        bytes_field(&mut h, 3, &bucket);
    }
    if let Some(native) = native {
        varint_field(&mut h, 5, zigzag(http::SCHEMA as i64));
        double_field(&mut h, 6, http::ZERO_THRESHOLD);
        varint_field(&mut h, 7, native.zero_count);
        let (spans, deltas) = native.native_spans();
        for (offset, length) in &spans {
            let mut span = Vec::new();
            varint_field(&mut span, 1, zigzag(*offset as i64));
            varint_field(&mut span, 2, *length as u64);
            bytes_field(&mut h, 12, &span);
        }
        if !deltas.is_empty() {
            let mut packed = Vec::new();
            for delta in &deltas {
                varint(&mut packed, zigzag(*delta));
            }
            bytes_field(&mut h, 13, &packed);
        }
        if spans.is_empty() && native.zero_count == 0 {
            // No observation yet: an empty span marks it as native anyway
            // (client_golang does the same).
            bytes_field(&mut h, 12, &[0x08, 0x00, 0x10, 0x00]);
        }
    }
    h
}

fn zigzag(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

fn varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn varint_field(out: &mut Vec<u8>, field: u32, v: u64) {
    varint(out, (field as u64) << 3);
    varint(out, v);
}

fn double_field(out: &mut Vec<u8>, field: u32, v: f64) {
    varint(out, ((field as u64) << 3) | 1);
    out.extend_from_slice(&v.to_le_bytes());
}

fn bytes_field(out: &mut Vec<u8>, field: u32, bytes: &[u8]) {
    varint(out, ((field as u64) << 3) | 2);
    varint(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

fn string_field(out: &mut Vec<u8>, field: u32, s: &str) {
    bytes_field(out, field, s.as_bytes());
}

#[cfg(test)]
mod tests;
