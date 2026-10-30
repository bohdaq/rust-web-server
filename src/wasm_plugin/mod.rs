//! WebAssembly middleware (18.10.0, #40, feature `wasm-plugins`): a
//! [`WasmLayer`] runs a core WebAssembly module on each request and,
//! optionally, each response. The module may let the request pass, change
//! its headers or target, answer it itself, or change the response's status
//! and headers.
//!
//! **Isolation.** Plugins get no imports at all — no WASI, no clock, no
//! files, no network: a module that imports anything is refused at load.
//! They see only what the host hands them. CPU per call is bounded by fuel,
//! memory by a limit on the instance's linear memory. A trap, running out
//! of fuel or memory, or output the host can't read answers `500` and
//! counts in `rws_wasm_plugin_errors_total`; it never takes the server down.
//! Each request gets a fresh instance, so nothing leaks from one request to
//! the next.
//!
//! **ABI** (version 1). The module exports:
//!
//! | Export | Signature | |
//! |---|---|---|
//! | `memory` | memory | required |
//! | `rws_alloc` | `(len: i32) -> i32` | required: a buffer of `len` bytes the host writes input into |
//! | `rws_on_request` | `(ptr: i32, len: i32) -> i64` | required |
//! | `rws_on_response` | `(ptr: i32, len: i32) -> i64` | optional |
//! | `rws_init` | `(ptr: i32, len: i32)` | optional: called once per instance with the `config` string |
//!
//! A hook returns `0` to change nothing, or `(ptr << 32) | len` of its
//! output in its memory. The request a hook gets:
//!
//! ```text
//! GET /path?query HTTP/1.1
//! 203.0.113.7                  ← the client address
//! host: example.com            ← every header, "name: value"
//! user-agent: curl/8.11.1
//!                              ← blank line; the body isn't passed
//! ```
//!
//! The response: its status code on the first line, then its headers and a
//! blank line. Output is one directive per line:
//!
//! | Directive | In | Does |
//! |---|---|---|
//! | `set <name>: <value>` | both | replace the header |
//! | `add <name>: <value>` | both | add a header |
//! | `remove <name>` | both | remove the header |
//! | `uri <target>` | request | change the request target (origin form, `/…`) |
//! | `status <code>` | response | change the status |
//! | `respond <code>` | request | answer now: header lines (`name: value`) follow, then a blank line and the body |
//!
//! Anything else is an error (fail closed). Header names and values with
//! CR, LF or NUL are refused.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use wasmtime::{Config, Engine, Instance, InstancePre, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, Trap, TypedFunc};

use crate::application::Application;
use crate::header::Header;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

/// Fuel per call when not configured: roughly tens of milliseconds of CPU.
pub const DEFAULT_FUEL: u64 = 50_000_000;
/// Linear memory limit when not configured, in MiB.
pub const DEFAULT_MEMORY_MB: u64 = 16;
/// The most output a hook may return.
pub const MAX_OUTPUT: usize = 1024 * 1024;

fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut config = Config::new();
        config.consume_fuel(true);
        Engine::new(&config).expect("a wasmtime engine with fuel")
    })
}

/// What went wrong in a plugin call (the metric's `kind` label).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginError {
    /// Ran out of fuel.
    Fuel,
    /// Hit the memory limit.
    Memory,
    /// Any other trap (`unreachable`, out-of-bounds access, …).
    Trap(String),
    /// Output the host couldn't read.
    Output(String),
}

impl PluginError {
    fn kind(&self) -> &'static str {
        match self {
            PluginError::Fuel => "fuel",
            PluginError::Memory => "memory",
            PluginError::Trap(_) => "trap",
            PluginError::Output(_) => "output",
        }
    }
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginError::Fuel => write!(f, "ran out of fuel"),
            PluginError::Memory => write!(f, "hit its memory limit"),
            PluginError::Trap(t) => write!(f, "trapped: {t}"),
            PluginError::Output(o) => write!(f, "bad output: {o}"),
        }
    }
}

/// A request hook's decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestAction {
    Pass,
    /// Header edits and an optional new target.
    Modify { edits: Vec<Edit>, uri: Option<String> },
    Respond { status: u16, headers: Vec<(String, String)>, body: Vec<u8> },
}

/// A response hook's decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseAction {
    pub edits: Vec<Edit>,
    pub status: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    Set(String, String),
    Add(String, String),
    Remove(String),
}

fn valid_token(s: &str) -> bool {
    !s.is_empty() && !s.bytes().any(|b| b == b'\r' || b == b'\n' || b == 0)
}

fn header_pair(rest: &str) -> Result<(String, String), PluginError> {
    let (name, value) = rest.split_once(':').ok_or_else(|| PluginError::Output(format!("expected name: value, got {rest:?}")))?;
    let (name, value) = (name.trim(), value.trim());
    if !valid_token(name) || name.contains(' ') || value.bytes().any(|b| b == b'\r' || b == b'\n' || b == 0) {
        return Err(PluginError::Output(format!("bad header {name:?}")));
    }
    Ok((name.to_string(), value.to_string()))
}

fn status_code(s: &str) -> Result<u16, PluginError> {
    match s.trim().parse::<u16>() {
        Ok(code) if (100..=599).contains(&code) => Ok(code),
        _ => Err(PluginError::Output(format!("bad status {s:?}"))),
    }
}

/// A request hook's output, decoded. Public for the fuzz target.
pub fn parse_request_output(output: &[u8]) -> Result<RequestAction, PluginError> {
    if output.is_empty() {
        return Ok(RequestAction::Pass);
    }
    let text_end = output.len();
    let mut edits = Vec::new();
    let mut uri = None;
    let mut offset = 0;
    while offset < text_end {
        let end = output[offset..].iter().position(|&b| b == b'\n').map_or(text_end, |p| offset + p);
        let line = std::str::from_utf8(&output[offset..end]).map_err(|_| PluginError::Output("not UTF-8".to_string()))?;
        let line = line.strip_suffix('\r').unwrap_or(line);
        offset = (end + 1).min(text_end);
        if line.is_empty() {
            continue;
        }
        let (directive, rest) = line.split_once(' ').unwrap_or((line, ""));
        match directive {
            "set" => {
                let (n, v) = header_pair(rest)?;
                edits.push(Edit::Set(n, v));
            }
            "add" => {
                let (n, v) = header_pair(rest)?;
                edits.push(Edit::Add(n, v));
            }
            "remove" if valid_token(rest.trim()) => edits.push(Edit::Remove(rest.trim().to_string())),
            "uri" if rest.starts_with('/') && valid_token(rest) && !rest.contains(' ') => uri = Some(rest.to_string()),
            "respond" => {
                let status = status_code(rest)?;
                let mut headers = Vec::new();
                loop {
                    if offset >= text_end {
                        return Ok(RequestAction::Respond { status, headers, body: Vec::new() });
                    }
                    let end = output[offset..].iter().position(|&b| b == b'\n').map_or(text_end, |p| offset + p);
                    let line = std::str::from_utf8(&output[offset..end]).map_err(|_| PluginError::Output("not UTF-8".to_string()))?;
                    let line = line.strip_suffix('\r').unwrap_or(line);
                    offset = (end + 1).min(text_end);
                    if line.is_empty() {
                        let body = if end < text_end { output[offset..].to_vec() } else { Vec::new() };
                        return Ok(RequestAction::Respond { status, headers, body });
                    }
                    headers.push(header_pair(line)?);
                }
            }
            _ => return Err(PluginError::Output(format!("unknown directive {line:?}"))),
        }
    }
    Ok(if edits.is_empty() && uri.is_none() { RequestAction::Pass } else { RequestAction::Modify { edits, uri } })
}

/// A response hook's output, decoded. Public for the fuzz target.
pub fn parse_response_output(output: &[u8]) -> Result<ResponseAction, PluginError> {
    let text = std::str::from_utf8(output).map_err(|_| PluginError::Output("not UTF-8".to_string()))?;
    let mut action = ResponseAction { edits: Vec::new(), status: None };
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let (directive, rest) = line.split_once(' ').unwrap_or((line, ""));
        match directive {
            "set" => {
                let (n, v) = header_pair(rest)?;
                action.edits.push(Edit::Set(n, v));
            }
            "add" => {
                let (n, v) = header_pair(rest)?;
                action.edits.push(Edit::Add(n, v));
            }
            "remove" if valid_token(rest.trim()) => action.edits.push(Edit::Remove(rest.trim().to_string())),
            "status" => action.status = Some(status_code(rest)?),
            _ => return Err(PluginError::Output(format!("unknown directive {line:?}"))),
        }
    }
    Ok(action)
}

fn apply(headers: &mut Vec<Header>, edits: &[Edit]) {
    for edit in edits {
        match edit {
            Edit::Set(name, value) => {
                headers.retain(|h| !h.name.eq_ignore_ascii_case(name));
                headers.push(Header::new(name.clone(), value.clone()));
            }
            Edit::Add(name, value) => headers.push(Header::new(name.clone(), value.clone())),
            Edit::Remove(name) => headers.retain(|h| !h.name.eq_ignore_ascii_case(name)),
        }
    }
}

fn request_input(request: &Request, connection: &ConnectionInfo) -> Vec<u8> {
    let mut out = format!("{} {} {}\n{}\n", request.method, request.request_uri, request.http_version, connection.client.ip).into_bytes();
    for h in &request.headers {
        out.extend_from_slice(h.name.to_ascii_lowercase().as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(h.value.as_bytes());
        out.push(b'\n');
    }
    out.push(b'\n');
    out
}

fn response_input(response: &Response) -> Vec<u8> {
    let mut out = format!("{}\n", response.status_code).into_bytes();
    let content_type = response.content_range_list.first().map(|p| p.content_type.clone()).filter(|c| !c.is_empty());
    if let Some(ct) = content_type.filter(|_| !response.headers.iter().any(|h| h.name.eq_ignore_ascii_case("content-type"))) {
        out.extend_from_slice(format!("content-type: {ct}\n").as_bytes());
    }
    for h in &response.headers {
        out.extend_from_slice(h.name.to_ascii_lowercase().as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(h.value.as_bytes());
        out.push(b'\n');
    }
    out.push(b'\n');
    out
}

struct State {
    limits: StoreLimits,
}

/// One instance of the plugin, for one request.
struct Call {
    store: Store<State>,
    instance: Instance,
}

impl Call {
    fn error(&self, e: wasmtime::Error) -> PluginError {
        match e.downcast_ref::<Trap>() {
            Some(Trap::OutOfFuel) => PluginError::Fuel,
            Some(Trap::AllocationTooLarge) => PluginError::Memory,
            Some(trap) => PluginError::Trap(trap.to_string()),
            None => {
                // A grow past the limit (trap_on_grow_failure) is an error
                // whose cause names it, not a Trap.
                let chain = format!("{e:#}");
                if chain.contains("grow") || (chain.contains("memory") && chain.contains("limit")) {
                    PluginError::Memory
                } else {
                    PluginError::Trap(chain.lines().next().unwrap_or_default().to_string())
                }
            }
        }
    }

    fn write_input(&mut self, input: &[u8]) -> Result<(i32, i32), PluginError> {
        let alloc: TypedFunc<i32, i32> = self
            .instance
            .get_typed_func(&mut self.store, "rws_alloc")
            .map_err(|e| PluginError::Output(format!("no rws_alloc: {e}")))?;
        let len = i32::try_from(input.len()).map_err(|_| PluginError::Output("input too large".to_string()))?;
        let ptr = alloc.call(&mut self.store, len).map_err(|e| self.error(e))?;
        let memory = self.instance.get_memory(&mut self.store, "memory").ok_or_else(|| PluginError::Output("no memory export".to_string()))?;
        memory
            .write(&mut self.store, ptr as u32 as usize, input)
            .map_err(|_| PluginError::Output("rws_alloc returned a buffer outside memory".to_string()))?;
        Ok((ptr, len))
    }

    /// Call hook `name` with `input`; `None` when the module doesn't export it.
    fn hook(&mut self, name: &str, input: &[u8]) -> Result<Option<Vec<u8>>, PluginError> {
        let Ok(func) = self.instance.get_typed_func::<(i32, i32), i64>(&mut self.store, name) else {
            return Ok(None);
        };
        let (ptr, len) = self.write_input(input)?;
        let packed = func.call(&mut self.store, (ptr, len)).map_err(|e| self.error(e))?;
        if packed == 0 {
            return Ok(Some(Vec::new()));
        }
        let (out_ptr, out_len) = ((packed as u64 >> 32) as usize, (packed as u64 & 0xffff_ffff) as usize);
        if out_len > MAX_OUTPUT {
            return Err(PluginError::Output(format!("output of {out_len} bytes is over {MAX_OUTPUT}")));
        }
        let memory = self.instance.get_memory(&mut self.store, "memory").ok_or_else(|| PluginError::Output("no memory export".to_string()))?;
        let data = memory.data(&self.store);
        let end = out_ptr.checked_add(out_len).filter(|&e| e <= data.len()).ok_or_else(|| PluginError::Output("output outside memory".to_string()))?;
        Ok(Some(data[out_ptr..end].to_vec()))
    }
}

struct Plugin {
    name: String,
    pre: InstancePre<State>,
    fuel: u64,
    memory_bytes: usize,
    config: Vec<u8>,
    has_response_hook: bool,
}

/// WebAssembly middleware. See the [module docs](self).
#[derive(Clone)]
pub struct WasmLayer {
    plugin: Arc<Plugin>,
}

impl std::fmt::Debug for WasmLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmLayer").field("name", &self.plugin.name).field("fuel", &self.plugin.fuel).finish_non_exhaustive()
    }
}

impl WasmLayer {
    /// Compile `wasm` (binary module bytes) as plugin `name`. Refused: a
    /// module that imports anything, or lacks `memory`, `rws_alloc` or
    /// `rws_on_request` of the right types.
    pub fn new(name: impl Into<String>, wasm: &[u8]) -> Result<WasmLayer, String> {
        let name = name.into();
        let module = Module::new(engine(), wasm).map_err(|e| format!("plugin {name}: not a valid WebAssembly module: {e}"))?;
        if let Some(import) = module.imports().next() {
            return Err(format!(
                "plugin {name}: imports {}::{}; plugins get no imports (no WASI)",
                import.module(),
                import.name()
            ));
        }
        let exports: Vec<(String, wasmtime::ExternType)> = module.exports().map(|e| (e.name().to_string(), e.ty())).collect();
        let func = |n: &str| exports.iter().find(|(name, _)| name == n).and_then(|(_, t)| t.func().cloned());
        let shape = |n: &str, params: &[wasmtime::ValType], results: &[wasmtime::ValType]| {
            func(n).is_some_and(|f| {
                f.params().map(|p| p.to_string()).collect::<Vec<_>>() == params.iter().map(|p| p.to_string()).collect::<Vec<_>>()
                    && f.results().map(|p| p.to_string()).collect::<Vec<_>>() == results.iter().map(|p| p.to_string()).collect::<Vec<_>>()
            })
        };
        use wasmtime::ValType::{I32, I64};
        if !exports.iter().any(|(n, t)| n == "memory" && t.memory().is_some()) {
            return Err(format!("plugin {name}: exports no memory"));
        }
        if !shape("rws_alloc", &[I32], &[I32]) {
            return Err(format!("plugin {name}: needs rws_alloc(len: i32) -> i32"));
        }
        if !shape("rws_on_request", &[I32, I32], &[I64]) {
            return Err(format!("plugin {name}: needs rws_on_request(ptr: i32, len: i32) -> i64"));
        }
        if func("rws_on_response").is_some() && !shape("rws_on_response", &[I32, I32], &[I64]) {
            return Err(format!("plugin {name}: rws_on_response must be (ptr: i32, len: i32) -> i64"));
        }
        if func("rws_init").is_some() && !shape("rws_init", &[I32, I32], &[]) {
            return Err(format!("plugin {name}: rws_init must be (ptr: i32, len: i32)"));
        }
        let pre = Linker::new(engine()).instantiate_pre(&module).map_err(|e| format!("plugin {name}: {e}"))?;
        Ok(WasmLayer {
            plugin: Arc::new(Plugin {
                has_response_hook: func("rws_on_response").is_some(),
                name,
                pre,
                fuel: DEFAULT_FUEL,
                memory_bytes: (DEFAULT_MEMORY_MB * 1024 * 1024) as usize,
                config: Vec::new(),
            }),
        })
    }

    /// [`Self::new`] from a `.wasm` file; the plugin is named after it.
    pub fn from_file(path: &str) -> Result<WasmLayer, String> {
        let wasm = std::fs::read(path).map_err(|e| format!("plugin {path}: {e}"))?;
        let name = std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string());
        WasmLayer::new(name, &wasm)
    }

    fn edit(self, f: impl FnOnce(&mut Plugin)) -> WasmLayer {
        let mut plugin = Plugin {
            name: self.plugin.name.clone(),
            pre: self.plugin.pre.clone(),
            fuel: self.plugin.fuel,
            memory_bytes: self.plugin.memory_bytes,
            config: self.plugin.config.clone(),
            has_response_hook: self.plugin.has_response_hook,
        };
        f(&mut plugin);
        WasmLayer { plugin: Arc::new(plugin) }
    }

    /// Fuel per hook call (default [`DEFAULT_FUEL`]).
    pub fn fuel(self, fuel: u64) -> WasmLayer {
        self.edit(|p| p.fuel = fuel.max(1))
    }

    /// Linear memory limit in MiB (default [`DEFAULT_MEMORY_MB`]).
    pub fn memory_mb(self, mb: u64) -> WasmLayer {
        self.edit(|p| p.memory_bytes = (mb.max(1) * 1024 * 1024) as usize)
    }

    /// The string `rws_init` gets.
    pub fn config(self, config: impl Into<String>) -> WasmLayer {
        let config = config.into().into_bytes();
        self.edit(|p| p.config = config)
    }

    pub fn name(&self) -> &str {
        &self.plugin.name
    }

    fn instantiate(&self) -> Result<Call, PluginError> {
        let p = &self.plugin;
        let limits = StoreLimitsBuilder::new().memory_size(p.memory_bytes).instances(1).tables(4).memories(1).trap_on_grow_failure(true).build();
        let mut store = Store::new(engine(), State { limits });
        store.limiter(|s| &mut s.limits);
        store.set_fuel(p.fuel).map_err(|e| PluginError::Trap(e.to_string()))?;
        let instance = match p.pre.instantiate(&mut store) {
            Ok(instance) => instance,
            Err(e) => {
                let text = format!("{e:#}");
                return Err(if text.contains("memory") { PluginError::Memory } else if text.contains("fuel") { PluginError::Fuel } else { PluginError::Trap(text) });
            }
        };
        let mut call = Call { store, instance };
        if !p.config.is_empty() {
            if let Ok(init) = call.instance.get_typed_func::<(i32, i32), ()>(&mut call.store, "rws_init") {
                let (ptr, len) = call.write_input(&p.config)?;
                init.call(&mut call.store, (ptr, len)).map_err(|e| call.error(e))?;
            }
        }
        Ok(call)
    }

    /// Run the request hook on its own (for tests and embedders).
    pub fn on_request(&self, request: &Request, connection: &ConnectionInfo) -> Result<RequestAction, PluginError> {
        let mut call = self.instantiate()?;
        let output = call.hook("rws_on_request", &request_input(request, connection))?.unwrap_or_default();
        parse_request_output(&output)
    }

    fn failed(&self, hook: &'static str, e: &PluginError) -> Response {
        metrics::error(&self.plugin.name, e.kind());
        eprintln!("wasm plugin {} ({hook}) {e}", self.plugin.name);
        Response::text(STATUS_CODE_REASON_PHRASE.n500_internal_server_error, "500 Internal Server Error: a plugin failed")
    }
}

impl Middleware for WasmLayer {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        metrics::call(&self.plugin.name, "request");
        let mut call = match self.instantiate() {
            Ok(call) => call,
            Err(e) => return Ok(self.failed("request", &e)),
        };
        let action = call
            .hook("rws_on_request", &request_input(request, connection))
            .and_then(|output| parse_request_output(&output.unwrap_or_default()));
        let mut response = match action {
            Err(e) => return Ok(self.failed("request", &e)),
            Ok(RequestAction::Respond { status, headers, body }) => {
                let reason = STATUS_CODE_REASON_PHRASE_FOR(status);
                let content_type = headers
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case("content-type"))
                    .map(|(_, v)| v.clone())
                    .unwrap_or_else(|| "text/plain".to_string());
                let mut response = Response::get_response(reason, None, Some(vec![crate::range::Range::get_content_range(body, content_type)]));
                response.status_code = status as i16;
                for (name, value) in headers.into_iter().filter(|(n, _)| !n.eq_ignore_ascii_case("content-type") && !n.eq_ignore_ascii_case("content-length")) {
                    response.headers.push(Header::new(name, value));
                }
                return Ok(response);
            }
            Ok(RequestAction::Pass) => next.execute(request, connection)?,
            Ok(RequestAction::Modify { edits, uri }) => {
                let mut changed = request.clone();
                apply(&mut changed.headers, &edits);
                if let Some(uri) = uri {
                    changed.request_uri = uri;
                }
                next.execute(&changed, connection)?
            }
        };
        if self.plugin.has_response_hook {
            metrics::call(&self.plugin.name, "response");
            let action = call
                .hook("rws_on_response", &response_input(&response))
                .and_then(|output| parse_response_output(&output.unwrap_or_default()));
            match action {
                Err(e) => return Ok(self.failed("response", &e)),
                Ok(action) => {
                    apply(&mut response.headers, &action.edits);
                    if let Some(status) = action.status {
                        response.status_code = status as i16;
                        response.reason_phrase = STATUS_CODE_REASON_PHRASE_FOR(status).reason_phrase.to_string();
                    }
                }
            }
        }
        Ok(response)
    }

    /// Headers only: a streamed request body passes through untouched.
    fn accepts_streamed_body(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }
}

/// The reason phrase table entry for `status` (a generic one for codes
/// without a standard phrase).
#[allow(non_snake_case)]
fn STATUS_CODE_REASON_PHRASE_FOR(status: u16) -> &'static crate::response::StatusCodeReasonPhrase {
    crate::response::Response::status_code_reason_phrase_list()
        .into_iter()
        .find(|s| *s.status_code as u16 == status)
        .unwrap_or(STATUS_CODE_REASON_PHRASE.n200_ok)
}

/// `rws_wasm_plugin_*` series.
pub(crate) mod metrics {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Stats {
        calls: BTreeMap<(String, &'static str), u64>,
        errors: BTreeMap<(String, &'static str), u64>,
    }

    static ANY: AtomicU64 = AtomicU64::new(0);

    fn stats() -> &'static Mutex<Stats> {
        static STATS: OnceLock<Mutex<Stats>> = OnceLock::new();
        STATS.get_or_init(Mutex::default)
    }

    pub(crate) fn call(plugin: &str, hook: &'static str) {
        ANY.fetch_add(1, Ordering::Relaxed);
        *stats().lock().unwrap_or_else(|e| e.into_inner()).calls.entry((plugin.to_string(), hook)).or_default() += 1;
    }

    pub(crate) fn error(plugin: &str, kind: &'static str) {
        *stats().lock().unwrap_or_else(|e| e.into_inner()).errors.entry((plugin.to_string(), kind)).or_default() += 1;
    }

    fn label(value: &str) -> String {
        value.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
    }

    pub(crate) fn prometheus_text() -> String {
        if ANY.load(Ordering::Relaxed) == 0 {
            return String::new();
        }
        let stats = stats().lock().unwrap_or_else(|e| e.into_inner());
        let mut out = String::from("# HELP rws_wasm_plugin_calls_total WebAssembly plugin hook calls\n# TYPE rws_wasm_plugin_calls_total counter\n");
        for ((plugin, hook), n) in &stats.calls {
            out.push_str(&format!("rws_wasm_plugin_calls_total{{plugin=\"{}\",hook=\"{hook}\"}} {n}\n", label(plugin)));
        }
        if !stats.errors.is_empty() {
            out.push_str("# HELP rws_wasm_plugin_errors_total WebAssembly plugin calls that failed (answered 500), by kind: fuel, memory, trap, output\n# TYPE rws_wasm_plugin_errors_total counter\n");
            for ((plugin, kind), n) in &stats.errors {
                out.push_str(&format!("rws_wasm_plugin_errors_total{{plugin=\"{}\",kind=\"{kind}\"}} {n}\n", label(plugin)));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests;
