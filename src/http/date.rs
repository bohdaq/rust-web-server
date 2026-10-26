//! HTTP dates (RFC 9110 §5.6.7, IMF-fixdate): `Sun, 06 Nov 1994 08:49:37 GMT`.

use std::cell::RefCell;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::scheduler::cron::{days_to_ymd, ymd_to_days};

const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"]; // 1970-01-01 was a Thursday
const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// `secs` since the Unix epoch as an IMF-fixdate.
pub(crate) fn format(secs: u64) -> String {
    let days = secs / 86_400;
    let (year, month, day) = days_to_ymd(days);
    let rem = secs % 86_400;
    let mut out = String::with_capacity(29);
    out.push_str(DAYS[(days % 7) as usize]);
    out.push_str(", ");
    push2(&mut out, day as u64);
    out.push(' ');
    out.push_str(MONTHS[(month - 1) as usize]);
    out.push(' ');
    out.push_str(crate::ext::decimal::Decimal::new(year as u128).as_str());
    out.push(' ');
    push2(&mut out, rem / 3600);
    out.push(':');
    push2(&mut out, rem % 3600 / 60);
    out.push(':');
    push2(&mut out, rem % 60);
    out.push_str(" GMT");
    out
}

fn push2(out: &mut String, n: u64) {
    out.push((b'0' + (n / 10 % 10) as u8) as char);
    out.push((b'0' + (n % 10) as u8) as char);
}

/// The current time as an IMF-fixdate, formatted at most once per second
/// per thread (the value has one-second resolution).
pub(crate) fn now() -> String {
    thread_local! {
        static CACHED: RefCell<(u64, String)> = const { RefCell::new((u64::MAX, String::new())) };
    }
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    CACHED.with(|cached| {
        let mut cached = cached.borrow_mut();
        if cached.0 != secs {
            *cached = (secs, format(secs));
        }
        cached.1.clone()
    })
}

/// An HTTP-date back to seconds since the epoch: the IMF-fixdate rws sends,
/// and the two obsolete forms a recipient must still accept (RFC 9110
/// §5.6.7, #56) — RFC 850 (`Sunday, 06-Nov-94 08:49:37 GMT`) and asctime
/// (`Sun Nov  6 08:49:37 1994`). `None` for anything else (a recipient may
/// ignore a date it can't parse, §13.1.3).
pub(crate) fn parse(value: &str) -> Option<u64> {
    let value = value.trim();
    parse_imf_fixdate(value).or_else(|| parse_rfc850(value)).or_else(|| parse_asctime(value))
}

/// All ASCII digits, as a number.
fn digits(text: &str) -> Option<u64> {
    (!text.is_empty() && text.len() <= 4 && text.bytes().all(|d| d.is_ascii_digit()))
        .then(|| text.bytes().fold(0u64, |n, d| n * 10 + (d - b'0') as u64))
}

/// `HH:MM:SS`.
fn time_of_day(text: &str) -> Option<(u64, u64, u64)> {
    let mut parts = text.split(':');
    let (h, m, s) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || h.len() != 2 || m.len() != 2 || s.len() != 2 {
        return None;
    }
    Some((digits(h)?, digits(m)?, digits(s)?))
}

fn to_secs(year: u64, month: &str, day: u64, (hour, min, sec): (u64, u64, u64)) -> Option<u64> {
    let month = MONTHS.iter().position(|m| *m == month)? as u64 + 1;
    if year < 1970 || !(1..=31).contains(&day) || hour > 23 || min > 59 || sec > 60 {
        return None;
    }
    Some(ymd_to_days(year as u32, month as u32, day as u32) * 86_400 + hour * 3600 + min * 60 + sec)
}

/// `Sunday, 06-Nov-94 08:49:37 GMT`. A two-digit year is taken in
/// 1970–2069 (the RFC's "more than 50 years in the future is the past").
fn parse_rfc850(value: &str) -> Option<u64> {
    let (weekday, rest) = value.split_once(", ")?;
    if !weekday.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    let mut fields = rest.split(' ');
    let (date, time, zone) = (fields.next()?, fields.next()?, fields.next()?);
    if fields.next().is_some() || zone != "GMT" {
        return None;
    }
    let mut date = date.split('-');
    let (day, month, year) = (date.next()?, date.next()?, date.next()?);
    if date.next().is_some() || day.len() != 2 || year.len() != 2 {
        return None;
    }
    let yy = digits(year)?;
    to_secs(if yy < 70 { 2000 + yy } else { 1900 + yy }, month, digits(day)?, time_of_day(time)?)
}

/// `Sun Nov  6 08:49:37 1994` (the day space-padded).
fn parse_asctime(value: &str) -> Option<u64> {
    let mut fields = value.split_whitespace();
    let (weekday, month, day, time, year) = (fields.next()?, fields.next()?, fields.next()?, fields.next()?, fields.next()?);
    if fields.next().is_some() || weekday.len() != 3 || year.len() != 4 || day.len() > 2 {
        return None;
    }
    to_secs(digits(year)?, month, digits(day)?, time_of_day(time)?)
}

/// `Sun, 06 Nov 1994 08:49:37 GMT`.
fn parse_imf_fixdate(value: &str) -> Option<u64> {
    let b = value.as_bytes();
    // "Sun, 06 Nov 1994 08:49:37 GMT"
    if b.len() != 29 || &b[3..5] != b", " || b[7] != b' ' || b[11] != b' ' || b[16] != b' '
        || b[19] != b':' || b[22] != b':' || &b[25..] != b" GMT"
    {
        return None;
    }
    let num = |range: std::ops::Range<usize>| -> Option<u64> {
        let digits = &b[range];
        digits.iter().all(u8::is_ascii_digit).then(|| digits.iter().fold(0u64, |n, d| n * 10 + (d - b'0') as u64))
    };
    let day = num(5..7)?;
    let month = MONTHS.iter().position(|m| m.as_bytes() == &b[8..11])? as u64 + 1;
    let year = num(12..16)?;
    let (hour, min, sec) = (num(17..19)?, num(20..22)?, num(23..25)?);
    if year < 1970 || !(1..=31).contains(&day) || hour > 23 || min > 59 || sec > 60 {
        return None;
    }
    Some(ymd_to_days(year as u32, month as u32, day as u32) * 86_400 + hour * 3600 + min * 60 + sec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_rfc_examples_and_edges() {
        assert_eq!("Sun, 06 Nov 1994 08:49:37 GMT", format(784_111_777));
        assert_eq!("Thu, 01 Jan 1970 00:00:00 GMT", format(0));
        assert_eq!("Thu, 29 Feb 2024 23:59:59 GMT", format(1_709_251_199));
        assert_eq!("Fri, 01 Jan 2027 00:00:00 GMT", format(1_798_761_600));
    }

    #[test]
    fn parse_is_the_inverse_of_format() {
        for secs in [0u64, 784_111_777, 1_709_251_199, 1_767_222_000, 1_798_761_600, 4_102_444_799] {
            assert_eq!(Some(secs), parse(&format(secs)), "{}", format(secs));
        }
        assert_eq!(Some(784_111_777), parse("  Sun, 06 Nov 1994 08:49:37 GMT "));
    }

    #[test]
    fn parse_rejects_other_formats_and_garbage() {
        for bad in [
            "", "Sunday, 06-Nov-94 08:49:37 UTC", "Sun Nov  6 08:49:37 94", "Sun, 06 Nov 1994 08:49:37 UTC",
            "Sunday, 6-Nov-94 08:49:37 GMT", "Sun Nov 006 08:49:37 1994", "Sun Nov  6 8:49:37 1994",
            "Sun, 06 Xyz 1994 08:49:37 GMT", "Sun, 6x Nov 1994 08:49:37 GMT", "Sun, 06 Nov 1969 08:49:37 GMT",
            "Sun, 06 Nov 1994 24:00:00 GMT", "Sun, 32 Nov 1994 08:49:37 GMT", "Sün, 06 Nov 1994 08:49:37 GMT",
        ] {
            assert_eq!(None, parse(bad), "{bad:?}");
        }
    }

    #[test]
    fn parse_accepts_the_obsolete_forms() {
        // RFC 9110 §5.6.7's three spellings of one instant (#56).
        assert_eq!(Some(784_111_777), parse("Sunday, 06-Nov-94 08:49:37 GMT"));
        assert_eq!(Some(784_111_777), parse("Sun Nov  6 08:49:37 1994"));
        assert_eq!(Some(784_111_777), parse("Sun Nov 6 08:49:37 1994"));
        assert_eq!(parse("Thu, 01 Jan 2026 00:00:00 GMT"), parse("Thursday, 01-Jan-26 00:00:00 GMT"));
        assert_eq!(parse("Thu, 01 Jan 1970 00:00:00 GMT"), parse("Thursday, 01-Jan-70 00:00:00 GMT"));
    }

    #[test]
    fn now_is_a_current_imf_fixdate() {
        let value = now();
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let parsed = parse(&value).unwrap();
        assert!(secs.abs_diff(parsed) <= 1, "{value}");
    }
}
