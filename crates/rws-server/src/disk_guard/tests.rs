use super::*;

#[test]
fn floors_parse_as_sizes_or_percentages() {
    assert_eq!(parse_floor(""), Some(Floor::Default));
    assert_eq!(parse_floor("2G"), Some(Floor::Bytes(2 << 30)));
    assert_eq!(parse_floor(" 500M "), Some(Floor::Bytes(500 << 20)));
    assert_eq!(parse_floor("10%"), Some(Floor::Percent(10)));
    assert_eq!(parse_floor("0"), Some(Floor::Off));
    assert_eq!(parse_floor("0%"), Some(Floor::Off));
    assert_eq!(parse_floor("150%"), None);
    assert_eq!(parse_floor("lots"), None);
}

#[test]
fn the_default_is_one_percent_but_at_least_a_gibibyte() {
    assert_eq!(required(Floor::Default, 400 << 30), 4 << 30);
    assert_eq!(required(Floor::Default, 10 << 30), 1 << 30, "1 GiB on a small disk");
    assert_eq!(required(Floor::Default, 1 << 30), 512 << 20, "never more than half of a tiny disk");
    assert_eq!(required(Floor::Percent(10), 1000), 100);
    assert_eq!(required(Floor::Bytes(7), 1000), 7);
    assert_eq!(required(Floor::Off, 1000), 0);
}

#[cfg(all(unix, any(feature = "http1", feature = "http2")))]
#[test]
fn a_disk_under_the_floor_has_no_room_until_space_returns() {
    let dir = crate::test_env::TempPath::new("disk_guard_dir");
    std::fs::create_dir_all(&*dir).unwrap();
    let file = dir.join("access.log");
    assert!(space(&dir).is_some_and(|(free, total)| total > 0 && free <= total));
    assert!(!has_room_with(&file, Floor::Bytes(u64::MAX)), "nothing has that much free");
    let reading = status().into_iter().find(|(d, ..)| *d == dir.display().to_string()).unwrap();
    assert!(reading.3, "reported low");
    assert!(prometheus_text().contains(&format!("rws_disk_low{{dir=\"{}\"}} 1", dir.display())));
    assert!(has_room_with(&file, Floor::Bytes(1)), "space back");
    assert!(has_room_with(&file, Floor::Off));
    assert!(has_room_with(Path::new("/no/such/dir/at/all/x.log"), Floor::Bytes(u64::MAX)), "unreadable: don't block writes");
}
