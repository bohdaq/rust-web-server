---
title: Controllers
description: The Controller trait is the lowest-level way to handle HTTP requests in rust-web-server.
---

## The Controller trait

`Controller` is defined in `src/controller/mod.rs` and has two static methods:

```rust
pub trait Controller {
    fn is_matching(request: &Request, connection: &ConnectionInfo) -> bool;
    fn process(request: &Request, response: Response, connection: &ConnectionInfo) -> Response;
}
```

- `is_matching` — called in declaration order inside `App::execute`; the first controller that returns `true` wins.
- `process` — receives the partially-built `response` (already populated with standard headers from `Header::get_header_list`) and must return it with a status code and body.

## Complete minimal example

```rust
use rust_web_server::controller::Controller;
use rust_web_server::request::{METHOD, Request};
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;
use rust_web_server::server::ConnectionInfo;

pub struct HelloController;

impl Controller for HelloController {
    fn is_matching(request: &Request, _conn: &ConnectionInfo) -> bool {
        request.method == METHOD.get && request.request_uri == "/hello"
    }

    fn process(_req: &Request, mut response: Response, _conn: &ConnectionInfo) -> Response {
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        response.content_range_list = vec![
            Range::get_content_range(
                b"Hello, world!".to_vec(),
                MimeType::TEXT_PLAIN.to_string(),
            )
        ];
        response
    }
}
```

## Registering a controller

Controllers are hardcoded in `App::execute` inside `src/app/mod.rs`. To add your own, you currently need a custom `Application` implementation that checks your controller before (or after) the built-in list:

```rust
use rust_web_server::application::Application;
use rust_web_server::app::App;
use rust_web_server::controller::Controller;
use rust_web_server::header::Header;
use rust_web_server::request::Request;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::server::ConnectionInfo;
use rust_web_server::core::New;

pub struct MyApp;

impl Application for MyApp {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        // Check custom controllers first
        if HelloController::is_matching(request, connection) {
            let header_list = Header::get_header_list(request);
            let response = Response::get_response(
                STATUS_CODE_REASON_PHRASE.n501_not_implemented,
                Some(header_list),
                None,
            );
            return Ok(HelloController::process(request, response, connection));
        }
        // Fall through to the built-in controller chain
        App::new().execute(request, connection)
    }
}
```

## Built-in controllers

These are registered in `App::execute` in this order (first match wins):

| Controller | Matches |
|---|---|
| `IndexController` | `GET /` — serves `index.html` from the static directory |
| `StyleController` | `GET *.css` |
| `ScriptController` | `GET *.js` |
| `DirectoryListingAssetsController` | `GET /rws-directory-listing.css` / `.js` — same-origin CSS/JS for the directory listing page below |
| `FileUploadInitiateController` | `POST /upload/initiate` |
| `FormUrlEncodedEnctypePostMethodController` | `POST` with `application/x-www-form-urlencoded` |
| `FormGetMethodController` | `GET` requests with query parameters to form paths |
| `FormMultipartEnctypePostMethodController` | `POST` with `multipart/form-data` |
| `HealthController` | `GET /healthz` — returns `200 OK` |
| `ReadyController` | `GET /readyz` — returns `200` when `SERVER_READY` is set |
| `MetricsController` | `GET /metrics` — Prometheus text format |
| `FaviconController` | `GET /favicon.ico` |
| `StaticResourceController` | Any `GET` for a file found under the static directory; a directory with no `index.html` renders a directory listing page instead of falling through to `404` |
| `NotFoundController` | Catch-all — returns `404 Not Found` |

:::note[Prefer App::with_state for new routes]
For new application routes, `App::with_state(S).get(pattern, handler)` or the `routes!` macro is less boilerplate than writing a custom `Controller` implementation and wiring it into a custom `Application`. Use `Controller` when you need matching logic that path patterns cannot express, or when you need to intercept built-in behaviour.
:::
