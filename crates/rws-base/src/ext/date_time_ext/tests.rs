use std::time::SystemTime;
use crate::ext::date_time_ext::DateTimeExt;


#[test]
fn system_to_nanos() {
    let now = SystemTime::now();
    let nanos = DateTimeExt::_system_time_to_unix_nanos(now);
    assert_ne!(nanos, 0);
}

#[test]
fn now_as_nanos() {
    let nanos = DateTimeExt::_now_unix_epoch_nanos();
    assert_ne!(nanos, 0);
}