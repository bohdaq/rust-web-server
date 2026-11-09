use super::{Context, TeraEngine};

// ── TeraEngine::from_raw ──────────────────────────────────────────────────────

#[test]
fn simple_variable_substitution() {
    let engine = TeraEngine::from_raw(&[("greeting.html", "<h1>Hello, {{ name }}!</h1>")]).unwrap();
    let mut ctx = Context::new();
    ctx.insert("name", "World");
    let out = engine.render("greeting.html", &ctx).unwrap();
    assert_eq!("<h1>Hello, World!</h1>", out);
}

#[test]
fn for_loop() {
    let engine = TeraEngine::from_raw(&[(
        "list.html",
        "{% for item in items %}{{ item }},{% endfor %}",
    )])
    .unwrap();
    let mut ctx = Context::new();
    ctx.insert("items", &["a", "b", "c"]);
    let out = engine.render("list.html", &ctx).unwrap();
    assert_eq!("a,b,c,", out);
}

#[test]
fn if_conditional() {
    let engine = TeraEngine::from_raw(&[(
        "cond.html",
        "{% if logged_in %}yes{% else %}no{% endif %}",
    )])
    .unwrap();

    let mut ctx = Context::new();
    ctx.insert("logged_in", &true);
    assert_eq!("yes", engine.render("cond.html", &ctx).unwrap());

    let mut ctx2 = Context::new();
    ctx2.insert("logged_in", &false);
    assert_eq!("no", engine.render("cond.html", &ctx2).unwrap());
}

#[test]
fn template_inheritance() {
    let engine = TeraEngine::from_raw(&[
        ("base.html", "HEADER {% block body %}{% endblock %} FOOTER"),
        ("child.html", "{% extends \"base.html\" %}{% block body %}CONTENT{% endblock %}"),
    ])
    .unwrap();
    let ctx = Context::new();
    let out = engine.render("child.html", &ctx).unwrap();
    assert_eq!("HEADER CONTENT FOOTER", out);
}

#[test]
fn number_variable() {
    let engine = TeraEngine::from_raw(&[("num.html", "{{ count }}")]).unwrap();
    let mut ctx = Context::new();
    ctx.insert("count", &42u32);
    assert_eq!("42", engine.render("num.html", &ctx).unwrap());
}

#[test]
fn missing_variable_renders_empty() {
    let engine = TeraEngine::from_raw(&[("t.html", "{{ missing | default(value=\"\") }}")]).unwrap();
    let ctx = Context::new();
    let out = engine.render("t.html", &ctx).unwrap();
    assert_eq!("", out);
}

#[test]
fn unknown_template_returns_err() {
    let engine = TeraEngine::from_raw(&[("t.html", "hi")]).unwrap();
    let ctx = Context::new();
    let result = engine.render("not_there.html", &ctx);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("render 'not_there.html' failed"));
}

// ── TeraEngine::response ──────────────────────────────────────────────────────

#[test]
fn response_is_200_html() {
    let engine = TeraEngine::from_raw(&[("page.html", "<p>hi</p>")]).unwrap();
    let ctx = Context::new();
    let resp = engine.response("page.html", &ctx).unwrap();
    assert_eq!(200, resp.status_code);
    assert_eq!(1, resp.content_range_list.len());
    let body = std::str::from_utf8(&resp.content_range_list[0].body).unwrap();
    assert_eq!("<p>hi</p>", body);
    assert!(resp.content_range_list[0]
        .content_type
        .contains("text/html"));
}

// ── from_raw error cases ──────────────────────────────────────────────────────

#[test]
fn from_raw_invalid_syntax_returns_err() {
    let result = TeraEngine::from_raw(&[("bad.html", "{% if %}broken")]);
    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(err.contains("bad.html"));
}

// ── multiple renders from same engine ─────────────────────────────────────────

#[test]
fn engine_reusable_across_renders() {
    let engine = TeraEngine::from_raw(&[("t.html", "{{ v }}")]).unwrap();
    for i in 0..5u32 {
        let mut ctx = Context::new();
        ctx.insert("v", &i);
        let out = engine.render("t.html", &ctx).unwrap();
        assert_eq!(i.to_string(), out);
    }
}

// ── TeraEngine::reload (hot reload) ───────────────────────────────────────────

fn tempdir_path() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}/rws_template_test_{}_{}", std::env::temp_dir().display(), std::process::id(), n)
}

#[test]
fn reload_picks_up_edited_template_content() {
    let dir = tempdir_path();
    std::fs::create_dir_all(&dir).unwrap();
    let file = format!("{}/greeting.html", dir);
    std::fs::write(&file, "<h1>v1</h1>").unwrap();

    let mut engine = TeraEngine::from_dir(&dir).unwrap();
    let ctx = Context::new();
    assert_eq!("<h1>v1</h1>", engine.render("greeting.html", &ctx).unwrap());

    std::fs::write(&file, "<h1>v2</h1>").unwrap();
    engine.reload().expect("reload should succeed");
    assert_eq!("<h1>v2</h1>", engine.render("greeting.html", &ctx).unwrap());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reload_picks_up_newly_added_template_file() {
    let dir = tempdir_path();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(format!("{}/a.html", dir), "A").unwrap();

    let mut engine = TeraEngine::from_dir(&dir).unwrap();
    let ctx = Context::new();
    assert_eq!("A", engine.render("a.html", &ctx).unwrap());
    assert!(engine.render("b.html", &ctx).is_err(), "b.html doesn't exist yet");

    std::fs::write(format!("{}/b.html", dir), "B").unwrap();
    engine.reload().expect("reload should succeed");
    assert_eq!("B", engine.render("b.html", &ctx).unwrap());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reload_on_from_raw_engine_returns_err() {
    let mut engine = TeraEngine::from_raw(&[("t.html", "hi")]).unwrap();
    let err = engine.reload().expect_err("from_raw engine has no glob to reload from");
    assert!(err.contains("from_raw"));
}

#[test]
fn reload_with_broken_template_keeps_previous_templates_serving() {
    let dir = tempdir_path();
    std::fs::create_dir_all(&dir).unwrap();
    let file = format!("{}/page.html", dir);
    std::fs::write(&file, "<p>good</p>").unwrap();

    let mut engine = TeraEngine::from_dir(&dir).unwrap();
    let ctx = Context::new();
    assert_eq!("<p>good</p>", engine.render("page.html", &ctx).unwrap());

    // Break the template (unclosed tag) and reload.
    std::fs::write(&file, "{% if %}broken").unwrap();
    let err = engine.reload().expect_err("broken template should fail reload");
    assert!(err.contains("template reload failed"));

    // The engine must still serve the last-known-good version — the whole
    // point of building the replacement set before swapping it in.
    assert_eq!("<p>good</p>", engine.render("page.html", &ctx).unwrap());

    let _ = std::fs::remove_dir_all(&dir);
}

// ── Global singleton: init / render / reload lifecycle ────────────────────────
//
// `OnceLock::set` only succeeds once per process, so only one test in this
// binary may call `template::init()`. This is that test — do not add another.

#[test]
fn global_init_render_and_reload_lifecycle() {
    let dir = tempdir_path();
    std::fs::create_dir_all(&dir).unwrap();
    let file = format!("{}/index.html", dir);
    std::fs::write(&file, "<h1>before</h1>").unwrap();

    super::init(&dir).expect("first init call should succeed");
    assert!(super::init(&dir).is_err(), "second init call must fail");

    let ctx = Context::new();
    let resp = super::render("index.html", &ctx).expect("render should succeed");
    assert_eq!(200, resp.status_code);
    let body = std::str::from_utf8(&resp.content_range_list[0].body).unwrap();
    assert_eq!("<h1>before</h1>", body);

    std::fs::write(&file, "<h1>after</h1>").unwrap();
    super::reload().expect("reload should succeed");

    let resp2 = super::render("index.html", &ctx).expect("render after reload should succeed");
    let body2 = std::str::from_utf8(&resp2.content_range_list[0].body).unwrap();
    assert_eq!("<h1>after</h1>", body2);

    let _ = std::fs::remove_dir_all(&dir);
}

// ── view helpers (18.35.0) ────────────────────────────────────────────────────

fn request(uri: &str, cookie: Option<&str>) -> crate::request::Request {
    let headers = cookie.map(|c| vec![crate::header::Header::new("Cookie", c.to_string())]).unwrap_or_default();
    crate::request::Request { method: "GET".into(), request_uri: uri.into(), http_version: "HTTP/1.1".into(), headers, body: Vec::new().into() }
}

#[test]
fn url_for_fills_parameters_and_query() {
    crate::router::name_route("tpl_post", "/tpl/posts/{id}");
    let engine = TeraEngine::from_raw(&[("a.html", r#"<a href="{{ url_for(name='tpl_post', id=7, tab='x y') | safe }}"><a href="{{ url_for(name='tpl_post', id=7) }}">"#)]).unwrap();
    // Escaped without `| safe`: `/` as `&#x2F;`, which browsers read back as `/`.
    assert_eq!(r#"<a href="/tpl/posts/7?tab=x%20y"><a href="&#x2F;tpl&#x2F;posts&#x2F;7">"#, engine.render("a.html", &Context::new()).unwrap());
}

#[test]
fn url_for_errors_name_the_problem() {
    let engine = TeraEngine::from_raw(&[("bad.html", "{{ url_for(name='tpl_missing') }}"), ("noname.html", "{{ url_for(id=1) }}")]).unwrap();
    assert!(engine.render("bad.html", &Context::new()).is_err());
    assert!(engine.render("noname.html", &Context::new()).is_err());
}

#[test]
fn vite_functions_read_the_installed_helper() {
    let manifest = r#"{"src/tpl.ts": {"file": "assets/tpl.js"}, "src/tpl.svg": {"file": "assets/tpl-1.svg"}}"#;
    crate::vite::install(crate::vite::Vite::from_manifest_str(manifest, "/").unwrap());
    let engine = TeraEngine::from_raw(&[("v.html", r#"{{ vite(entry="src/tpl.ts") | safe }}|{{ vite_asset(path="src/tpl.svg") | safe }}"#), ("missing.html", r#"{{ vite(entry="src/none.ts") }}"#)]).unwrap();
    assert_eq!(r#"<script type="module" src="/assets/tpl.js"></script>|/assets/tpl-1.svg"#, engine.render("v.html", &Context::new()).unwrap());
    assert!(engine.render("missing.html", &Context::new()).is_err());
}

#[test]
fn page_context_has_flash_csrf_and_path() {
    let engine = TeraEngine::from_raw(&[(
        "page.html",
        "{{ path }}|{% if notice %}{{ notice }}{% endif %}|{% for m in flash %}{{ m.kind }}:{{ m.text }};{% endfor %}|{{ csrf_field | safe }}",
    )])
    .unwrap();
    let plain = engine.render("page.html", &super::page_context(&request("/posts?x=1", None))).unwrap();
    assert_eq!("&#x2F;posts|||", plain);
    let with = engine.render("page.html", &super::page_context(&request("/", Some("rws_flash=notice=%3Cb%3E&alert=No; _csrf=tok")))).unwrap();
    #[cfg(feature = "csrf")]
    assert_eq!(r#"&#x2F;|&lt;b&gt;|notice:&lt;b&gt;;alert:No;|<input type="hidden" name="_csrf" value="tok">"#, with);
    #[cfg(not(feature = "csrf"))]
    assert_eq!("&#x2F;|&lt;b&gt;|notice:&lt;b&gt;;alert:No;|", with);
}
