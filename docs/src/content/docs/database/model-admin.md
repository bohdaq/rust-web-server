---
title: Model admin
description: Pages to list, search, create, edit and delete database rows — Django's admin for rws, as one middleware.
---

`model_admin::ModelAdmin` gives the tables you register pages to list, search, show, create, edit and delete rows — Django's admin, for a `DbPool` (any `model-*` feature). It's a middleware, so it works in front of sync and async apps alike (18.35.0).

```rust,no_run
use rust_web_server::auth_flows::CurrentUser;
use rust_web_server::model_admin::ModelAdmin;

let admins = ["ann@example.com".to_string()];
let admin = ModelAdmin::new(pool.clone())
    .title("Notes admin")
    .model::<Note>()                         // its #[table] name
    .table("rws_users")                      // or a table by name
    .allow(move |request| CurrentUser::of(request).is_some_and(|u| admins.contains(&u.email)));

// The first layer wrapped is the outermost.
let site = app(pool)
    .wrap(FlashLayer::new())                 // clears the messages the admin pages show
    .wrap(auth)                              // AuthFlows: who's asking
    .wrap(admin);
```

**Nobody gets in until `allow` says so**: without it every admin page answers `403`. The check runs for each request, after the layers outside the admin — wrap it after [`AuthFlows`](/features/auth-flows/) (inside it) to use `CurrentUser`, and after [`FlashLayer`](/building-apps/forms-uploads/#flash-messages-and-redirects) and [`CsrfLayer`](/features/csrf/) (whose token the forms carry).

## Pages

| Path | Page |
|---|---|
| `/admin` | The tables, with their row counts |
| `/admin/<table>?q=&page=` | 25 rows a page, newest key first; `q` searches the text columns (case-insensitive) |
| `/admin/<table>/new`, `POST /admin/<table>` | A new row |
| `/admin/<table>/<id>` | One row |
| `/admin/<table>/<id>/edit`, `POST /admin/<table>/<id>` | Change it |
| `POST /admin/<table>/<id>/delete` | Delete it |

`.prefix("/backoffice")` moves them. Only registered tables are served; table names are letters, digits and `_`.

## How it reads the database

The columns, their types and the primary key come from the database itself ([`DbPool::schema`](/database/migrations/#the-schema-as-data)) each time a page is drawn, so a migration shows at once — no admin code per model. Form values are converted by the column's type: integers and floats are checked, a `BOOLEAN` column is a checkbox, an empty field is `NULL` for a nullable column; `created_at` / `updated_at` are set to now when the form leaves them empty. On an edit, a column the form doesn't send keeps its value. What the database refuses — `NOT NULL`, a unique index, a foreign key — comes back as an alert on the form, through a [flash message](/building-apps/forms-uploads/#flash-messages-and-redirects) (add `FlashLayer`).

A table needs a one-column primary key to be edited; others are listed only. An integer key is left to the database when creating a row; any other key is a form field. On SQLite a column declared `INTEGER` is a number field even when it holds booleans (declare it `BOOLEAN` for a checkbox).

Values are shown as text and escaped; long ones are cut to 80 characters in the list. The pages are plain HTML with a little inline CSS and no scripts beyond a delete confirmation.
