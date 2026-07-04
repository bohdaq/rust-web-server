---
title: HTML Templates
description: Render HTML responses with the Tera template engine via the tera feature flag, with hot reload for edited templates.
---

`rust-web-server` integrates the [Tera](https://keats.github.io/tera/) template engine — a Jinja2-compatible HTML templating system with variables, control flow, filters, and inheritance.

:::caution[Feature requirement]
The template integration requires the `tera` feature:

```toml
[dependencies]
rust-web-server = { version = "17", features = ["tera"] }
```
:::

## Quick start

1. Create a `templates/` directory next to your binary.
2. Call `template::init("templates")` once at startup.
3. Call `template::render("page.html", &ctx)` from any handler.

```rust
use rust_web_server::template::{self, Context};

fn main() {
    // Initialize the global template engine once at startup.
    template::init("templates").expect("failed to load templates");

    // Start the server ...
}
```

## Initialisation

There are two ways to initialise the global engine:

### `template::init(dir)`

Loads all files under `dir` recursively (equivalent to the glob `dir/**/*`):

```rust
template::init("templates").unwrap();
```

### `template::init_from_env()`

Reads the directory from the `RWS_CONFIG_TEMPLATE_DIR` environment variable (default: `"templates"`):

```rust
template::init_from_env().unwrap();
```

Both functions return `Err` if called a second time (the engine is a `OnceLock`).

## Rendering in a handler

```rust
use rust_web_server::template::{self, Context};
use rust_web_server::request::Request;
use rust_web_server::response::Response;
use rust_web_server::router::PathParams;
use rust_web_server::server::ConnectionInfo;

fn home(
    _req: &Request,
    _params: &PathParams,
    _conn: &ConnectionInfo,
    _state: &(),
) -> Response {
    let mut ctx = Context::new();
    ctx.insert("title",  "Welcome");
    ctx.insert("items",  &["Rust", "rws", "Tera"]);
    ctx.insert("logged_in", &true);

    template::render("index.html", &ctx).unwrap_or_else(|e| {
        // render() returns Err only when the template file is missing
        // or contains a syntax error — treat as 500
        let mut r = Response::new();
        r.status_code = 500;
        r
    })
}
```

`template::render` delegates to the global `TeraEngine`, renders the named template, and returns a `200 OK` response with `Content-Type: text/html`.

## The `Context` type

`Context` is a re-export of `tera::Context`. Call `.insert(key, value)` with any `Serialize` value:

```rust
use rust_web_server::template::Context;
use serde::Serialize;

#[derive(Serialize)]
struct User {
    name:  String,
    email: String,
}

let user = User { name: "Alice".into(), email: "alice@example.com".into() };

let mut ctx = Context::new();
ctx.insert("user",  &user);
ctx.insert("count", &42_u32);
ctx.insert("flags", &["new", "featured"]);
```

## Directory structure convention

```
your-project/
├── src/
│   └── main.rs
└── templates/
    ├── base.html          <- base layout
    ├── index.html
    ├── users/
    │   ├── list.html
    │   └── detail.html
    └── partials/
        └── nav.html
```

Template names passed to `render` are relative paths within the templates directory (e.g. `"users/list.html"`).

## Tera template syntax

### Variables

```html
<h1>{{ title }}</h1>
<p>Hello, {{ user.name }}!</p>
<p>Total: {{ count }}</p>
```

### Conditionals

```html
{% if logged_in %}
  <a href="/logout">Logout</a>
{% else %}
  <a href="/login">Login</a>
{% endif %}
```

### Loops

```html
<ul>
{% for item in items %}
  <li>{{ item }}</li>
{% endfor %}
</ul>
```

Loop provides a `loop` variable with helpers: `loop.index` (1-based), `loop.index0` (0-based), `loop.first`, `loop.last`.

### Template inheritance

Define a base layout:

```html
{# templates/base.html #}
<!DOCTYPE html>
<html>
<head><title>{% block title %}My Site{% endblock %}</title></head>
<body>
  <nav>{% include "partials/nav.html" %}</nav>
  <main>{% block content %}{% endblock %}</main>
</body>
</html>
```

Extend it in a child template:

```html
{# templates/index.html #}
{% extends "base.html" %}

{% block title %}Home — My Site{% endblock %}

{% block content %}
  <h1>{{ heading }}</h1>
  <p>{{ body }}</p>
{% endblock %}
```

### Built-in filters

Tera ships many filters you can apply with `|`:

```html
{{ name | upper }}              {# ALICE #}
{{ title | truncate(length=20) }}
{{ items | length }}
{{ price | round(precision=2) }}
{{ html_content | safe }}       {# disable auto-escaping #}
{{ date | date(format="%Y-%m-%d") }}
```

Auto-escaping is enabled by default for `{{ }}` output — HTML special characters (`<`, `>`, `&`, `"`) are escaped. Use `| safe` only for trusted content.

### Macros

Define reusable template fragments:

```html
{% macro input(name, label, type="text") %}
  <label for="{{ name }}">{{ label }}</label>
  <input type="{{ type }}" id="{{ name }}" name="{{ name }}">
{% endmacro %}

{{ self::input(name="email", label="Email", type="email") }}
```

## `TeraEngine` directly

The global singleton wraps `TeraEngine`. You can also create a standalone engine when you need multiple template directories, or for testing:

```rust
use rust_web_server::template::{TeraEngine, Context};

// From a directory
let engine = TeraEngine::from_dir("templates").unwrap();

// From in-memory strings (useful in tests)
let engine = TeraEngine::from_raw(&[
    ("hello.html", "<p>Hello, {{ name }}!</p>"),
]).unwrap();

let mut ctx = Context::new();
ctx.insert("name", "World");
let html: String = engine.render("hello.html", &ctx).unwrap();
let response = engine.response("hello.html", &ctx).unwrap();
```

`TeraEngine::from_glob(pattern)` accepts any glob pattern Tera accepts, e.g. `"templates/**/*.html"`.

## Hot reload

Edit a template file and reload it without restarting the server:

```rust
use rust_web_server::template;

template::init("templates").unwrap();
// ... edit templates/index.html on disk ...
template::reload().unwrap();
```

`template::reload()`:

- **Re-globs the whole directory** — it picks up edited content in existing files *and* newly added or removed files matching the original pattern, not just a diff of what changed.
- **Is atomic.** It builds the full replacement set of templates before swapping it in. If one edited file has a syntax error (or an `{% extends %}`/`{% import %}` chain no longer resolves), `reload()` returns `Err` and the previously-loaded templates keep serving — a bad edit can't leave the live server with a half-broken template set.
- **Is wired into `SIGHUP` automatically.** [`config_reload::reload()`](/features/hot-reload/) calls it for you (as a no-op if `template::init()` was never called), so `kill -HUP $(pidof rws)` picks up edited templates alongside CORS/rate-limit/TLS-cert changes. Call `template::reload()` directly only if you want a different trigger — a file-watcher thread, or your own admin endpoint.

The equivalent method exists on a standalone `TeraEngine` too — `engine.reload()` — for the multi-directory case described above. It requires the engine to have been built from a glob (`from_dir`/`from_glob`/`init`); an engine built with `TeraEngine::from_raw` has no directory to re-read and `reload()` returns `Err` for it.

```rust
use rust_web_server::template::TeraEngine;

let mut engine = TeraEngine::from_dir("templates").unwrap();
// ... edit a template file ...
engine.reload().unwrap();
```
