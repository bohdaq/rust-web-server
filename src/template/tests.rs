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
