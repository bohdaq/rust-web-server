use std::net::SocketAddr;
use file_ext::FileExt;
use crate::entry_point::command_line_args::CommandLineArgument;
use crate::request::Request;
use crate::response::Response;

#[cfg(test)]
mod tests;

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

        let log_request_response = format!("\n\nRequest (peer address is {}):\n  {} {} {}  {}\nEnd of Request\nResponse:\n  {} {} {}\n\n  Body: {} part(s), {} byte(s) total\nEnd of Response",
                                           peer_addr,
                                           &request.http_version,
                                           &request.method,
                                           &request.request_uri,
                                           request_headers,

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

    pub fn server_url_thread_count(protocol: &str, bind_addr: &String, thread_count: i32) -> String {
        let url = format!("Server is up and running at: {}://{}\n", protocol, &bind_addr);
        let thread_count = format!("Spawned {} thread(s) to handle incoming requests\n", thread_count);
        [url, thread_count].join("")
    }
}