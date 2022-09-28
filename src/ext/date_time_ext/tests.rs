use chrono::DateTime;
use crate::ext::date_time_ext::DateTimeExt;


#[test]
fn base() {
    let now_rfc2822 = DateTimeExt::_now_rfc2822();
    assert!(DateTime::parse_from_rfc2822(&now_rfc2822).is_ok());
}