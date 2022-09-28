use chrono::{DateTime, Utc, Local};

#[cfg(test)]
mod tests;

pub struct DateTimeExt;

impl DateTimeExt {
    pub fn _now_utc() -> DateTime<Utc> {
        let current_utc: DateTime<Utc> = Utc::now();
        current_utc
    }

    pub fn _now_local() -> DateTime<Local> {
        let current_utc: DateTime<Local> = Local::now();
        current_utc
    }

    pub fn _now_rfc2822() -> String {
        let current_utc: DateTime<Utc> = Utc::now();
        let rfc2822 = current_utc.to_rfc2822();
        rfc2822
    }
}