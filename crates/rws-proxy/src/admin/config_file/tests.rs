use std::cell::Cell;

use super::*;
use crate::test_env::TempPath;

const BEFORE: &str = "port = 7878\n\n[rate_limit]\nmax_requests = 100\n";
const AFTER: &str = "port = 7878\n\n[rate_limit]\nmax_requests = 250\n";

fn file(name: &str, text: &str) -> TempPath {
    let path = TempPath::new(name);
    std::fs::write(&*path, text).unwrap();
    path
}

fn text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

fn sha(text: &str) -> String {
    sha256_hex(text.as_bytes())
}

/// A reload that counts its calls and answers what `outcome` says.
fn reload<'a>(calls: &'a Cell<u32>, outcome: impl Fn(u32) -> Reload + 'a) -> impl Fn() -> Reload + 'a {
    move || {
        calls.set(calls.get() + 1);
        outcome(calls.get())
    }
}

#[test]
fn a_valid_change_replaces_the_file_keeps_the_previous_one_and_reloads() {
    let path = file("config_file_write.toml", BEFORE);
    let _bak = TempPath::new("config_file_write.toml.bak");
    let calls = Cell::new(0);
    let written = write_at(&path, AFTER, &sha(BEFORE), &reload(&calls, |_| Reload::Done)).unwrap();
    assert_eq!(text(&path), AFTER);
    assert_eq!(calls.get(), 1);
    assert!(written.changed);
    assert_eq!(written.sha256, sha(AFTER));
    let backup = written.backup.clone().expect("the previous file is kept");
    assert!(backup.ends_with("config_file_write.toml.bak"), "{backup}");
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), BEFORE);
    assert!(written.restart.is_empty(), "{:?}", written.restart);
    assert!(!beside(&path, ".rws-admin.tmp").exists(), "the temporary file is gone");
    let json = written.to_json();
    assert!(json.starts_with(r#"{"applied":true,"changed":true,"path":""#), "{json}");
    assert!(json.contains(&format!(r#""sha256":"{}""#, sha(AFTER))), "{json}");
}

#[cfg(unix)]
#[test]
fn the_new_file_and_the_backup_keep_the_files_mode() {
    use std::os::unix::fs::PermissionsExt;
    let path = file("config_file_mode.toml", BEFORE);
    let _bak = TempPath::new("config_file_mode.toml.bak");
    std::fs::set_permissions(&*path, std::fs::Permissions::from_mode(0o640)).unwrap();
    let calls = Cell::new(0);
    write_at(&path, AFTER, &sha(BEFORE), &reload(&calls, |_| Reload::Done)).unwrap();
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&path), 0o640);
    assert_eq!(mode(&beside(&path, ".bak")), 0o640);
}

#[test]
fn a_file_changed_since_it_was_read_is_not_overwritten() {
    let path = file("config_file_stale.toml", BEFORE);
    let calls = Cell::new(0);
    let refusal = write_at(&path, AFTER, &sha("port = 1\n"), &reload(&calls, |_| Reload::Done)).unwrap_err();
    let (status, body) = refusal.answer();
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("changed on disk since it was read"), "{body}");
    assert_eq!(text(&path), BEFORE);
    assert_eq!(calls.get(), 0);
    assert!(!beside(&path, ".bak").exists());
}

#[test]
fn a_configuration_with_errors_is_not_written() {
    let path = file("config_file_invalid.toml", BEFORE);
    let calls = Cell::new(0);
    let broken = "port = 7878\n[rate_limit\nmax_requests = 250\n";
    let refusal = write_at(&path, broken, &sha(BEFORE), &reload(&calls, |_| Reload::Done)).unwrap_err();
    let (status, body) = refusal.answer();
    assert_eq!(status, 422, "{body}");
    assert!(body.contains(r#""errors":[""#), "{body}");
    assert!(body.contains("line 2"), "the error names the line: {body}");
    assert_eq!(text(&path), BEFORE);
    assert_eq!(calls.get(), 0);
}

#[test]
fn the_same_text_writes_nothing() {
    let path = file("config_file_same.toml", BEFORE);
    let calls = Cell::new(0);
    let written = write_at(&path, BEFORE, &sha(BEFORE), &reload(&calls, |_| Reload::Done)).unwrap();
    assert!(!written.changed);
    assert_eq!(written.backup, None);
    assert_eq!(calls.get(), 0);
    assert!(!beside(&path, ".bak").exists());
}

#[test]
fn a_refused_reload_puts_the_previous_file_back() {
    let path = file("config_file_refused.toml", BEFORE);
    let _bak = TempPath::new("config_file_refused.toml.bak");
    let calls = Cell::new(0);
    let outcome = |call| if call == 1 { Reload::Refused("route 'api': no such upstream".to_string()) } else { Reload::Done };
    let refusal = write_at(&path, AFTER, &sha(BEFORE), &reload(&calls, outcome)).unwrap_err();
    let (status, body) = refusal.answer();
    assert_eq!(status, 500, "{body}");
    assert!(body.contains("no such upstream") && body.contains("the previous file is back"), "{body}");
    assert_eq!(text(&path), BEFORE);
    assert_eq!(calls.get(), 2, "reloaded again with the previous file");
}

#[test]
fn a_reload_that_does_not_answer_leaves_the_new_file_and_says_so() {
    let path = file("config_file_unknown.toml", BEFORE);
    let _bak = TempPath::new("config_file_unknown.toml.bak");
    let calls = Cell::new(0);
    let refusal = write_at(&path, AFTER, &sha(BEFORE), &reload(&calls, |_| Reload::Unknown("the reload didn't finish in time".to_string()))).unwrap_err();
    let (status, body) = refusal.answer();
    assert_eq!(status, 504, "{body}");
    assert!(body.contains("is written, but the reload didn't finish in time"), "{body}");
    assert_eq!(text(&path), AFTER);
}

#[test]
fn a_huge_configuration_is_refused() {
    let path = file("config_file_large.toml", BEFORE);
    let huge = format!("# {}\n", "x".repeat(MAX_BYTES));
    let calls = Cell::new(0);
    assert_eq!(write_at(&path, &huge, &sha(BEFORE), &reload(&calls, |_| Reload::Done)).unwrap_err().answer().0, 413);
    assert_eq!(check_at(&path, &huge).unwrap_err().answer().0, 413);
    assert_eq!(text(&path), BEFORE);
}

#[test]
fn reading_gives_the_text_and_its_hash() {
    let path = file("config_file_read.toml", "# \"quoted\"\nport = 7878\n");
    let json = read_at(&path).unwrap();
    assert!(json.contains(r#""editable":true"#), "{json}");
    assert!(json.contains(r##""content":"# \"quoted\"\nport = 7878\n""##), "{json}");
    assert!(json.contains(&format!(r#""sha256":"{}""#, sha("# \"quoted\"\nport = 7878\n"))), "{json}");
    assert!(read_at(&TempPath::new("config_file_missing.toml")).unwrap_err().contains("cannot read"));
}

#[test]
fn a_check_reports_errors_warnings_and_what_waits_for_a_restart() {
    let path = file("config_file_check.toml", BEFORE);
    let good = check_at(&path, "port = 9000\n\n[rate_limit]\nmax_requests = 250\n").unwrap();
    assert!(good.starts_with(r#"{"ok":true,"errors":[],"#), "{good}");
    assert!(good.ends_with(r#""restart":["port"]}"#), "{good}");
    let bad = check_at(&path, "port = \n").unwrap();
    assert!(bad.starts_with(r#"{"ok":false,"errors":[""#), "{bad}");
    assert_eq!(text(&path), BEFORE, "a check writes nothing");
}

#[test]
fn what_a_reload_does_not_apply() {
    // A changed value of a hot setting: nothing waits.
    assert!(needs_restart(BEFORE, AFTER).is_empty());
    // Comments and blank lines aren't changes.
    assert!(needs_restart(BEFORE, &format!("# a note\n\n{BEFORE}")).is_empty());
    assert_eq!(needs_restart("port = 1\nip = \"127.0.0.1\"\n", "port = 2\nip = \"127.0.0.1\"\n"), ["port"]);
    assert_eq!(needs_restart("", "watch = true\nthread_count = 8\n"), ["thread_count", "watch"]);
    // The admin listener is set up once; the two settings read per request aren't.
    assert_eq!(needs_restart("[admin]\nlisten = \"127.0.0.1:1\"\n", "[admin]\nlisten = \"127.0.0.1:2\"\nedit_config = true\ncount_own_requests = true\n"), ["admin_listen"]);
    // A setting taken out keeps its value until a restart.
    assert_eq!(needs_restart(BEFORE, "port = 7878\n"), ["rate_limit_max_requests (removed: it keeps its value until a restart)"]);
    // Sections with their own listeners.
    let tcp = "[[tcp_proxy]]\nname = \"db\"\nlisten = \"127.0.0.1:5433\"\nbackends = [\"127.0.0.1:5432\"]\n";
    assert_eq!(needs_restart(tcp, &tcp.replace("5433", "5434")), ["[[tcp_proxy]]"]);
    assert_eq!(needs_restart("", tcp), ["[[tcp_proxy]]"]);
    assert!(needs_restart(tcp, &format!("{tcp}# the database\n\n[[route]]\nname = \"a\"\n")).is_empty());
}

#[test]
fn the_body_text_is_decoded_strictly() {
    let body = br#"{"sha256":"abc","content":"a = \"b\"\n# caf\u00e9 \ud83d\ude00 \\ \/\t\b\f\r"}"#;
    assert_eq!(body_text(body, "content").unwrap(), "a = \"b\"\n# caf\u{e9} \u{1F600} \\ /\t\u{8}\u{c}\r");
    assert_eq!(body_text(body, "sha256").unwrap(), "abc");
    assert_eq!(body_text("{\"content\":\"żółw 🐢\"}".as_bytes(), "content").unwrap(), "żółw 🐢");
    for bad in [
        r#"{"content":"\ud83d"}"#,
        r#"{"content":"\ud83d\u0041"}"#,
        r#"{"content":"\ude00"}"#,
        r#"{"content":"\x41"}"#,
        r#"{"content":"\u12"}"#,
        r#"{"content":7}"#,
        r#"{"content":null}"#,
        r#"{"other":"x"}"#,
        r#"["content"]"#,
        "",
    ] {
        assert!(body_text(bad.as_bytes(), "content").is_err(), "{bad}");
    }
    assert!(body_text(&[b'{', 0xff, b'}'], "content").is_err());
}

#[test]
fn the_audit_log_gets_the_hash_not_the_text() {
    let body = br#"{"content":"[admin]\ntoken = \"hunter2\"\n","sha256":"0123abcd"}"#;
    let detail = audit_detail(body);
    assert!(!detail.contains("hunter2") && !detail.contains("token"), "{detail}");
    assert_eq!(detail, format!(r#"{{"based_on":"0123abcd","sha256":"{}","bytes":26}}"#, sha("[admin]\ntoken = \"hunter2\"\n")));
    assert_eq!(audit_detail(b"port = 1 secret"), r#"{"content":"(unreadable)"}"#);
}

#[test]
fn off_unless_the_setting_is_true() {
    let _g = crate::test_env::lock();
    let before = std::env::var(RWS_CONFIG_ADMIN_EDIT_CONFIG).ok();
    std::env::remove_var(RWS_CONFIG_ADMIN_EDIT_CONFIG);
    assert!(!enabled());
    let calls = Cell::new(0);
    assert_eq!(write("port = 1\n", "x", &reload(&calls, |_| Reload::Done)).unwrap_err().answer().0, 403);
    assert_eq!(check_json("port = 1\n").unwrap_err().answer().0, 403);
    let json = read_json();
    assert!(json.starts_with(r#"{"enabled":false,"#) && json.contains(r#""content":null"#) && json.contains("edit_config = true"), "{json}");
    std::env::set_var(RWS_CONFIG_ADMIN_EDIT_CONFIG, "TRUE");
    assert!(enabled());
    std::env::set_var(RWS_CONFIG_ADMIN_EDIT_CONFIG, "yes");
    assert!(!enabled());
    match before {
        Some(value) => std::env::set_var(RWS_CONFIG_ADMIN_EDIT_CONFIG, value),
        None => std::env::remove_var(RWS_CONFIG_ADMIN_EDIT_CONFIG),
    }
}
