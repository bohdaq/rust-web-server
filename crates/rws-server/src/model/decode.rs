//! Column values sqlx can't decode without its `chrono`/`uuid` features:
//! PostgreSQL `TIMESTAMPTZ`/`TIMESTAMP`/`DATE`/`UUID` and MySQL
//! `DATETIME`/`TIMESTAMP`/`DATE`, read from their wire bytes into
//! [`Value::Text`] — RFC 3339 for instants (UTC; a zone-less type is taken
//! as UTC), `YYYY-MM-DD` for dates, the hyphenated form for UUIDs. Pure
//! functions, so they're tested on every build.
// Each backend uses its own half; the tests use both.
#![allow(dead_code)]

use super::timestamp::{days_from_civil, Timestamp};
use super::uuid::Uuid;
use super::Value;

/// Seconds from 1970-01-01 to 2000-01-01, PostgreSQL's epoch.
const PG_EPOCH_UNIX_SECS: i64 = 946_684_800;

/// A PostgreSQL value of type `type_name` from its bytes (`binary`: the
/// binary format, else text). `None` when the bytes don't fit the type.
pub(crate) fn pg_value(type_name: &str, binary: bool, bytes: &[u8]) -> Option<Value> {
    if !binary {
        let text = std::str::from_utf8(bytes).ok()?;
        return Some(match type_name {
            "TIMESTAMPTZ" | "TIMESTAMP" => match Timestamp::parse(text) {
                Ok(t) => Value::Text(t.to_rfc3339()),
                Err(_) => Value::Text(text.to_string()),
            },
            _ => Value::Text(text.to_string()),
        });
    }
    match type_name {
        "TIMESTAMPTZ" | "TIMESTAMP" => {
            let micros = i64::from_be_bytes(bytes.try_into().ok()?);
            Some(Value::Text(match micros {
                i64::MAX => "infinity".to_string(),
                i64::MIN => "-infinity".to_string(),
                _ => Timestamp::from_unix_micros(micros.checked_add(PG_EPOCH_UNIX_SECS * 1_000_000)?).to_rfc3339(),
            }))
        }
        "DATE" => {
            let days = i32::from_be_bytes(bytes.try_into().ok()?);
            Some(Value::Text(match days {
                i32::MAX => "infinity".to_string(),
                i32::MIN => "-infinity".to_string(),
                _ => date_text(days as i64 + PG_EPOCH_UNIX_SECS / 86_400),
            }))
        }
        "UUID" => Some(Value::Text(Uuid::from_bytes(bytes.try_into().ok()?).to_string())),
        _ => None,
    }
}

/// A MySQL `DATETIME`/`TIMESTAMP`/`DATE` value from its bytes: the binary
/// protocol's length-prefixed form (`len`, year LE16, month, day, then
/// hour, minute, second, then microseconds LE32 — `len` 0, 4, 7 or 11),
/// or text.
pub(crate) fn mysql_temporal(type_name: &str, bytes: &[u8]) -> Option<Value> {
    let date_only = type_name == "DATE";
    if let Some((&len, rest)) = bytes.split_first() {
        if len as usize == rest.len() && matches!(len, 0 | 4 | 7 | 11) {
            if len == 0 {
                return Some(Value::Text(if date_only { "0000-00-00" } else { "0000-00-00 00:00:00" }.to_string()));
            }
            let year = u16::from_le_bytes([rest[0], rest[1]]) as i64;
            let (month, day) = (rest[2] as u32, rest[3] as u32);
            if date_only {
                return Some(Value::Text(format!("{year:04}-{month:02}-{day:02}")));
            }
            let (h, m, s) = if len >= 7 { (rest[4] as i64, rest[5] as i64, rest[6] as i64) } else { (0, 0, 0) };
            let micros = if len == 11 { u32::from_le_bytes([rest[7], rest[8], rest[9], rest[10]]) as i64 } else { 0 };
            if month == 0 || day == 0 {
                return Some(Value::Text(format!("{year:04}-{month:02}-{day:02} {h:02}:{m:02}:{s:02}")));
            }
            let secs = days_from_civil(year, month, day) * 86_400 + h * 3600 + m * 60 + s;
            return Some(Value::Text(Timestamp::from_unix_micros(secs * 1_000_000 + micros).to_rfc3339()));
        }
    }
    let text = std::str::from_utf8(bytes).ok()?;
    if date_only {
        return Some(Value::Text(text.to_string()));
    }
    Some(Value::Text(match Timestamp::parse(text) {
        Ok(t) => t.to_rfc3339(),
        Err(_) => text.to_string(),
    }))
}

fn date_text(days_since_unix_epoch: i64) -> String {
    let (y, m, d) = super::timestamp::civil_from_days(days_since_unix_epoch);
    format!("{y:04}-{m:02}-{d:02}")
}
