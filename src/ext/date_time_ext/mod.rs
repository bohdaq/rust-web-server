use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
mod tests;

pub struct DateTimeExt;

impl DateTimeExt {
    pub fn _now_unix_epoch_nanos() -> u128 {
        let now = SystemTime::now();
        let nanos = DateTimeExt::_system_time_to_unix_nanos(now);
        nanos
    }

    pub fn _system_time_to_unix_nanos(system_time: SystemTime) -> u128 {
        let boxed_duration = system_time.duration_since(UNIX_EPOCH);
        if boxed_duration.is_err() {
            eprintln!("unable to get duration from system time {}", boxed_duration.err().unwrap());
            let nanos = 0 as u128;
            return nanos
        }

        let nanos = boxed_duration.unwrap().as_nanos();
        nanos
    }

}