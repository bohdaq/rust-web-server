use super::cron::{CronSchedule, days_to_ymd, epoch_to_datetime, Field};
use super::Scheduler;
use std::time::Duration;

// ── Field parsing ─────────────────────────────────────────────────────────────

#[test]
fn field_any() {
    let f = Field::parse("*", 0, 59).unwrap();
    assert!(f.matches(0));
    assert!(f.matches(59));
    assert!(f.matches(30));
}

#[test]
fn field_exact() {
    let f = Field::parse("5", 0, 59).unwrap();
    assert!(f.matches(5));
    assert!(!f.matches(0));
    assert!(!f.matches(6));
}

#[test]
fn field_step() {
    let f = Field::parse("*/15", 0, 59).unwrap();
    assert!(f.matches(0));
    assert!(f.matches(15));
    assert!(f.matches(30));
    assert!(f.matches(45));
    assert!(!f.matches(1));
    assert!(!f.matches(16));
}

#[test]
fn field_range() {
    let f = Field::parse("1-5", 0, 59).unwrap();
    for v in 1..=5u32 { assert!(f.matches(v)); }
    assert!(!f.matches(0));
    assert!(!f.matches(6));
}

#[test]
fn field_list() {
    let f = Field::parse("0,15,30,45", 0, 59).unwrap();
    assert!(f.matches(0));
    assert!(f.matches(15));
    assert!(f.matches(30));
    assert!(f.matches(45));
    assert!(!f.matches(1));
}

#[test]
fn field_combined() {
    // "0,10-12,*/20" on 0-59 → {0, 10, 11, 12, 20, 40}
    let f = Field::parse("0,10-12,*/20", 0, 59).unwrap();
    for v in [0u32, 10, 11, 12, 20, 40] { assert!(f.matches(v), "should match {}", v); }
    assert!(!f.matches(1));
    assert!(!f.matches(13));
}

#[test]
fn field_error_out_of_range() {
    assert!(Field::parse("60", 0, 59).is_err());
    assert!(Field::parse("0-60", 0, 59).is_err());
}

#[test]
fn field_error_zero_step() {
    assert!(Field::parse("*/0", 0, 59).is_err());
}

// ── Calendar arithmetic ───────────────────────────────────────────────────────

#[test]
fn days_to_ymd_epoch_zero() {
    assert_eq!(days_to_ymd(0), (1970, 1, 1));
}

#[test]
fn days_to_ymd_2024_jan_01() {
    // 2024-01-01 = 19723 days since 1970-01-01
    assert_eq!(days_to_ymd(19723), (2024, 1, 1));
}

#[test]
fn days_to_ymd_leap_day_2024() {
    // 2024-02-29 = 19723 + 59 = 19782
    assert_eq!(days_to_ymd(19782), (2024, 2, 29));
}

#[test]
fn epoch_to_datetime_midnight_2024_jan_01() {
    // 2024-01-01 00:00:00 UTC = 1704067200
    let (sec, min, hour, day, month, dow) = epoch_to_datetime(1704067200);
    assert_eq!(sec, 0);
    assert_eq!(min, 0);
    assert_eq!(hour, 0);
    assert_eq!(day, 1);
    assert_eq!(month, 1);
    assert_eq!(dow, 1); // Monday
}

#[test]
fn epoch_to_datetime_known_time() {
    // 2024-06-15 13:45:30 UTC
    // 2024-06-15 00:00:00 UTC = 1718409600
    // + 13*3600 + 45*60 + 30 = 49530
    // epoch = 1718459130
    let (sec, min, hour, day, month, _dow) = epoch_to_datetime(1718459130);
    assert_eq!(sec, 30);
    assert_eq!(min, 45);
    assert_eq!(hour, 13);
    assert_eq!(day, 15);
    assert_eq!(month, 6);
}

// ── CronSchedule parsing ──────────────────────────────────────────────────────

#[test]
fn cron_parse_all_stars() {
    let s = CronSchedule::parse("* * * * * *").unwrap();
    assert!(s.matches_epoch(0));
    assert!(s.matches_epoch(1704067200));
}

#[test]
fn cron_parse_exact_midnight_jan1() {
    // Fires only at 00:00:00 on Jan 1 (any year, any weekday)
    let s = CronSchedule::parse("0 0 0 1 1 *").unwrap();
    assert!(s.matches_epoch(1704067200)); // 2024-01-01 00:00:00
    assert!(!s.matches_epoch(1704067201)); // one second later
    assert!(!s.matches_epoch(1706745600)); // 2024-02-01 00:00:00
}

#[test]
fn cron_parse_every_minute() {
    let s = CronSchedule::parse("0 * * * * *").unwrap();
    // 1704067200 is at second 0 of its minute
    assert!(s.matches_epoch(1704067200));
    assert!(!s.matches_epoch(1704067201));
}

#[test]
fn cron_parse_every_quarter_hour() {
    // Fires at minutes 0, 15, 30, 45 of every hour
    let s = CronSchedule::parse("0 0,15,30,45 * * * *").unwrap();
    // 1704067200 = 2024-01-01 00:00:00 → sec=0, min=0 → matches
    assert!(s.matches_epoch(1704067200));
    // 1704067200 + 15*60 = 1704068100 → sec=0, min=15 → matches
    assert!(s.matches_epoch(1704068100));
    // 1704067200 + 5*60 = 1704067500 → sec=0, min=5 → no match
    assert!(!s.matches_epoch(1704067500));
}

#[test]
fn cron_parse_weekday_only() {
    // Monday only (dow=1); 2024-01-01 is Monday
    let s = CronSchedule::parse("0 0 0 * * 1").unwrap();
    assert!(s.matches_epoch(1704067200)); // 2024-01-01 Mon
    // 2024-01-02 is Tuesday: 1704067200 + 86400 = 1704153600
    assert!(!s.matches_epoch(1704153600));
}

#[test]
fn cron_parse_error_wrong_field_count() {
    assert!(CronSchedule::parse("* * * * *").is_err());   // 5 fields
    assert!(CronSchedule::parse("* * * * * * *").is_err()); // 7 fields
}

#[test]
fn cron_parse_error_invalid_value() {
    assert!(CronSchedule::parse("60 * * * * *").is_err()); // sec out of range
    assert!(CronSchedule::parse("* 60 * * * *").is_err()); // min out of range
    assert!(CronSchedule::parse("* * 24 * * *").is_err()); // hour out of range
    assert!(CronSchedule::parse("* * * 0 * *").is_err());  // day < 1
    assert!(CronSchedule::parse("* * * * 0 *").is_err());  // month < 1
    assert!(CronSchedule::parse("* * * * * 7").is_err());  // weekday > 6
}

// ── Scheduler builder ─────────────────────────────────────────────────────────

#[test]
fn scheduler_registers_tasks() {
    let s = Scheduler::new()
        .every(Duration::from_secs(60), || {})
        .after(Duration::from_secs(30), || {})
        .cron("0 * * * * *", || {}).unwrap();
    assert_eq!(s.tasks.len(), 3);
}

#[test]
fn scheduler_initial_delay_applies_to_last_task() {
    let s = Scheduler::new()
        .every(Duration::from_secs(60), || {})
        .initial_delay(Duration::from_secs(5));
    assert_eq!(s.tasks[0].initial_delay, Duration::from_secs(5));
}

#[test]
fn scheduler_default() {
    let s: Scheduler = Default::default();
    assert_eq!(s.tasks.len(), 0);
}
