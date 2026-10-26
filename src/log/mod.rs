use std::net::SocketAddr;
use std::thread;
use crate::entry_point::command_line_args::CommandLineArgument;
use crate::request::Request;
use crate::response::Response;

#[cfg(test)]
mod tests;
pub(crate) mod access_sink;

/// Logging helpers for access logs and server info.
///
/// [`Log::combined`] produces standard Combined Log Format lines compatible with
/// GoAccess, AWStats, and similar tools. All three server code paths (HTTP/1.1,
/// HTTP/2, HTTP/3) use it.
pub struct Log;

impl Log {
    pub fn request_response(request: &Request, response: &Response, peer_addr: &SocketAddr) -> String {
        let mut request_headers = "".to_string();
        for header in &request.headers {
            if &header.name.chars().count() > &0 {
                request_headers = [
                    request_headers,
                    "\n  ".to_string(),
                    header.name.to_string(),
                    ": ".to_string(),
                    header.value.to_string()
                ].join("");
            }
        }

        let mut response_headers = "".to_string();
        for header in &response.headers {
            if &header.name.chars().count() > &0 {
                response_headers = [
                    response_headers,
                    "\n  ".to_string(),
                    header.name.to_string(),
                    ": ".to_string(),
                    header.value.to_string()
                ].join("");
            }
        }

        let mut response_body_length = 0;
        let mut response_body_parts_number = 0;
        for content_range in &response.content_range_list {
            let boxed_parse = content_range.size.parse::<i32>();
            if boxed_parse.is_ok() {
                response_body_length += boxed_parse.unwrap();
                response_body_parts_number += 1;
            }
        }

        let current_thread = thread::current();
        let thread_id = current_thread.name().unwrap();

        let log_request_response = format!("\n\nRequest (thread id: {} peer address is {}):\n  {} {} {}  {}\n  Body: {} byte(s) total (including default initialization vector)\nEnd of Request\nResponse:\n  {} {} {}\n\n  Body: {} part(s), {} byte(s) total\nEnd of Response",
                                           thread_id,
                                           peer_addr,
                                           &request.http_version,
                                           &request.method,
                                           &request.request_uri,
                                           request_headers,
                                           request.body.len(),

                                           &response.status_code,
                                           &response.reason_phrase,
                                           response_headers,
                                           response_body_parts_number,
                                           response_body_length);

        log_request_response
    }

    pub fn usage_information() -> String {
        let mut log = "Usage:\n\n".to_string();
        let command_line_arg_list = CommandLineArgument::get_command_line_arg_list();
        for arg in command_line_arg_list {
            let flags = if arg.short_form.is_empty() {
                format!("--{}", arg.long_form)
            } else {
                format!("-{} or --{}", arg.short_form, arg.long_form)
            };
            let argument_info = format!("  {} environment variable\n  {} as command line line argument\n  {}\n\n", arg.environment_variable, flags, arg._hint.unwrap());
            log = [log, argument_info].join("");
        }
        log = [log, "End of usage section\n\n".to_string()].join("");
        log
    }

    /// The user rws runs as, for the startup banner: `$USER`, `$LOGNAME` or
    /// `%USERNAME%`, else `"unknown"`. Deliberately not `whoami` (what
    /// `FileExt::get_current_user` runs): minimal container images such as
    /// distroless have no `whoami`, and the banner's `unwrap()` on that
    /// error crashed the published Docker image at startup.
    pub fn current_user() -> String {
        ["USER", "LOGNAME", "USERNAME"]
            .iter()
            .find_map(|var| std::env::var(var).ok().filter(|v| !v.trim().is_empty()))
            .unwrap_or_else(|| "unknown".to_string())
    }

    pub fn info(name: &str) -> String {
        let mut log = format!("{}\n", name).to_string();
        const VERSION: &str = env!("CARGO_PKG_VERSION");
        const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
        const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
        const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");
        const RUST_VERSION: &str = env!("CARGO_PKG_RUST_VERSION");
        const LICENSE: &str = env!("CARGO_PKG_LICENSE");
        let user = Log::current_user();
        let working_directory = crate::app::controller::static_resource::working_directory().to_string();

        let version = format!("Version:           {}\n", VERSION);
        log = [log, version].join("");

        let authors = format!("Authors:           {}\n", AUTHORS);
        log = [log, authors].join("");

        let repository = format!("Repository:        {}\n", REPOSITORY);
        log = [log, repository].join("");

        let description = format!("Description:       {}\n", DESCRIPTION);
        log = [log, description].join("");

        let rust_version = format!("Rust Version:      {}\n", RUST_VERSION);
        log = [log, rust_version].join("");

        let license = format!("License:           {}\n", LICENSE);
        log = [log, license].join("");

        let license = format!("User:              {}\n", user);
        log = [log, license].join("");

        let license = format!("Working Directory: {}\n", working_directory);
        log = [log, license].join("");

        log
    }

    /// Returns a structured JSON log line for one request/response pair.
    ///
    /// Enable with `RWS_CONFIG_LOG_FORMAT=json`. Useful for log aggregators
    /// (Loki, Fluentd, Datadog) that ingest JSON from pod stdout.
    pub fn json(request: &Request, response: &Response, peer_addr: &SocketAddr) -> String {
        let mut line = Vec::with_capacity(160);
        Log::write_json(&mut line, request, response, peer_addr);
        bytes_to_string(line)
    }

    /// [`Log::json`] appended to `out`, without intermediate strings.
    fn write_json(out: &mut Vec<u8>, request: &Request, response: &Response, peer_addr: &SocketAddr) {
        use std::io::Write;
        let body_size: usize = response.content_range_list.iter().map(|cr| cr.body.len()).sum();
        out.extend_from_slice(b"{\"time\":\"");
        with_timestamps(|iso, _| out.extend_from_slice(iso.as_bytes()));
        out.extend_from_slice(b"\",\"remote_addr\":\"");
        let _ = write!(out, "{}", peer_addr.ip());
        out.extend_from_slice(b"\",\"method\":\"");
        push_json_escaped(out, &request.method);
        out.extend_from_slice(b"\",\"path\":\"");
        push_json_escaped(out, &request.request_uri);
        out.extend_from_slice(b"\",\"protocol\":\"");
        push_json_escaped(out, &request.http_version);
        out.extend_from_slice(b"\",\"status\":");
        let _ = write!(out, "{}", response.status_code);
        out.extend_from_slice(b",\"bytes\":");
        out.extend_from_slice(crate::ext::decimal::Decimal::new(body_size as u128).as_str().as_bytes());
        out.push(b'}');
    }

    /// Writes one access log line to stdout using the format configured by
    /// `RWS_CONFIG_LOG_FORMAT` (`"combined"` or `"json"`). Call this instead of
    /// `Log::combined` so that format selection is centralised.
    ///
    /// Lines are buffered per thread and written by a background thread every
    /// 100 ms (and when a thread's buffer passes 32 KiB, when the thread
    /// exits, and at process exit); [`Log::flush_access_log`] writes them now.
    pub fn log_access(request: &Request, response: &Response, peer_addr: &SocketAddr) {
        let config = crate::server_config::ServerConfig::current();
        let format = &config.log_format;
        if format.eq_ignore_ascii_case("off") || format.eq_ignore_ascii_case("none") {
            return;
        }
        let json = format == "json";
        access_sink::write_line(|out| {
            if json {
                Log::write_json(out, request, response, peer_addr)
            } else {
                Log::write_combined(out, request, response, peer_addr)
            }
        });
    }

    /// Have `SIGUSR1` reopen the access-log file (`RWS_CONFIG_ACCESS_LOG`)
    /// before its next write, for logrotate; does nothing when the log goes
    /// to stdout. The `rws` binary calls it at startup.
    pub fn reopen_access_log_on_sigusr1() {
        access_sink::install_reopen_signal();
    }

    /// Reopen the access-log file before its next write — what `SIGUSR1`
    /// does, for a program that moves the file itself.
    pub fn reopen_access_log() {
        access_sink::reopen();
    }

    /// Write out the buffered access-log lines now (they're otherwise written
    /// within 100 ms) — e.g. before exiting without returning from `main`.
    pub fn flush_access_log() {
        access_sink::flush();
    }

    /// The access-log line for `RWS_CONFIG_LOG_FORMAT`: `json` (the `rws`
    /// binary's default), Combined Log Format for any other value (including
    /// unset, in library use), or `None` for `off`/`none` — no per-request
    /// line, e.g. behind a proxy that already logs or when benchmarking.
    /// Taken from `ServerConfig::current()`, so it's hot-reloadable.
    pub fn access_log_line(request: &Request, response: &Response, peer_addr: &SocketAddr) -> Option<String> {
        let config = crate::server_config::ServerConfig::current();
        let format = &config.log_format;
        if format.eq_ignore_ascii_case("off") || format.eq_ignore_ascii_case("none") {
            return None;
        }
        Some(if format == "json" {
            Log::json(request, response, peer_addr)
        } else {
            Log::combined(request, response, peer_addr)
        })
    }

    fn format_iso8601(secs: u64) -> String {
        let sec = secs % 60;
        let min = (secs / 60) % 60;
        let hour = (secs / 3600) % 24;
        let days = secs / 86400;
        let (year, month, day) = Log::days_to_ymd(days);
        format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", year, month, day, hour, min, sec)
    }

    /// Returns a Combined Log Format (CLF) line for one request/response pair:
    /// `IP - - [DD/Mon/YYYY:HH:MM:SS +0000] "METHOD URI VERSION" STATUS SIZE`
    pub fn combined(request: &Request, response: &Response, peer_addr: &SocketAddr) -> String {
        let mut line = Vec::with_capacity(128);
        Log::write_combined(&mut line, request, response, peer_addr);
        bytes_to_string(line)
    }

    /// [`Log::combined`] appended to `out`, without intermediate strings.
    fn write_combined(out: &mut Vec<u8>, request: &Request, response: &Response, peer_addr: &SocketAddr) {
        use std::io::Write;
        let body_size: usize = response.content_range_list.iter().map(|cr| cr.body.len()).sum();
        let _ = write!(out, "{}", peer_addr.ip());
        out.extend_from_slice(b" - - [");
        with_timestamps(|_, clf| out.extend_from_slice(clf.as_bytes()));
        out.extend_from_slice(b"] \"");
        out.extend_from_slice(request.method.as_bytes());
        out.push(b' ');
        out.extend_from_slice(request.request_uri.as_bytes());
        out.push(b' ');
        out.extend_from_slice(request.http_version.as_bytes());
        out.extend_from_slice(b"\" ");
        let _ = write!(out, "{}", response.status_code);
        out.push(b' ');
        if body_size > 0 {
            out.extend_from_slice(crate::ext::decimal::Decimal::new(body_size as u128).as_str().as_bytes());
        } else {
            out.push(b'-');
        }
    }

    fn format_clf_timestamp(secs: u64) -> String {
        let sec = secs % 60;
        let min = (secs / 60) % 60;
        let hour = (secs / 3600) % 24;
        let days = secs / 86400;
        let (year, month, day) = Log::days_to_ymd(days);
        const MONTHS: [&str; 12] = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
        format!("{:02}/{}/{:04}:{:02}:{:02}:{:02} +0000",
            day, MONTHS[(month - 1) as usize], year, hour, min, sec)
    }

    fn days_to_ymd(days: u64) -> (u64, u64, u64) {
        let z = days + 719468;
        let era = z / 146097;
        let doe = z % 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };
        (y, m, d)
    }

    pub fn server_url_thread_count(protocol: &str, bind_addr: &String, thread_count: i32) -> String {
        let url = format!("Server is up and running at: {}://{}\n", protocol, &bind_addr);
        let thread_count = format!("Spawned {} thread(s) to handle incoming requests\n", thread_count);
        [url, thread_count].join("")
    }
}
/// The current second's ISO 8601 and Combined Log Format timestamps,
/// formatted at most once per second per thread.
fn with_timestamps(use_them: impl FnOnce(&str, &str)) {
    use std::cell::RefCell;
    use std::time::{SystemTime, UNIX_EPOCH};
    thread_local! {
        static CACHED: RefCell<(u64, String, String)> = const { RefCell::new((u64::MAX, String::new(), String::new())) };
    }
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    CACHED.with(|cached| {
        let mut cached = cached.borrow_mut();
        if cached.0 != secs {
            *cached = (secs, Log::format_iso8601(secs), Log::format_clf_timestamp(secs));
        }
        use_them(&cached.1, &cached.2);
    });
}

/// `s` JSON-escaped as the access log always did it: backslash, double
/// quote, newline and carriage return.
fn push_json_escaped(out: &mut Vec<u8>, s: &str) {
    for b in s.bytes() {
        match b {
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'"' => out.extend_from_slice(b"\\\""),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            _ => out.push(b),
        }
    }
}

/// A line built only from `&str` pieces is UTF-8.
fn bytes_to_string(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}
