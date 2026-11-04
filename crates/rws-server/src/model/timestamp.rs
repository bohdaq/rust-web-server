//! [`Timestamp`]: a UTC instant with microsecond precision, the type of the
//! `created_at` / `updated_at` / `deleted_at` columns the repository
//! maintains (18.30.0).
//!
//! How it is stored, per backend:
//!
//! | Backend | Column type | What rws writes |
//! |---|---|---|
//! | SQLite | `TEXT` | `2026-11-04T12:34:56.123456Z` (RFC 3339, sorts as text) |
//! | PostgreSQL | `TIMESTAMPTZ` (or `TIMESTAMP`) | the same text, cast to `TIMESTAMPTZ` |
//! | MySQL | `DATETIME(6)` | `2026-11-04 12:34:56.123456` (UTC, no zone) |
//!
//! Reading accepts every form these produce (`T` or space, a fraction of 0
//! to 9 digits, `Z`, `+00`, `+00:00` or no zone, a date alone) and, for an
//! integer column, Unix seconds.

use super::backend::Backend;
use super::{DbError, FromColumn, ToColumn, Value};

/// A UTC instant, microseconds since the Unix epoch.
///
/// ```rust
/// # extern crate rws_server as rust_web_server;
/// use rust_web_server::model::Timestamp;
///
/// let t = Timestamp::parse("2026-11-04 12:34:56.5+00:00").unwrap();
/// assert_eq!(t.to_string(), "2026-11-04T12:34:56.500000Z");
/// assert_eq!(t, Timestamp::from_unix_micros(1_793_795_696_500_000));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(i64);

impl Timestamp {
    /// The current time (the system clock, truncated to microseconds).
    pub fn now() -> Timestamp {
        use std::time::{SystemTime, UNIX_EPOCH};
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(d) => Timestamp(d.as_micros() as i64),
            Err(e) => Timestamp(-(e.duration().as_micros() as i64)),
        }
    }

    /// The instant `micros` microseconds after the Unix epoch.
    pub const fn from_unix_micros(micros: i64) -> Timestamp {
        Timestamp(micros)
    }

    /// The instant `seconds` seconds after the Unix epoch.
    pub const fn from_unix_seconds(seconds: i64) -> Timestamp {
        Timestamp(seconds.saturating_mul(1_000_000))
    }

    /// Microseconds since the Unix epoch.
    pub const fn unix_micros(self) -> i64 {
        self.0
    }

    /// Whole seconds since the Unix epoch (rounded down).
    pub const fn unix_seconds(self) -> i64 {
        self.0.div_euclid(1_000_000)
    }

    /// Parse `YYYY-MM-DD`, optionally followed by `T` or a space,
    /// `HH:MM[:SS[.fraction]]` and a zone (`Z`, `±HH`, `±HHMM`, `±HH:MM`; none
    /// means UTC). Digits past the sixth of the fraction are dropped.
    pub fn parse(text: &str) -> Result<Timestamp, DbError> {
        parse(text.trim()).ok_or_else(|| {
            DbError::new(format!(
                "cannot read {text:?} as a timestamp: expected e.g. 2026-11-04T12:34:56Z or 2026-11-04 12:34:56.123456"
            ))
        })
    }

    /// RFC 3339 in UTC with six fraction digits:
    /// `2026-11-04T12:34:56.123456Z` (also what `Display` writes).
    pub fn to_rfc3339(self) -> String {
        let (date, time, micros) = self.parts();
        format!("{date}T{time}.{micros:06}Z")
    }

    /// The value rws binds for this instant on `backend`: RFC 3339 text,
    /// or `YYYY-MM-DD HH:MM:SS.ffffff` on MySQL, whose `DATETIME` takes no
    /// zone designator.
    pub fn to_value(self, backend: Backend) -> Value {
        match backend {
            #[cfg(feature = "model-mysql")]
            Backend::MySql => {
                let (date, time, micros) = self.parts();
                Value::Text(format!("{date} {time}.{micros:06}"))
            }
            #[allow(unreachable_patterns)]
            _ => Value::Text(self.to_rfc3339()),
        }
    }

    fn parts(self) -> (String, String, i64) {
        let secs = self.0.div_euclid(1_000_000);
        let micros = self.0.rem_euclid(1_000_000);
        let days = secs.div_euclid(86_400);
        let rem = secs.rem_euclid(86_400);
        let (y, m, d) = civil_from_days(days);
        (
            format!("{y:04}-{m:02}-{d:02}"),
            format!("{:02}:{:02}:{:02}", rem / 3600, (rem % 3600) / 60, rem % 60),
            micros,
        )
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_rfc3339())
    }
}

impl std::str::FromStr for Timestamp {
    type Err = DbError;
    fn from_str(s: &str) -> Result<Self, DbError> {
        Timestamp::parse(s)
    }
}

impl FromColumn for Timestamp {
    fn from_column(v: Value) -> Result<Self, DbError> {
        match v {
            Value::Text(s) => Timestamp::parse(&s),
            Value::Int(secs) => Ok(Timestamp::from_unix_seconds(secs)),
            Value::Null => Err(DbError::new("unexpected NULL for Timestamp (use Option<Timestamp>)")),
            other => Err(DbError::new(format!("cannot convert {other:?} to Timestamp"))),
        }
    }
}

impl ToColumn for Timestamp {
    fn to_column(&self) -> Value {
        Value::Text(self.to_rfc3339())
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Timestamp {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_rfc3339())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Timestamp {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Timestamp::parse(&text).map_err(|e| serde::de::Error::custom(e.0))
    }
}

/// Days since 1970-01-01 → (year, month, day), proleptic Gregorian
/// (Howard Hinnant's `civil_from_days`).
#[doc(hidden)]
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// (year, month, day) → days since 1970-01-01 (`days_from_civil`).
#[doc(hidden)]
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// A cursor over ASCII digits and separators.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Cursor<'_> {
    fn digits(&mut self, n: usize) -> Option<i64> {
        let slice = self.bytes.get(self.at..self.at + n)?;
        let mut value = 0i64;
        for &b in slice {
            if !b.is_ascii_digit() {
                return None;
            }
            value = value * 10 + (b - b'0') as i64;
        }
        self.at += n;
        Some(value)
    }

    fn eat(&mut self, byte: u8) -> bool {
        if self.bytes.get(self.at) == Some(&byte) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn done(&self) -> bool {
        self.at == self.bytes.len()
    }
}

fn parse(text: &str) -> Option<Timestamp> {
    let mut c = Cursor { bytes: text.as_bytes(), at: 0 };
    let year = c.digits(4)?;
    if !c.eat(b'-') {
        return None;
    }
    let month = c.digits(2)? as u32;
    if !c.eat(b'-') {
        return None;
    }
    let day = c.digits(2)? as u32;
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    let (mut hour, mut minute, mut second, mut micros) = (0, 0, 0, 0i64);
    let mut offset_secs = 0i64;
    if !c.done() {
        if !(c.eat(b'T') || c.eat(b't') || c.eat(b' ')) {
            return None;
        }
        hour = c.digits(2)?;
        if !c.eat(b':') {
            return None;
        }
        minute = c.digits(2)?;
        if c.eat(b':') {
            second = c.digits(2)?;
            if c.eat(b'.') || c.eat(b',') {
                let start = c.at;
                let mut scale = 100_000i64;
                while let Some(b) = c.peek().filter(u8::is_ascii_digit) {
                    micros += (b - b'0') as i64 * scale;
                    scale /= 10;
                    c.at += 1;
                }
                if c.at == start {
                    return None;
                }
            }
        }
        if hour > 23 || minute > 59 || second > 60 {
            return None;
        }
        if c.eat(b'Z') || c.eat(b'z') {
        } else if let Some(sign @ (b'+' | b'-')) = c.peek() {
            c.at += 1;
            let oh = c.digits(2)?;
            c.eat(b':');
            let om = if c.done() { 0 } else { c.digits(2)? };
            if oh > 23 || om > 59 {
                return None;
            }
            offset_secs = (oh * 3600 + om * 60) * if sign == b'+' { 1 } else { -1 };
        }
        if !c.done() {
            return None;
        }
    }
    let days = days_from_civil(year, month, day);
    let secs = days * 86_400 + hour * 3600 + minute * 60 + second.min(59) - offset_secs;
    Some(Timestamp(secs.checked_mul(1_000_000)?.checked_add(micros)?))
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}
