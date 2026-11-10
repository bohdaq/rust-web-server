//! Model admin (18.35.0, #109): list, search, show, create, edit and delete
//! rows of the tables you register, as server-rendered pages — Django's
//! admin, for a `DbPool` (`model-*` features). A middleware, so it works in
//! front of sync and async apps alike.
//!
//! ```rust,no_run
//! # async fn example(pool: rust_web_server::model::DbPool) {
//! use rust_web_server::app::App;
//! use rust_web_server::auth_flows::CurrentUser;
//! use rust_web_server::model_admin::ModelAdmin;
//! # #[derive(rust_web_server::Model)] struct Post { #[primary_key(auto_increment)] id: i64, title: String }
//!
//! let admin = ModelAdmin::new(pool.clone())
//!     .model::<Post>()                      // or .table("posts")
//!     .allow(|request| CurrentUser::of(request).is_some_and(|u| u.email == "ann@example.com"));
//! // Inside the layers it needs (the first wrapped is the outermost):
//! // FlashLayer, CsrfLayer, AuthFlows, then the admin. Pages under /admin.
//! let app = App::with_async_state(pool).wrap(admin);
//! # }
//! ```
//!
//! **Nobody gets in until [`allow`](ModelAdmin::allow) says so**: without it
//! every page answers `403`. Wrap it after your sign-in ([`AuthFlows`]),
//! `FlashLayer` and [`CsrfLayer`](crate::csrf::CsrfLayer) — layers wrapped
//! first are outside it and see its pages; the forms carry the CSRF field
//! when that layer is there.
//!
//! The columns, their types and the primary key come from the database
//! ([`DbPool::schema`](crate::model::DbPool::schema)), read when a page is
//! drawn, so a migration shows at once. Form values are converted by the
//! column's type (integers, floats, booleans as a checkbox, text; an
//! empty field is `NULL` for a nullable column); `created_at`/`updated_at`
//! columns are set to now when the form leaves them empty. What the
//! database refuses (a `NOT NULL`, a unique index, a foreign key) comes
//! back as an alert on the form. Tables need a one-column primary key to
//! be edited; others are listed only.
//!
//! [`AuthFlows`]: crate::auth_flows::AuthFlows

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;

use crate::application::Application;
use crate::error::IntoResponse;
use crate::flash::{Flash, Redirect};
use crate::header::Header;
use crate::middleware::Middleware;
use crate::model::db_schema::{ColumnSchema, TableSchema};
use crate::model::{Backend, DbPool, ModelRow, Value};
use crate::request::Request;
use crate::response::{Response, StatusCodeReasonPhrase, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

type Allow = Arc<dyn Fn(&Request) -> bool + Send + Sync>;

/// Rows per page.
pub const PAGE_SIZE: i64 = 25;

/// The admin pages. See the [module docs](self).
#[derive(Clone)]
pub struct ModelAdmin {
    pool: DbPool,
    prefix: String,
    tables: Vec<String>,
    allow: Option<Allow>,
    title: String,
}

impl ModelAdmin {
    /// Pages for `pool`'s tables you register, under `/admin`.
    pub fn new(pool: DbPool) -> Self {
        ModelAdmin { pool, prefix: "/admin".into(), tables: Vec::new(), allow: None, title: "Admin".into() }
    }

    /// Where the pages live (default `/admin`).
    pub fn prefix(mut self, prefix: &str) -> Self {
        self.prefix = format!("/{}", prefix.trim_matches('/'));
        self
    }

    /// The heading of every page (default "Admin").
    pub fn title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    /// Manage the table of `T`.
    pub fn model<T: crate::model::Model>(self) -> Self {
        self.table(T::table_name())
    }

    /// Manage `table` (letters, digits and `_` only; others are ignored).
    pub fn table(mut self, table: &str) -> Self {
        if !table.is_empty() && table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !self.tables.iter().any(|t| t == table) {
            self.tables.push(table.to_string());
        }
        self
    }

    /// Who may use the pages: `check(request)` runs for each request
    /// (after the layers outside this one, such as `AuthFlows`). Without
    /// it, nobody.
    pub fn allow(mut self, check: impl Fn(&Request) -> bool + Send + Sync + 'static) -> Self {
        self.allow = Some(Arc::new(check));
        self
    }

    fn run<T: Send + 'static>(&self, work: impl std::future::Future<Output = Result<T, crate::model::DbError>> + Send + 'static) -> Result<T, String> {
        rws_server::async_bridge::block_on_shared(work).map_err(|e| e.to_string())
    }

    fn schema(&self, table: &str) -> Result<Option<TableSchema>, String> {
        let pool = self.pool.clone();
        let schema = self.run(async move { pool.schema().await })?;
        Ok(schema.tables.into_iter().find(|t| t.name.eq_ignore_ascii_case(table)))
    }

    fn route(&self, request: &Request) -> Option<Response> {
        let path = request.request_uri.split('?').next().unwrap_or("");
        let rest = path.strip_prefix(&self.prefix)?;
        if !(rest.is_empty() || rest.starts_with('/')) {
            return None;
        }
        if !self.allow.as_ref().is_some_and(|allow| allow(request)) {
            return Some(crate::error::problem::problem(STATUS_CODE_REASON_PHRASE.n403_forbidden, "not allowed"));
        }
        let parts: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
        let get = request.method == "GET" || request.method == "HEAD";
        let post = request.method == "POST";
        let page = Page { admin: self, request };
        let answer = match (parts.as_slice(), get, post) {
            ([], true, _) => page.index(),
            ([table], true, _) => page.with_table(table, |p, t| p.list(t)),
            ([table], _, true) => page.with_table(table, |p, t| p.save(t, None)),
            ([table, "new"], true, _) => page.with_table(table, |p, t| p.form(t, None, None)),
            ([table, id], true, _) => page.with_table(table, |p, t| p.show(t, id)),
            ([table, id, "edit"], true, _) => page.with_table(table, |p, t| p.edit(t, id)),
            ([table, id], _, true) => page.with_table(table, |p, t| p.save(t, Some(id))),
            ([table, id, "delete"], _, true) => page.with_table(table, |p, t| p.delete(t, id)),
            _ => Err((STATUS_CODE_REASON_PHRASE.n404_not_found, "no such admin page".to_string())),
        };
        Some(answer.unwrap_or_else(|(status, message)| page.error(status, &message)))
    }
}

impl Middleware for ModelAdmin {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        match self.route(request) {
            Some(response) => Ok(response),
            None => next.execute(request, connection),
        }
    }

    fn is_passthrough(&self, request: &Request, _connection: &ConnectionInfo) -> bool {
        !request.request_uri.starts_with(&self.prefix)
    }

    fn accepts_streamed_body(&self, request: &Request, _connection: &ConnectionInfo) -> bool {
        !request.request_uri.starts_with(&self.prefix)
    }

    fn layer_name(&self) -> &'static str {
        "ModelAdmin"
    }
}

type PageResult = Result<Response, (&'static StatusCodeReasonPhrase, String)>;

fn internal(e: String) -> (&'static StatusCodeReasonPhrase, String) {
    eprintln!("[model admin] {e}");
    (STATUS_CODE_REASON_PHRASE.n500_internal_server_error, e)
}

/// One request's page.
struct Page<'a> {
    admin: &'a ModelAdmin,
    request: &'a Request,
}

/// How a column's values are read from a form and bound.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Int,
    Float,
    Bool,
    Timestamp,
    Text,
    /// A type bound as text and cast by PostgreSQL (`uuid`, `date`, …).
    Other,
}

fn kind(column: &ColumnSchema) -> Kind {
    kind_of(&column.data_type)
}

fn kind_of(data_type: &str) -> Kind {
    let t = data_type.to_ascii_lowercase();
    if t.contains("bool") || t == "tinyint(1)" {
        Kind::Bool
    } else if t.contains("int") || t == "serial" || t == "bigserial" {
        Kind::Int
    } else if t.contains("real") || t.contains("double") || t.contains("float") || t.contains("numeric") || t.contains("decimal") {
        Kind::Float
    } else if t.contains("timestamp") || t.contains("datetime") {
        Kind::Timestamp
    } else if t.contains("char") || t.contains("text") || t.is_empty() || t == "string" {
        Kind::Text
    } else {
        Kind::Other
    }
}

fn quote(backend: Backend, name: &str) -> String {
    match backend {
        #[cfg(feature = "model-mysql")]
        Backend::MySql => format!("`{}`", name.replace('`', "")),
        #[allow(unreachable_patterns)]
        _ => format!("\"{}\"", name.replace('"', "")),
    }
}

fn placeholder(backend: Backend, n: usize, column: &ColumnSchema) -> String {
    match backend {
        #[cfg(feature = "model-postgres")]
        Backend::Postgres => match kind(column) {
            Kind::Timestamp | Kind::Other => format!("CAST(${n} AS {})", column.data_type),
            _ => format!("${n}"),
        },
        #[allow(unreachable_patterns)]
        _ => {
            let _ = (n, column);
            "?".to_string()
        }
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn display(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Text(s) => s.clone(),
        Value::Bytes(b) => format!("({} bytes)", b.len()),
    }
}

fn cell(row: &ModelRow, column: &str) -> Value {
    row.columns.iter().find(|(c, _)| c.eq_ignore_ascii_case(column)).map(|(_, v)| v.clone()).unwrap_or(Value::Null)
}

/// `value` (from a form or a URL) as `column`'s value; `Err` names what's wrong.
fn parse(column: &ColumnSchema, value: Option<&str>) -> Result<Value, String> {
    parse_as(&column.name, &column.data_type, column.nullable, value)
}

fn parse_as(name: &str, data_type: &str, nullable: bool, value: Option<&str>) -> Result<Value, String> {
    let k = kind_of(data_type);
    if k == Kind::Bool {
        // Every backend binds a bool (SQLite and MySQL as 0/1).
        return Ok(Value::Bool(matches!(value, Some("on" | "true" | "1"))));
    }
    let text = value.unwrap_or("").trim();
    if text.is_empty() {
        return if nullable { Ok(Value::Null) } else if k == Kind::Text { Ok(Value::Text(String::new())) } else { Err(format!("{} is required", name)) };
    }
    match k {
        Kind::Int => text.parse().map(Value::Int).map_err(|_| format!("{} must be a whole number", name)),
        Kind::Float => text.parse().map(Value::Float).map_err(|_| format!("{} must be a number", name)),
        _ => Ok(Value::Text(value.unwrap_or("").to_string())),
    }
}

impl<'a> Page<'a> {
    fn backend(&self) -> Backend {
        self.admin.pool.backend()
    }

    fn with_table(&self, table: &str, f: impl FnOnce(&Self, &TableSchema) -> PageResult) -> PageResult {
        if !self.admin.tables.iter().any(|t| t == table) {
            return Err((STATUS_CODE_REASON_PHRASE.n404_not_found, format!("{table} isn't managed here")));
        }
        match self.admin.schema(table).map_err(internal)? {
            Some(schema) => f(self, &schema),
            None => Err((STATUS_CODE_REASON_PHRASE.n404_not_found, format!("the database has no table {table}"))),
        }
    }

    fn pk<'t>(&self, table: &'t TableSchema) -> Option<&'t ColumnSchema> {
        match table.primary_key.as_slice() {
            [one] => table.column(one),
            _ => None,
        }
    }

    fn query(&self, sql: String, params: Vec<Value>) -> Result<Vec<ModelRow>, String> {
        let pool = self.admin.pool.clone();
        self.admin.run(async move { pool.query_rows(&sql, &params).await })
    }

    fn execute(&self, sql: String, params: Vec<Value>) -> Result<u64, String> {
        let pool = self.admin.pool.clone();
        self.admin.run(async move { pool.execute(&sql, &params).await })
    }

    fn url(&self, parts: &[&str]) -> String {
        let mut url = self.admin.prefix.clone();
        for part in parts {
            url.push('/');
            url.push_str(&crate::router::build_path("/{x}", &[("x", part)]).unwrap_or_default()[1..]);
        }
        url
    }

    fn query_param(&self, name: &str) -> String {
        let query = self.request.request_uri.split_once('?').map(|(_, q)| q).unwrap_or("");
        form_fields(query).remove(name).unwrap_or_default()
    }

    fn html(&self, status: &StatusCodeReasonPhrase, heading: &str, body: &str) -> Response {
        let tables: String = self.admin.tables.iter().map(|t| format!("<a href=\"{}\">{}</a>", escape(&self.url(&[t])), escape(t))).collect::<Vec<_>>().join(" ");
        let flash: String = Flash::read(self.request).messages().iter().map(|m| format!("<p class=\"{}\">{}</p>", escape(&m.kind), escape(&m.text))).collect();
        let page = format!(
            "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{heading} · {title}</title>\
             <style>body{{font:15px system-ui,sans-serif;margin:0;color:#1d1b20}}header{{background:#1d1b20;color:#fff;padding:.75rem 1rem}}header a{{color:#cfc;margin-right:1rem}}main{{padding:1rem;max-width:72rem}}\
             table{{border-collapse:collapse;width:100%}}td,th{{border-bottom:1px solid #ddd;padding:.4rem .5rem;text-align:left;vertical-align:top;max-width:24rem;overflow-wrap:anywhere}}\
             label{{display:block;margin:.6rem 0}}input[type=text],textarea{{display:block;width:100%;max-width:36rem;padding:.4rem;box-sizing:border-box}}.alert{{color:#b00020}}.notice{{color:#1b5e20}}button{{padding:.4rem .9rem}}form.inline{{display:inline}}</style></head>\
             <body><header><a href=\"{home}\"><strong>{title}</strong></a>{tables}</header><main><h1>{heading}</h1>{flash}{body}</main></body></html>",
            title = escape(&self.admin.title),
            home = escape(&self.admin.prefix),
            heading = escape(heading),
        );
        let content = crate::range::Range::get_content_range(page.into_bytes(), crate::mime_type::MimeType::TEXT_HTML.to_string());
        let mut response = Response::get_response(status, None, Some(vec![content]));
        response.headers.push(Header::new(Header::_CACHE_CONTROL, "no-store"));
        response
    }

    fn error(&self, status: &'static StatusCodeReasonPhrase, message: &str) -> Response {
        self.html(status, &format!("{} {}", status.status_code, status.reason_phrase), &format!("<p>{}</p>", escape(message)))
    }

    fn csrf(&self) -> String {
        #[cfg(feature = "csrf")]
        if let Some(token) = crate::csrf::CsrfToken::from_request(self.request) {
            return format!("<input type=\"hidden\" name=\"_csrf\" value=\"{}\">", escape(token.value()));
        }
        String::new()
    }

    fn index(&self) -> PageResult {
        let mut rows = String::new();
        for table in &self.admin.tables {
            let count = match self.query(format!("SELECT COUNT(*) AS n FROM {}", quote(self.backend(), table)), Vec::new()) {
                Ok(r) => r.first().map(|r| display(&cell(r, "n"))).unwrap_or_default(),
                Err(_) => "(missing)".into(),
            };
            rows.push_str(&format!("<tr><td><a href=\"{}\">{}</a></td><td>{}</td></tr>", escape(&self.url(&[table])), escape(table), escape(&count)));
        }
        Ok(self.html(STATUS_CODE_REASON_PHRASE.n200_ok, &self.admin.title, &format!("<table><tr><th>Table</th><th>Rows</th></tr>{rows}</table>")))
    }

    fn list(&self, table: &TableSchema) -> PageResult {
        let backend = self.backend();
        let page: i64 = self.query_param("page").parse().unwrap_or(1).max(1);
        let q = self.query_param("q");
        let text_columns: Vec<&ColumnSchema> = table.columns.iter().filter(|c| kind(c) == Kind::Text).collect();
        let (mut filter, mut params) = (String::new(), Vec::new());
        if !q.is_empty() && !text_columns.is_empty() {
            let conditions: Vec<String> = text_columns
                .iter()
                .enumerate()
                .map(|(i, c)| format!("LOWER({}) LIKE LOWER({})", quote(backend, &c.name), placeholder(backend, i + 1, c)))
                .collect();
            filter = format!(" WHERE {}", conditions.join(" OR "));
            params = text_columns.iter().map(|_| Value::Text(format!("%{q}%"))).collect();
        }
        let order = self.pk(table).map(|pk| format!(" ORDER BY {} DESC", quote(backend, &pk.name))).unwrap_or_default();
        let t = quote(backend, &table.name);
        let total = self.query(format!("SELECT COUNT(*) AS n FROM {t}{filter}"), params.clone()).map_err(internal)?;
        let total = match total.first().map(|r| cell(r, "n")) {
            Some(Value::Int(n)) => n,
            _ => 0,
        };
        let rows = self
            .query(format!("SELECT * FROM {t}{filter}{order} LIMIT {PAGE_SIZE} OFFSET {}", (page - 1) * PAGE_SIZE), params)
            .map_err(internal)?;
        let pk = self.pk(table);
        let head: String = table.columns.iter().map(|c| format!("<th>{}</th>", escape(&c.name))).collect();
        let mut body = String::new();
        for row in &rows {
            body.push_str("<tr>");
            for c in &table.columns {
                let mut text = display(&cell(row, &c.name));
                if text.chars().count() > 80 {
                    text = text.chars().take(80).collect::<String>() + "…";
                }
                match pk {
                    Some(pk) if pk.name == c.name => {
                        body.push_str(&format!("<td><a href=\"{}\">{}</a></td>", escape(&self.url(&[&table.name, &display(&cell(row, &pk.name))])), escape(&text)))
                    }
                    _ => body.push_str(&format!("<td>{}</td>", escape(&text))),
                }
            }
            body.push_str("</tr>");
        }
        let pages = ((total + PAGE_SIZE - 1) / PAGE_SIZE).max(1);
        let link = |p: i64| {
            let path = crate::router::build_path(&self.url(&[&table.name]), &[("page", &p.to_string()), ("q", &q)]).unwrap_or_default();
            escape(&path)
        };
        let mut nav = format!("<p>{total} rows · page {page} of {pages}");
        if page > 1 {
            nav.push_str(&format!(" · <a href=\"{}\">newer</a>", link(page - 1)));
        }
        if page < pages {
            nav.push_str(&format!(" · <a href=\"{}\">older</a>", link(page + 1)));
        }
        nav.push_str("</p>");
        let new = if pk.is_some() { format!("<p><a href=\"{}\">New {}</a></p>", escape(&self.url(&[&table.name, "new"])), escape(&table.name)) } else { "<p>(no one-column primary key: rows can't be edited here)</p>".into() };
        let search = format!(
            "<form method=\"get\"><input type=\"text\" name=\"q\" value=\"{}\" placeholder=\"Search\" style=\"display:inline;max-width:20rem\"> <button>Search</button></form>",
            escape(&q)
        );
        Ok(self.html(STATUS_CODE_REASON_PHRASE.n200_ok, &table.name, &format!("{new}{search}{nav}<table><tr>{head}</tr>{body}</table>")))
    }

    fn find(&self, table: &TableSchema, id: &str) -> Result<ModelRow, (&'static StatusCodeReasonPhrase, String)> {
        let pk = self.pk(table).ok_or((STATUS_CODE_REASON_PHRASE.n404_not_found, format!("{} has no one-column primary key", table.name)))?;
        let key = parse(pk, Some(id)).map_err(|e| (STATUS_CODE_REASON_PHRASE.n404_not_found, e))?;
        let backend = self.backend();
        let rows = self
            .query(format!("SELECT * FROM {} WHERE {} = {}", quote(backend, &table.name), quote(backend, &pk.name), placeholder(backend, 1, pk)), vec![key])
            .map_err(internal)?;
        rows.into_iter().next().ok_or((STATUS_CODE_REASON_PHRASE.n404_not_found, format!("no {} {id}", table.name)))
    }

    fn show(&self, table: &TableSchema, id: &str) -> PageResult {
        let row = self.find(table, id)?;
        let fields: String = table.columns.iter().map(|c| format!("<tr><th>{}</th><td>{}</td></tr>", escape(&c.name), escape(&display(&cell(&row, &c.name))))).collect();
        let actions = format!(
            "<p><a href=\"{edit}\">Edit</a> · <a href=\"{list}\">All {t}</a></p><form class=\"inline\" method=\"post\" action=\"{delete}\" onsubmit=\"return confirm('Delete this row?')\">{csrf}<button>Delete</button></form>",
            edit = escape(&self.url(&[&table.name, id, "edit"])),
            list = escape(&self.url(&[&table.name])),
            delete = escape(&self.url(&[&table.name, id, "delete"])),
            t = escape(&table.name),
            csrf = self.csrf(),
        );
        Ok(self.html(STATUS_CODE_REASON_PHRASE.n200_ok, &format!("{} {id}", table.name), &format!("<table>{fields}</table>{actions}")))
    }

    fn edit(&self, table: &TableSchema, id: &str) -> PageResult {
        let row = self.find(table, id)?;
        self.form(table, Some(id), Some(&row))
    }

    /// Whether the form offers `column`: not the auto key.
    fn editable(&self, table: &TableSchema, column: &ColumnSchema, editing: bool) -> bool {
        let is_pk = self.pk(table).is_some_and(|pk| pk.name == column.name);
        !(is_pk && (editing || kind(column) == Kind::Int))
    }

    fn form(&self, table: &TableSchema, id: Option<&str>, row: Option<&ModelRow>) -> PageResult {
        if self.pk(table).is_none() {
            return Err((STATUS_CODE_REASON_PHRASE.n404_not_found, format!("{} has no one-column primary key", table.name)));
        }
        let mut fields = String::new();
        for c in table.columns.iter().filter(|c| self.editable(table, c, id.is_some())) {
            let value = row.map(|r| display(&cell(r, &c.name))).unwrap_or_default();
            let hint = format!("{}{}", c.data_type.to_lowercase(), if c.nullable { ", optional" } else { "" });
            let input = match kind(c) {
                Kind::Bool => format!("<input type=\"checkbox\" name=\"{}\"{}>", escape(&c.name), if matches!(value.as_str(), "true" | "1") { " checked" } else { "" }),
                Kind::Text if value.len() > 120 || value.contains('\n') => format!("<textarea name=\"{}\" rows=\"6\">{}</textarea>", escape(&c.name), escape(&value)),
                _ => format!("<input type=\"text\" name=\"{}\" value=\"{}\">", escape(&c.name), escape(&value)),
            };
            fields.push_str(&format!("<label>{} <small>({})</small>{input}</label>", escape(&c.name), escape(&hint)));
        }
        let action = match id {
            Some(id) => self.url(&[&table.name, id]),
            None => self.url(&[&table.name]),
        };
        let heading = match id {
            Some(id) => format!("Edit {} {id}", table.name),
            None => format!("New {}", table.name),
        };
        let body = format!("<form method=\"post\" action=\"{}\">{}{fields}<button>Save</button></form>", escape(&action), self.csrf());
        Ok(self.html(STATUS_CODE_REASON_PHRASE.n200_ok, &heading, &body))
    }

    fn save(&self, table: &TableSchema, id: Option<&str>) -> PageResult {
        let pk = self.pk(table).ok_or((STATUS_CODE_REASON_PHRASE.n404_not_found, format!("{} has no one-column primary key", table.name)))?;
        let backend = self.backend();
        let form = form_fields(&String::from_utf8_lossy(&self.request.body));
        let back = match id {
            Some(id) => self.url(&[&table.name, id, "edit"]),
            None => self.url(&[&table.name, "new"]),
        };
        let mut columns = Vec::new();
        let mut values = Vec::new();
        for c in table.columns.iter().filter(|c| self.editable(table, c, id.is_some())) {
            let given = form.get(&c.name).map(String::as_str);
            // `#[timestamps]` columns: TIMESTAMPTZ / DATETIME, or TEXT on SQLite.
            let stamp = matches!(kind(c), Kind::Timestamp | Kind::Text)
                && given.is_none_or(|v| v.trim().is_empty())
                && (c.name == "updated_at" || (c.name == "created_at" && id.is_none()));
            // An edit leaves what the form didn't send alone (a checkbox
            // sends nothing when unticked: that's `false`).
            if id.is_some() && given.is_none() && !stamp && kind(c) != Kind::Bool {
                continue;
            }
            let value = if stamp {
                crate::model::Timestamp::now().to_value(backend)
            } else {
                match parse(c, given) {
                    Ok(v) => v,
                    Err(e) => return Ok(Redirect::to(&back).alert(&e).into_response()),
                }
            };
            columns.push(c);
            values.push(value);
        }
        let result = match id {
            None => {
                let names: Vec<String> = columns.iter().map(|c| quote(backend, &c.name)).collect();
                let marks: Vec<String> = columns.iter().enumerate().map(|(i, c)| placeholder(backend, i + 1, c)).collect();
                let sql = if columns.is_empty() {
                    format!("INSERT INTO {} DEFAULT VALUES", quote(backend, &table.name))
                } else {
                    format!("INSERT INTO {} ({}) VALUES ({})", quote(backend, &table.name), names.join(", "), marks.join(", "))
                };
                self.execute(sql, values).map(|_| None)
            }
            Some(id) => {
                let key = parse(pk, Some(id)).map_err(|e| (STATUS_CODE_REASON_PHRASE.n404_not_found, e))?;
                let sets: Vec<String> = columns.iter().enumerate().map(|(i, c)| format!("{} = {}", quote(backend, &c.name), placeholder(backend, i + 1, c))).collect();
                let sql = format!("UPDATE {} SET {} WHERE {} = {}", quote(backend, &table.name), sets.join(", "), quote(backend, &pk.name), placeholder(backend, columns.len() + 1, pk));
                values.push(key);
                self.execute(sql, values).map(|n| Some((n, id.to_string())))
            }
        };
        match result {
            Err(e) => Ok(Redirect::to(&back).alert(&format!("The database refused it: {e}")).into_response()),
            Ok(Some((0, id))) => Err((STATUS_CODE_REASON_PHRASE.n404_not_found, format!("no {} {id}", table.name))),
            Ok(Some((_, id))) => Ok(Redirect::to(&self.url(&[&table.name, &id])).notice("Saved.").into_response()),
            Ok(None) => Ok(Redirect::to(&self.url(&[&table.name])).notice("Created.").into_response()),
        }
    }

    fn delete(&self, table: &TableSchema, id: &str) -> PageResult {
        let pk = self.pk(table).ok_or((STATUS_CODE_REASON_PHRASE.n404_not_found, format!("{} has no one-column primary key", table.name)))?;
        let backend = self.backend();
        let key = parse(pk, Some(id)).map_err(|e| (STATUS_CODE_REASON_PHRASE.n404_not_found, e))?;
        let sql = format!("DELETE FROM {} WHERE {} = {}", quote(backend, &table.name), quote(backend, &pk.name), placeholder(backend, 1, pk));
        match self.execute(sql, vec![key]) {
            Ok(0) => Err((STATUS_CODE_REASON_PHRASE.n404_not_found, format!("no {} {id}", table.name))),
            Ok(_) => Ok(Redirect::to(&self.url(&[&table.name])).notice("Deleted.").into_response()),
            Err(e) => Ok(Redirect::to(&self.url(&[&table.name, id])).alert(&format!("The database refused it: {e}")).into_response()),
        }
    }
}

/// An urlencoded form's fields (the last of a repeated name wins).
fn form_fields(text: &str) -> HashMap<String, String> {
    fn decode(text: &str) -> Option<String> {
        let bytes = text.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'+' => {
                    out.push(b' ');
                    i += 1;
                }
                b'%' => {
                    out.push(u8::from_str_radix(text.get(i + 1..i + 3)?, 16).ok()?);
                    i += 3;
                }
                b => {
                    out.push(b);
                    i += 1;
                }
            }
        }
        String::from_utf8(out).ok()
    }
    text.split('&')
        .filter(|p| !p.is_empty())
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            Some((decode(k)?, decode(v)?))
        })
        .collect()
}
