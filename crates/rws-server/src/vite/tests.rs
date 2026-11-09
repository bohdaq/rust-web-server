use super::*;

const MANIFEST: &str = r#"{
  "src/main.ts": {"file": "assets/main.js", "isEntry": true, "css": ["assets/main.css"], "imports": ["_a.js", "_b.js"]},
  "_a.js": {"file": "assets/a.js", "css": ["assets/a.css"], "imports": ["_b.js"]},
  "_b.js": {"file": "assets/b.js", "imports": ["_a.js"]},
  "src/logo.svg": {"file": "assets/logo-12ab.svg"}
}"#;

#[test]
fn manifest_tags_follow_imports_once() {
    let vite = Vite::from_manifest_str(MANIFEST, "/static").unwrap();
    let tags = vite.tags("src/main.ts").unwrap();
    let lines: Vec<&str> = tags.lines().collect();
    assert_eq!(
        vec![
            r#"<link rel="stylesheet" href="/static/assets/main.css">"#,
            r#"<link rel="stylesheet" href="/static/assets/a.css">"#,
            r#"<link rel="modulepreload" href="/static/assets/b.js">"#,
            r#"<link rel="modulepreload" href="/static/assets/a.js">"#,
            r#"<script type="module" src="/static/assets/main.js"></script>"#,
        ],
        lines
    );
    assert_eq!(None, vite.tags("src/other.ts"));
    assert_eq!(Some("/static/assets/logo-12ab.svg".to_string()), vite.asset("src/logo.svg"));
}

#[test]
fn bad_manifests_are_errors() {
    assert!(Vite::from_manifest_str("not json", "/").is_err());
    assert!(Vite::from_manifest_str(r#"{"a": {"css": []}}"#, "/").is_err());
    assert!(Vite::from_manifest_file("/no/such/manifest.json", "/").is_err());
}

#[test]
fn dev_server_tags_escape_their_input() {
    let vite = Vite::dev("http://localhost:5173/");
    assert_eq!(
        "<script type=\"module\" src=\"http://localhost:5173/@vite/client\"></script>\n<script type=\"module\" src=\"http://localhost:5173/src/a&quot;b.ts\"></script>",
        vite.tags("/src/a\"b.ts").unwrap()
    );
    assert_eq!(Some("http://localhost:5173/src/logo.svg".to_string()), vite.asset("src/logo.svg"));
}

#[test]
fn installed_helper_answers_tags() {
    install(Vite::from_manifest_str(MANIFEST, "/").unwrap());
    assert!(tags("src/main.ts").unwrap().ends_with(r#"<script type="module" src="/assets/main.js"></script>"#));
    assert_eq!(Some("/assets/logo-12ab.svg".to_string()), asset("src/logo.svg"));
}
