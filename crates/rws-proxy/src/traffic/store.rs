//! Raw page views and script reports in PostgreSQL (opt-in, 18.31.0):
//! `[traffic] database_url_env` names the environment variable holding a
//! `postgres://` URL. A background thread writes in batches; a full queue
//! drops rows (counted) rather than slowing requests. With it, the admin API
//! answers any date range and filter combination from SQL
//! (`GET /traffic/query`), past `retention_days`.
//!
//! Rows hold no IP address and no User-Agent: the visitor is the day's
//! salted hash (it changes every day and at every restart), the browser
//! only its family.
//!
//! ```sql
//! rws_traffic_pageviews(at timestamptz, host, path, visitor bigint,
//!   session_start timestamptz, referrer_host, channel, utm_source,
//!   utm_medium, utm_campaign, country, browser, os, device)
//! rws_traffic_reports(at timestamptz, host, path, visitor bigint, kind,
//!   name, props, secs, scroll, width)
//! ```

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::model::{DbPool, Value};

/// Rows waiting to be written; past it new rows are dropped.
pub const QUEUE: usize = 20_000;
/// Rows written in one statement.
pub const BATCH: usize = 500;
/// How long a partial batch waits.
pub const FLUSH: Duration = Duration::from_secs(10);

/// A person's page view.
#[derive(Clone, Debug, Default, PartialEq)]
#[doc(hidden)]
pub struct PageView {
    pub at: u64,
    pub host: String,
    pub path: String,
    pub visitor: u64,
    pub session_start: u64,
    pub referrer_host: Option<String>,
    pub channel: Option<&'static str>,
    pub utm: [Option<String>; 3],
    pub country: Option<String>,
    pub browser: Option<&'static str>,
    pub os: Option<&'static str>,
    pub device: Option<&'static str>,
}

/// A report from the first-party script.
#[derive(Clone, Debug, Default, PartialEq)]
#[doc(hidden)]
pub struct Report {
    pub at: u64,
    pub host: String,
    pub path: String,
    pub visitor: u64,
    /// `leave` or `event`.
    pub kind: &'static str,
    pub name: Option<String>,
    /// `key=value` pairs, `&`-separated.
    pub props: Option<String>,
    pub secs: Option<u64>,
    pub scroll: Option<u64>,
    pub width: Option<u64>,
}

#[derive(Debug)]
#[doc(hidden)]
pub enum Row {
    PageView(PageView),
    Report(Report),
}

static ON: AtomicBool = AtomicBool::new(false);
static DROPPED: AtomicU64 = AtomicU64::new(0);
static WRITTEN: AtomicU64 = AtomicU64::new(0);
static FAILED: AtomicU64 = AtomicU64::new(0);

fn sender() -> &'static Mutex<Option<SyncSender<Row>>> {
    static SENDER: OnceLock<Mutex<Option<SyncSender<Row>>>> = OnceLock::new();
    SENDER.get_or_init(|| Mutex::new(None))
}

fn pool_slot() -> &'static Mutex<Option<DbPool>> {
    static POOL: OnceLock<Mutex<Option<DbPool>>> = OnceLock::new();
    POOL.get_or_init(|| Mutex::new(None))
}

/// Whether rows are being written.
pub fn enabled() -> bool {
    ON.load(Ordering::Relaxed)
}

/// Rows written, dropped (queue full) and lost to write errors.
pub fn stats() -> (u64, u64, u64) {
    (WRITTEN.load(Ordering::Relaxed), DROPPED.load(Ordering::Relaxed), FAILED.load(Ordering::Relaxed))
}

/// Queue a row; never blocks.
#[doc(hidden)]
pub fn enqueue(row: Row) {
    if !enabled() {
        return;
    }
    let guard = sender().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(tx) = guard.as_ref() {
        if let Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) = tx.try_send(row) {
            DROPPED.fetch_add(1, Ordering::Relaxed);
        }
    }
}

const SCHEMA: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS rws_traffic_pageviews (at timestamptz NOT NULL, host text NOT NULL, path text NOT NULL, visitor bigint NOT NULL, \
     session_start timestamptz NOT NULL, referrer_host text, channel text, utm_source text, utm_medium text, utm_campaign text, country text, \
     browser text, os text, device text)",
    "CREATE INDEX IF NOT EXISTS rws_traffic_pageviews_at ON rws_traffic_pageviews (at)",
    "CREATE TABLE IF NOT EXISTS rws_traffic_reports (at timestamptz NOT NULL, host text NOT NULL, path text NOT NULL, visitor bigint NOT NULL, \
     kind text NOT NULL, name text, props text, secs bigint, scroll bigint, width bigint)",
    "CREATE INDEX IF NOT EXISTS rws_traffic_reports_at ON rws_traffic_reports (at)",
];

fn text(v: &Option<String>) -> Value {
    v.clone().map_or(Value::Null, Value::Text)
}

fn word(v: Option<&'static str>) -> Value {
    v.map_or(Value::Null, |s| Value::Text(s.to_string()))
}

fn number(v: Option<u64>) -> Value {
    v.map_or(Value::Null, |n| Value::Int(n as i64))
}

/// A multi-row INSERT: `VALUES (to_timestamp($1), $2, …), (…)`; columns in
/// `bigints` get `::bigint` (a NULL parameter is sent as text).
#[doc(hidden)]
pub fn insert_sql(table: &str, columns: &[&str], rows: usize, timestamps: &[usize], bigints: &[usize]) -> String {
    let mut sql = format!("INSERT INTO {table} ({}) VALUES ", columns.join(", "));
    let mut n = 0;
    for r in 0..rows {
        if r > 0 {
            sql.push_str(", ");
        }
        sql.push('(');
        for c in 0..columns.len() {
            n += 1;
            if c > 0 {
                sql.push_str(", ");
            }
            if timestamps.contains(&c) {
                sql.push_str(&format!("to_timestamp(${n})"));
            } else if bigints.contains(&c) {
                sql.push_str(&format!("${n}::bigint"));
            } else {
                sql.push_str(&format!("${n}"));
            }
        }
        sql.push(')');
    }
    sql
}

async fn write(pool: &DbPool, rows: Vec<Row>) -> Result<u64, String> {
    let (mut views, mut reports) = (Vec::new(), Vec::new());
    for row in rows {
        match row {
            Row::PageView(v) => views.push(v),
            Row::Report(r) => reports.push(r),
        }
    }
    let mut written = 0;
    if !views.is_empty() {
        let columns = ["at", "host", "path", "visitor", "session_start", "referrer_host", "channel", "utm_source", "utm_medium", "utm_campaign", "country", "browser", "os", "device"];
        let mut params = Vec::with_capacity(views.len() * columns.len());
        for v in &views {
            params.extend([
                Value::Int(v.at as i64),
                Value::Text(v.host.clone()),
                Value::Text(v.path.clone()),
                Value::Int(v.visitor as i64),
                Value::Int(v.session_start as i64),
                text(&v.referrer_host),
                word(v.channel),
                text(&v.utm[0]),
                text(&v.utm[1]),
                text(&v.utm[2]),
                text(&v.country),
                word(v.browser),
                word(v.os),
                word(v.device),
            ]);
        }
        written += pool.execute(&insert_sql("rws_traffic_pageviews", &columns, views.len(), &[0, 4], &[3]), &params).await.map_err(|e| e.to_string())?;
    }
    if !reports.is_empty() {
        let columns = ["at", "host", "path", "visitor", "kind", "name", "props", "secs", "scroll", "width"];
        let mut params = Vec::with_capacity(reports.len() * columns.len());
        for r in &reports {
            params.extend([
                Value::Int(r.at as i64),
                Value::Text(r.host.clone()),
                Value::Text(r.path.clone()),
                Value::Int(r.visitor as i64),
                Value::Text(r.kind.to_string()),
                text(&r.name),
                text(&r.props),
                number(r.secs),
                number(r.scroll),
                number(r.width),
            ]);
        }
        written += pool.execute(&insert_sql("rws_traffic_reports", &columns, reports.len(), &[0], &[3, 7, 8, 9]), &params).await.map_err(|e| e.to_string())?;
    }
    Ok(written)
}

fn writer(pool: DbPool, rx: Receiver<Row>) {
    let mut batch = Vec::with_capacity(BATCH);
    let mut since = Instant::now();
    loop {
        let wait = FLUSH.saturating_sub(since.elapsed());
        let closed = match rx.recv_timeout(wait) {
            Ok(row) => {
                batch.push(row);
                false
            }
            Err(RecvTimeoutError::Timeout) => false,
            Err(RecvTimeoutError::Disconnected) => true,
        };
        if batch.len() >= BATCH || (!batch.is_empty() && (since.elapsed() >= FLUSH || closed)) {
            let rows = std::mem::take(&mut batch);
            let n = rows.len() as u64;
            let pool = pool.clone();
            match rws_server::async_bridge::block_on_shared(async move { write(&pool, rows).await }) {
                Ok(_) => {
                    WRITTEN.fetch_add(n, Ordering::Relaxed);
                }
                Err(e) => {
                    FAILED.fetch_add(n, Ordering::Relaxed);
                    eprintln!("traffic: can't write {n} rows to PostgreSQL: {e}");
                }
            }
        }
        if batch.is_empty() {
            since = Instant::now();
        }
        if closed {
            return;
        }
    }
}

/// Connect, create the tables and start the writer. Errors are reported
/// and leave the store off; counting goes on without it.
pub fn start(url: &str) -> Result<(), String> {
    let url = url.to_string();
    let pool = rws_server::async_bridge::block_on_shared(async move {
        let pool = DbPool::connect(&url).await.map_err(|e| e.to_string())?;
        for statement in SCHEMA {
            pool.execute(statement, &[]).await.map_err(|e| e.to_string())?;
        }
        Ok::<_, String>(pool)
    })?;
    let (tx, rx) = sync_channel(QUEUE);
    *sender().lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
    *pool_slot().lock().unwrap_or_else(|e| e.into_inner()) = Some(pool.clone());
    std::thread::Builder::new().name("rws-traffic-db".to_string()).spawn(move || writer(pool, rx)).map_err(|e| e.to_string())?;
    ON.store(true, Ordering::Relaxed);
    Ok(())
}

/// Stop taking rows and write what's queued (at shutdown, in tests).
pub fn stop() {
    ON.store(false, Ordering::Relaxed);
    // Dropping the sender ends the writer after its last batch.
    sender().lock().unwrap_or_else(|e| e.into_inner()).take();
}

/// Wait until everything queued so far has been written (tests): polls the
/// written counter.
#[doc(hidden)]
pub fn wait_written(n: u64, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if WRITTEN.load(Ordering::Relaxed) >= n {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

// ── querying ──────────────────────────────────────────────────────────────────

/// A query from `GET /traffic/query`: a date range (inclusive, UTC days)
/// and any of the filters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    /// Seconds since the epoch, start of the first day.
    pub from: u64,
    /// Seconds since the epoch, end of the last day (exclusive).
    pub to: u64,
    pub host: Option<String>,
    /// A path, or a prefix ending in `*`.
    pub path: Option<String>,
    pub country: Option<String>,
    pub channel: Option<String>,
    pub utm_source: Option<String>,
    pub utm_campaign: Option<String>,
    pub browser: Option<String>,
    pub os: Option<String>,
    pub device: Option<String>,
}

#[doc(hidden)]
pub use super::summary::parse_day;

impl Query {
    /// The WHERE clause and its parameters (starting at `$1`).
    #[doc(hidden)]
    pub fn filter(&self) -> (String, Vec<Value>) {
        let mut clauses = vec!["at >= to_timestamp($1)".to_string(), "at < to_timestamp($2)".to_string()];
        let mut params = vec![Value::Int(self.from as i64), Value::Int(self.to as i64)];
        let mut add = |column: &str, value: &Option<String>, prefix: bool| {
            if let Some(v) = value.as_ref().filter(|v| !v.is_empty()) {
                params.push(Value::Text(match (prefix, v.strip_suffix('*')) {
                    (true, Some(p)) => format!("{}%", p.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")),
                    _ => v.clone(),
                }));
                let n = params.len();
                if prefix && v.ends_with('*') {
                    clauses.push(format!("{column} LIKE ${n}"));
                } else {
                    clauses.push(format!("{column} = ${n}"));
                }
            }
        };
        add("host", &self.host.as_ref().map(|h| h.to_ascii_lowercase()), false);
        add("path", &self.path, true);
        add("country", &self.country.as_ref().map(|c| c.to_ascii_uppercase()), false);
        add("channel", &self.channel, false);
        add("utm_source", &self.utm_source, false);
        add("utm_campaign", &self.utm_campaign, false);
        add("browser", &self.browser, false);
        add("os", &self.os, false);
        add("device", &self.device, false);
        (clauses.join(" AND "), params)
    }
}

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The query's answer: totals (page views, visitors, sessions, bounces),
/// a daily series, and top lists — as JSON.
pub fn query_json(q: &Query) -> Result<String, String> {
    let pool = pool_slot().lock().unwrap_or_else(|e| e.into_inner()).clone().ok_or("no traffic database (set [traffic] database_url_env)")?;
    let (filter, params) = q.filter();
    let (from, to) = (q.from, q.to);
    rws_server::async_bridge::block_on_shared(async move {
        let one = |sql: String| {
            let pool = pool.clone();
            let params = params.clone();
            async move { pool.query_rows(&sql, &params).await.map_err(|e| format!("database: {e}")) }
        };
        let totals = one(format!(
            "SELECT count(*)::bigint AS views, count(DISTINCT (visitor, date_trunc('day', at)))::bigint AS visitors, \
             count(DISTINCT (visitor, session_start))::bigint AS sessions FROM rws_traffic_pageviews WHERE {filter}"
        ))
        .await?;
        let bounces = one(format!(
            "SELECT count(*)::bigint AS n FROM (SELECT 1 FROM rws_traffic_pageviews WHERE {filter} GROUP BY visitor, session_start HAVING count(*) = 1) s"
        ))
        .await?;
        let series = one(format!(
            "SELECT (extract(epoch FROM date_trunc('day', at))::bigint) AS day, count(*)::bigint AS views, count(DISTINCT visitor)::bigint AS visitors \
             FROM rws_traffic_pageviews WHERE {filter} GROUP BY 1 ORDER BY 1"
        ))
        .await?;
        let get = |rows: &[crate::model::ModelRow], col: &str| rows.first().and_then(|r| r.get::<i64>(col).ok()).unwrap_or(0);
        let mut lists = Vec::new();
        for (name, column) in [("pages", "host || path"), ("countries", "coalesce(country, '(unknown)')"), ("channels", "coalesce(channel, '(none)')"), ("referrers", "referrer_host"), ("utm_source", "utm_source"), ("utm_campaign", "utm_campaign"), ("browsers", "browser"), ("os", "os"), ("devices", "device")] {
            let rows = one(format!(
                "SELECT {column} AS k, count(*)::bigint AS n FROM rws_traffic_pageviews WHERE {filter} AND {column} IS NOT NULL GROUP BY 1 ORDER BY 2 DESC, 1 LIMIT 25"
            ))
            .await?;
            let pairs: Vec<String> = rows.iter().filter_map(|r| Some(format!("[{},{}]", quote(&r.get::<String>("k").ok()?), r.get::<i64>("n").ok()?))).collect();
            lists.push(format!("{}:[{}]", quote(name), pairs.join(",")));
        }
        let points: Vec<String> = series
            .iter()
            .filter_map(|r| Some(format!(r#"{{"t":{},"views":{},"visitors":{}}}"#, r.get::<i64>("day").ok()?, r.get::<i64>("views").ok()?, r.get::<i64>("visitors").ok()?)))
            .collect();
        Ok(format!(
            r#"{{"from":{},"to":{},"views":{},"visitors":{},"sessions":{},"bounces":{},"series":[{}],{}}}"#,
            from,
            to,
            get(&totals, "views"),
            get(&totals, "visitors"),
            get(&totals, "sessions"),
            get(&bounces, "n"),
            points.join(","),
            lists.join(",")
        ))
    })
}
