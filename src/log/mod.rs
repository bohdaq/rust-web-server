use std::net::SocketAddr;
use std::thread;
use file_ext::FileExt;
use crate::entry_point::command_line_args::CommandLineArgument;
use crate::request::Request;
use crate::response::Response;

#[cfg(test)]
mod tests;

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
            let argument_info = format!("  {} environment variable\n  -{} or --{} as command line line argument\n  {}\n\n", arg.environment_variable, arg.short_form, arg.long_form, arg._hint.unwrap());
            log = [log, argument_info].join("");
        }
        log = [log, "End of usage section\n\n".to_string()].join("");
        log
    }

    pub fn info(name: &str) -> String {
        let mut log = format!("{}\n", name).to_string();
        const VERSION: &str = env!("CARGO_PKG_VERSION");
        const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
        const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
        const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");
        const RUST_VERSION: &str = env!("CARGO_PKG_RUST_VERSION");
        const LICENSE: &str = env!("CARGO_PKG_LICENSE");
        let boxed_user = FileExt::get_current_user();
        if boxed_user.is_err() {
            let message = boxed_user.as_ref().err().unwrap();
            eprintln!("{}", message)
        }
        let user: String = boxed_user.unwrap();

        let boxed_working_directory = FileExt::get_static_filepath("");
        if boxed_working_directory.is_err() {
            let message = boxed_working_directory.as_ref().err().unwrap();
            eprintln!("{}", message)
        }

        let working_directory: String = boxed_working_directory.unwrap();

        let version = format!("Version:           {}\n", VERSION);
        log = [log, version].join("");

        let authors = format!("Authors:           {}\n", AUTHORS);
        log = [log, authors].join("");

        let repository = format!("Repository:        {}\n", REPOSITORY);
        log = [log, repository].join("");

        let description = format!("Desciption:        {}\n", DESCRIPTION);
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

    /// Returns a Combined Log Format (CLF) line for one request/response pair:
    /// `IP - - [DD/Mon/YYYY:HH:MM:SS +0000] "METHOD URI VERSION" STATUS SIZE`
    pub fn combined(request: &Request, response: &Response, peer_addr: &SocketAddr) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};

        let body_size: usize = response.content_range_list.iter()
            .map(|cr| cr.body.len())
            .sum();

        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let timestamp = Log::format_clf_timestamp(secs);

        let body_str = if body_size > 0 { body_size.to_string() } else { "-".to_string() };

        format!("{} - - [{}] \"{} {} {}\" {} {}",
            peer_addr.ip(),
            timestamp,
            request.method,
            request.request_uri,
            request.http_version,
            response.status_code,
            body_str,
        )
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