use std::collections::HashSet;

/// One field of a 6-part cron expression.
#[derive(Debug, Clone)]
#[doc(hidden)]
pub enum Field {
    Any,
    Values(HashSet<u32>),
}

impl Field {
    #[doc(hidden)]
    pub fn matches(&self, v: u32) -> bool {
        match self {
            Field::Any => true,
            Field::Values(set) => set.contains(&v),
        }
    }

    /// Parse one cron field. Supports `*`, exact values, `*/step`, `N-M` ranges,
    /// and comma-separated combinations of the above.
    #[doc(hidden)]
    pub fn parse(s: &str, min: u32, max: u32) -> Result<Self, String> {
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
    #[doc(hidden)]
    pub seconds: Field,
    #[doc(hidden)]
    pub minutes: Field,
    #[doc(hidden)]
    pub hours: Field,
    #[doc(hidden)]
    pub days_of_month: Field,
    #[doc(hidden)]
    pub months: Field,
    #[doc(hidden)]
    pub days_of_week: Field,
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

    /// The first second after `epoch_secs` (UTC) that matches, within the
    /// next five years; `None` for a schedule that never fires in that time
    /// (`0 0 0 31 2 *`).
    pub fn next_after(&self, epoch_secs: u64) -> Option<u64> {
        let mut t = epoch_secs.checked_add(1)?;
        let limit = epoch_secs.saturating_add(5 * 366 * 86_400);
        while t <= limit {
            let (sec, min, hour, day, month, dow) = epoch_to_datetime(t);
            if !(self.months.matches(month) && self.days_of_month.matches(day) && self.days_of_week.matches(dow)) {
                t = (t / 86_400 + 1) * 86_400;
            } else if !self.hours.matches(hour) {
                t = (t / 3_600 + 1) * 3_600;
            } else if !self.minutes.matches(min) {
                t = (t / 60 + 1) * 60;
            } else if !self.seconds.matches(sec) {
                t += 1;
            } else {
                return Some(t);
            }
        }
        None
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
#[doc(hidden)]
pub fn epoch_to_datetime(epoch_secs: u64) -> (u32, u32, u32, u32, u32, u32) {
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

// Calendar arithmetic lives in `http::date` (rws-base); cron and SAML use it.
use crate::http::date::days_to_ymd;
