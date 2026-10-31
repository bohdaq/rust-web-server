use super::*;
use crate::header::Header;

fn catalogs() -> Catalogs {
    Catalogs::new("en")
        .add("en", "# English\ngreeting = \"Hello, {name}!\"\nonly_en = 'raw \\n text' # literal\nitems.one = \"{count} item\"\nitems.other = \"{count} items\"\nbraces = \"{{name}} is {name}\"\n")
        .unwrap()
        .add("en-GB", "colour = \"colour\"\n")
        .unwrap()
        .add("uk", "greeting = \"Привіт, {name}!\"\nitems.one = \"{count} річ\"\nitems.few = \"{count} речі\"\nitems.many = \"{count} речей\"\nescaped = \"a\\tb \\\"c\\\" \\u00e9\"\n")
        .unwrap()
}

#[test]
fn accept_language_orders_by_q_then_header_order() {
    assert_eq!(AcceptLanguage(vec!["uk-ua".into(), "uk".into(), "en".into(), "*".into()]), AcceptLanguage::parse("en;q=0.5, uk-UA, *;q=0.1, uk;q=0.9"));
    assert_eq!(AcceptLanguage(vec!["de".into()]), AcceptLanguage::parse("fr;q=0, de;q=0.000001, de"));
    assert_eq!(AcceptLanguage(vec!["de".into()]), AcceptLanguage::parse("bad tag, x;q=2, y;q=abc, de, ;q=1, <script>"));
    assert_eq!(AcceptLanguage::default(), AcceptLanguage::parse(""));
    let long = vec!["en"; 1000].join(",");
    assert_eq!(MAX_LANGUAGE_RANGES, AcceptLanguage::parse(&long).0.len());
}

#[test]
fn negotiation_is_rfc_4647_lookup_then_a_regional_match() {
    let available = ["en", "en-GB", "uk", "pt-BR"];
    let pick = |header: &str| AcceptLanguage::parse(header).negotiate(&available);
    assert_eq!(Some("en-GB"), pick("en-GB"));
    assert_eq!(Some("uk"), pick("uk-UA"));
    assert_eq!(Some("en"), pick("en-US"));
    assert_eq!(Some("pt-BR"), pick("pt"));
    assert_eq!(Some("uk"), pick("fr, uk;q=0.2"));
    assert_eq!(None, pick("fr, *"));
    assert_eq!(None, pick(""));
}

#[test]
fn messages_fall_back_and_fill_placeholders() {
    let c = catalogs();
    let uk = c.translator("uk-UA,uk;q=0.9");
    assert_eq!("uk", uk.locale());
    assert_eq!("Привіт, Олена!", uk.t_with("greeting", &[("name", "Олена")]));
    assert_eq!("raw \\n text", uk.t("only_en"), "missing in uk: the default's");
    assert_eq!("no.such.key", uk.t("no.such.key"));
    assert_eq!("a\tb \"c\" é", uk.t("escaped"));

    let gb = c.translator("en-GB");
    assert_eq!("colour", gb.t("colour"));
    assert_eq!("Hello, {name}!", gb.t("greeting"), "base language, unknown placeholder kept");
    assert_eq!("{name} is x", gb.t_with("braces", &[("name", "x")]));

    assert_eq!("en", c.translator("fr").locale(), "nothing matches: the default");
    assert_eq!("de", c.for_language("de").locale());
    assert_eq!("Hello, A!", c.for_language("de").t_with("greeting", &[("name", "A")]));
}

#[test]
fn plurals_follow_each_language() {
    let c = catalogs();
    let uk = c.for_language("uk");
    let en = c.for_language("en");
    assert_eq!(["1 річ", "3 речі", "5 речей", "11 речей", "21 річ", "22 речі", "112 речей"], [1, 3, 5, 11, 21, 22, 112].map(|n| uk.plural("items", n)));
    assert_eq!(["0 items", "1 item", "2 items"], [0, 1, 2].map(|n| en.plural("items", n)));
    assert_eq!("missing.other", en.plural("missing", 2));
    assert_eq!(("one", "few", "many", "many"), (plural_category("pl", 1), plural_category("pl", 23), plural_category("pl", 21), plural_category("pl", 12)));
    assert_eq!(("one", "one", "other"), (plural_category("fr", 0), plural_category("pt-BR", 1), plural_category("pt-PT", 0)));
    assert_eq!(("few", "other"), (plural_category("cs", 4), plural_category("cs", 5)));
    assert_eq!("other", plural_category("ja", 1));
}

#[test]
fn catalog_errors_name_the_line() {
    for (text, expect) in [
        ("a = 1", "line 1: the value must be quoted"),
        ("\n\nkey", "line 3: expected key"),
        ("a b = \"x\"", "isn't a key"),
        ("a = \"open", "no closing quote"),
        ("a = \"x\" trailing", "unexpected"),
        ("a = \"\\q\"", "unknown escape"),
        ("a = \"\\uZZZZ\"", "bad \\u escape"),
    ] {
        let e = Catalog::parse(text).unwrap_err();
        assert!(e.contains(expect), "{text:?}: {e}");
    }
    let e = Catalogs::new("en").add("fr", "x").unwrap_err();
    assert!(e.starts_with("fr: line 1"), "{e}");
}

#[test]
fn catalogs_load_from_a_directory() {
    let dir = crate::test_env::TempPath::new("i18n_catalogs");
    std::fs::create_dir_all(&*dir).unwrap();
    std::fs::write(dir.join("en.toml"), "hi = \"Hi\"\n").unwrap();
    std::fs::write(dir.join("uk.toml"), "hi = \"Привіт\"\n").unwrap();
    std::fs::write(dir.join("notes.txt"), "ignored").unwrap();
    let c = Catalogs::from_dir(&*dir, "en").unwrap();
    assert_eq!(vec!["en", "uk"], c.languages());
    assert_eq!("Привіт", c.translator("uk").t("hi"));
    std::fs::write(dir.join("bad.toml"), "oops").unwrap();
    assert!(Catalogs::from_dir(&*dir, "en").unwrap_err().contains("bad.toml"));
    let empty = crate::test_env::TempPath::new("i18n_empty");
    std::fs::create_dir_all(&*empty).unwrap();
    assert!(Catalogs::from_dir(&*empty, "en").unwrap_err().contains("no <language>.toml"));
}

#[test]
fn requests_and_responses() {
    let c = catalogs();
    let request = Request::parse(b"GET / HTTP/1.1\r\nHost: a\r\nAccept-Language: uk, en;q=0.8\r\n\r\n").unwrap();
    use crate::extract::FromRequest;
    assert_eq!(AcceptLanguage(vec!["uk".into(), "en".into()]), <AcceptLanguage as FromRequest>::from_request(&request).unwrap());
    let t = c.for_request(&request);
    let mut response = Response::text(crate::response::STATUS_CODE_REASON_PHRASE.n200_ok, &t.t_with("greeting", &[("name", "A")]));
    response.headers.push(Header::new(Header::_VARY, "Accept-Encoding".to_string()));
    let response = t.localize(t.localize(response));
    let header = |name: &str| response.headers.iter().filter(|h| h.name.eq_ignore_ascii_case(name)).map(|h| h.value.to_string()).collect::<Vec<_>>();
    assert_eq!(vec!["uk"], header("content-language"));
    assert_eq!(vec!["Accept-Encoding, Accept-Language"], header("vary"));
    let bare = Request::parse(b"GET / HTTP/1.1\r\nHost: a\r\n\r\n").unwrap();
    assert_eq!("en", c.for_request(&bare).locale());
}
