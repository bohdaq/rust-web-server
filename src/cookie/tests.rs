use super::{Cookie, CookieJar, SetCookie};

#[test]
fn parses_single_cookie() {
    let jar = CookieJar::parse("session=abc123");
    assert_eq!(jar.cookies.len(), 1);
    assert_eq!(jar.cookies[0], Cookie { name: "session".into(), value: "abc123".into() });
}

#[test]
fn parses_multiple_cookies() {
    let jar = CookieJar::parse("session=abc123; theme=dark; lang=en");
    assert_eq!(jar.cookies.len(), 3);
    assert_eq!(jar.get("theme").unwrap().value, "dark");
    assert_eq!(jar.get("lang").unwrap().value, "en");
}

#[test]
fn get_returns_none_for_missing_cookie() {
    let jar = CookieJar::parse("a=1");
    assert!(jar.get("missing").is_none());
}

#[test]
fn cookie_value_may_contain_equals() {
    // values like base64 tokens may contain '='
    let jar = CookieJar::parse("token=abc=def==");
    assert_eq!(jar.get("token").unwrap().value, "abc=def==");
}

#[test]
fn set_cookie_basic() {
    let s = SetCookie::new("id", "42").build();
    assert_eq!(s, "id=42");
}

#[test]
fn set_cookie_all_attributes() {
    let s = SetCookie::new("session", "xyz")
        .path("/")
        .domain("example.com")
        .max_age(3600)
        .secure()
        .http_only()
        .same_site("Strict")
        .build();
    assert_eq!(s, "session=xyz; Path=/; Domain=example.com; Max-Age=3600; Secure; HttpOnly; SameSite=Strict");
}

#[test]
fn set_cookie_http_only_secure() {
    let s = SetCookie::new("tok", "abc")
        .http_only()
        .secure()
        .build();
    assert!(s.contains("HttpOnly"));
    assert!(s.contains("Secure"));
}
