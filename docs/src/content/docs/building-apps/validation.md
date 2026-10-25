---
title: Validation
description: Validate incoming request data with the Validate trait, the #[derive(Validate)] macro, and the Validated<T> extractor.
---

Validation in `rust-web-server` is built around three pieces that compose cleanly:

- `Validate` — a trait your types implement to declare field-level rules
- `#[derive(Validate)]` — a proc-macro that generates the implementation from annotations
- `Validated<T>` — a `FromRequest` wrapper that extracts and validates in one step

## The `Validate` trait

```rust
pub trait Validate {
    fn validate(&self) -> Result<(), ValidationErrors>;
}
```

Implement it manually when you need custom logic:

```rust
use rust_web_server::validate::{Validate, ValidationErrors};

struct Payload {
    name: String,
    score: u32,
}

impl Validate for Payload {
    fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();

        if self.name.is_empty() {
            errors.add("name", "must not be empty");
        }
        if self.score > 100 {
            errors.add("score", "must be 100 or less");
        }

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}
```

The checks behind the derive's `email` and `url` validators are public, so a manual implementation can reuse them: `rust_web_server::validate::is_email(&self.email)` and `rust_web_server::validate::is_url(&self.homepage)`.

## `ValidationErrors`

`ValidationErrors` accumulates every failing field so the caller sees all problems at once.

| Method | Description |
|---|---|
| `ValidationErrors::new()` | Creates an empty collector |
| `.add(field, message)` | Records one failure |
| `.is_empty()` | Returns `true` when there are no errors |
| `.errors()` | Returns `&[FieldError]` — each has `.field` and `.message` |
| `.into_json()` | Serialises as `{"errors":[{"field":"…","message":"…"}]}` |

## `#[derive(Validate)]`

Add `features = ["macros"]` to your dependency to unlock the derive macro.

```toml
[dependencies]
rust-web-server = { version = "17", features = ["macros"] }
```

Then annotate your struct fields with `#[validate(...)]` rules:

```rust
use rust_web_server::Validate;

#[derive(rust_web_server::Validate)]
struct SignupForm {
    #[validate(length(min = 1, max = 50))]
    username: String,

    #[validate(email)]
    email: String,

    #[validate(length(min = 8, max = 128))]
    password: String,

    #[validate(range(min = 13, max = 120))]
    age: u8,

    #[validate(url)]
    website: String,

    #[validate(required)]
    terms_accepted: String,
}
```

### Supported validators

| Syntax | What it checks |
|---|---|
| `length(min = N)` | `field.chars().count() >= N` |
| `length(max = N)` | `field.chars().count() <= N` |
| `length(min = N, max = N)` | both bounds |
| `range(min = N)` | `field as f64 >= N` |
| `range(max = N)` | `field as f64 <= N` |
| `range(min = N, max = N)` | both bounds |
| `email` | non-empty local part before the first `@`; domain after it at least 3 chars, contains `.`, and does not start or end with `.` |
| `required` | `!field.is_empty()` |
| `url` | starts with `http://` or `https://` |

All failures are collected before returning — the caller always sees every invalid field.

## `Validated<T>` extractor

`Validated<T>` implements `FromRequest` for any `T: FromRequest + Validate`. It first extracts `T` from the request, then runs validation:

- If extraction fails (e.g. invalid UTF-8) → `400 Bad Request`
- If validation fails → `422 Unprocessable Entity` with a JSON body

```rust
use rust_web_server::validate::{Validate, Validated, ValidationErrors};
use rust_web_server::extract::{FromRequest, BodyText};
use rust_web_server::request::Request;
use rust_web_server::response::Response;

fn handle(req: &Request) -> Response {
    let Validated(form) = match Validated::<SignupForm>::from_request(req) {
        Ok(v)  => v,
        Err(r) => return r,  // 400 or 422
    };

    // form.username, form.email, etc. are all valid here
    Response::new()
}
```

The 422 body looks like:

```json
{
  "errors": [
    { "field": "email",    "message": "email must be a valid email address" },
    { "field": "username", "message": "username must be at least 1 character(s) long" }
  ]
}
```

## Complete example — signup endpoint

```rust
use rust_web_server::Validate;
use rust_web_server::validate::{Validate as _, Validated, ValidationErrors};
use rust_web_server::json::Json;
use rust_web_server::extract::FromRequest;
use rust_web_server::request::Request;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::router::PathParams;
use rust_web_server::server::ConnectionInfo;
use rust_web_server::routes;
use rust_web_server::app::App;
use serde::Deserialize;

#[derive(Deserialize, rust_web_server::Validate)]
struct SignupRequest {
    #[validate(length(min = 1, max = 50))]
    username: String,

    #[validate(email)]
    email: String,

    #[validate(length(min = 8))]
    password: String,
}

impl FromRequest for SignupRequest {
    fn from_request(req: &Request) -> Result<Self, Response> {
        let Json(payload) = Json::<SignupRequest>::from_request(req)?;
        Ok(payload)
    }
}

fn signup(
    req: &Request,
    _params: &PathParams,
    _conn: &ConnectionInfo,
    _state: &(),
) -> Response {
    let Validated(body) = match Validated::<SignupRequest>::from_request(req) {
        Ok(v)  => v,
        Err(r) => return r,
    };

    // At this point body.username, body.email, and body.password are valid.
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n201_created.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n201_created.reason_phrase.to_string();
    r
}

let app = routes! {
    App::with_state(()),
    POST "/signup" => signup,
};
```

:::note[Manual vs. derived]
Prefer `#[derive(Validate)]` for straightforward field constraints. Implement `Validate` manually when you need cross-field rules (e.g. "password must equal password_confirm") or async lookups — just call the async code outside the `Validated<T>` path.
:::
