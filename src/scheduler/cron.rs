use std::collections::HashSet;

/// One field of a 6-part cron expression.
#[derive(Debug, Clone)]
pub(crate) enum Field {
    Any,
    Values(HashSet<u32>),
}

impl Field {
    pub(crate) fn matches(&self, v: u32) -> bool {
        match self {
            Field::Any => true,
            Field::Values(set) => set.contains(&v),
        }
    }

    /// Parse one cron field. Supports `*`, exact values, `*/step`, `N-M` ranges,
    /// and comma-separated combinations of the above.
    pub(crate) fn parse(s: &str, min: u32, max: u32) -> Result<Self, String> {
        if s == "*" {
            return Ok(Field::Any);
        }
        let mut values = HashSet::new();
        for part in s.split(',') {
            if let Some(step_expr) = part.strip_prefix("*/") {
                let step: u32 = step_expr
                    .parse()
                    .map_err(|_| format!("invalid step '{}' in cron field", part))?;
                if step == 0 {
                    return Err("cron step must be > 0".to_string());
                }
                let mut v = min;
                while v <= max {
                    values.insert(v);
                    v += step;
                }
            } else if let Some(dash) = part.find('-') {
                let lo: u32 = part[..dash]
                    .parse()
                    .map_err(|_| format!("invalid range start in '{}'", part))?;
                let hi: u32 = part[dash + 1..]
                    .parse()
                    .map_err(|_| format!("invalid range end in '{}'", part))?;
                if lo > hi {
                    return Err(format!("range {}-{} is empty", lo, hi));
                }
                if lo < min || hi > max {
                    return Err(format!(
                        "range {}-{} is out of bounds [{}, {}]",
                        lo, hi, min, max
                    ));
                }
                for v in lo..=hi {
                    values.insert(v);
                }
            } else {
                let v: u32 = part
                    .parse()
                    .map_err(|_| format!("invalid cron value '{}'", part))?;
                if v < min || v > max {
                    return Err(format!(
                        "value {} is out of bounds [{}, {}]",
                        v, min, max
                    ));
                }
                values.insert(v);
            }
        }
        Ok(Field::Values(values))
    }
}

/// A parsed 6-field cron expression: `second minute hour day-of-month month day-of-week`.
///
/// Field ranges:
/// - second: 0–59
/// - minute: 0–59
/// - hour:   0–23
/// - day:    1–31
/// - month:  1–12
/// - weekday: 0–6 (0 = Sunday)
#[derive(Debug, Clone)]
pub struct CronSchedule {
    pub(crate) seconds: Field,
    pub(crate) minutes: Field,
    pub(crate) hours: Field,
    pub(crate) days_of_month: Field,
    pub(crate) months: Field,
    pub(crate) days_of_week: Field,
}

impl CronSchedule {
    /// Parse a 6-field cron expression.
    ///
    /// Each field supports `*`, exact values, `*/step`, `N-M` ranges, and
    /// comma-separated combinations, e.g. `0,15,30,45 * * * * *`.
    pub fn parse(expr: &str) -> Result<Self, String> {
        let parts: Vec<&str> = expr.split_whitespace().collect();
        if parts.len() != 6 {
            return Err(format!(
                "expected 6 cron fields (sec min hour day month weekday), got {}",
                parts.len()
            ));
        }
        Ok(CronSchedule {
            seconds: Field::parse(parts[0], 0, 59)?,
            minutes: Field::parse(parts[1], 0, 59)?,
            hours: Field::parse(parts[2], 0, 23)?,
            days_of_month: Field::parse(parts[3], 1, 31)?,
            months: Field::parse(parts[4], 1, 12)?,
            days_of_week: Field::parse(parts[5], 0, 6)?,
        })
    }

    /// Returns `true` if the current wall-clock time (UTC) matches this schedule.
    pub fn matches_now(&self) -> bool {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.matches_epoch(secs)
    }

    /// Returns `true` if the given Unix timestamp (seconds since epoch, UTC) matches.
    pub fn matches_epoch(&self, epoch_secs: u64) -> bool {
        let (sec, min, hour, day, month, dow) = epoch_to_datetime(epoch_secs);
        self.seconds.matches(sec)
            && self.minutes.matches(min)
            && self.hours.matches(hour)
            && self.days_of_month.matches(day)
            && self.months.matches(month)
            && self.days_of_week.matches(dow)
    }
}

/// Decompose a Unix timestamp (UTC) into `(second, minute, hour, day, month, day_of_week)`.
/// `day` is 1-based, `month` is 1-based, `dow` is 0=Sunday..6=Saturday.
pub(crate) fn epoch_to_datetime(epoch_secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    let sec = (epoch_secs % 60) as u32;
    let mins_total = epoch_secs / 60;
    let min = (mins_total % 60) as u32;
    let hours_total = mins_total / 60;
    let hour = (hours_total % 24) as u32;
    let days_total = hours_total / 24;

    // 1970-01-01 was a Thursday (4)
    let dow = ((days_total + 4) % 7) as u32;

    let (_, month, day) = days_to_ymd(days_total);
    (sec, min, hour, day, month, dow)
}

/// Gregorian calendar decomposition of days-since-1970-01-01 into (year, month, day).
/// Uses Howard Hinnant's civil-from-days algorithm.
pub(crate) fn days_to_ymd(days: u64) -> (u32, u32, u32) {
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as u32, m as u32, d as u32)
}
