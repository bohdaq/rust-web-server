use std::time::{SystemTime, UNIX_EPOCH};
use chrono::{DateTime, Utc, Local, NaiveDateTime};

#[cfg(test)]
mod tests;

pub struct DateTimeExt;

impl DateTimeExt {
    pub fn _now_utc() -> DateTime<Utc> {
        let current_utc: DateTime<Utc> = Utc::now();
        current_utc
    }

    pub fn _now_local() -> DateTime<Local> {
        let current_local: DateTime<Local> = Local::now();
        current_local
    }

    pub fn _now_rfc2822() -> String {
        let current_utc: DateTime<Utc> = Utc::now();
        let rfc2822 = current_utc.to_rfc2822();
        rfc2822
    }

    pub fn _system_time_to_utc(system_time: SystemTime) -> DateTime<Utc> {
        let seconds = system_time.duration_since(UNIX_EPOCH).unwrap().as_secs();
        let naive_datetime = NaiveDateTime::from_timestamp(seconds as i64, 1111);
        let datetime_utc: DateTime<Utc> = DateTime::from_utc(naive_datetime, Utc);
        datetime_utc
    }

}