use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use file_ext::FileExt;
use crate::app::App;
use crate::core::New;
use crate::header::Header;
use crate::http::VERSION;
use crate::log::Log;
use crate::range::Range;
use crate::mime_type::MimeType;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

#[test]
fn log_request_response() {
    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/script.js".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![
            Header { name: Header::_HOST.to_string(), value: "127.0.0.1:7878".to_string() },
            Header { name: Header::_USER_AGENT.to_string(), value: "SOME USER AGENT".to_string() },
            Header { name: Header::_ACCEPT.to_string(), value: "*/*".to_string() },
            Header { name: Header::_ACCEPT_LANGUAGE.to_string(), value: "en-US,en;q=0.5".to_string() },
            Header { name: Header::_ACCEPT_ENCODING.to_string(), value: "gzip, deflate, br".to_string() },
            Header { name: Header::_REFERER.to_string(), value: "https://127.0.0.1:7878/".to_string() },
        ],
        body: vec![],
    };

    let (response, request) = App::handle_request(request);

    let working_directory = FileExt::get_static_filepath("").unwrap();
    let node_path = [working_directory.as_str(), "src/app/controller/script/script.js"];
    let path = FileExt::build_path(&node_path);
    let file_content = FileExt::read_file(&path).unwrap();
    let timestamp = response._get_header(Header::_DATE_UNIX_EPOCH_NANOS.to_string()).unwrap();
    let expected_log = format!("\n\nRequest (thread id: log::tests::log_request_response peer address is 0.0.0.0:0):\n  HTTP/1.1 GET /script.js  \n  Host: 127.0.0.1:7878\n  User-Agent: SOME USER AGENT\n  Accept: */*\n  Accept-Language: en-US,en;q=0.5\n  Accept-Encoding: gzip, deflate, br\n  Referer: https://127.0.0.1:7878/\n  Body: 0 byte(s) total (including default initialization vector)\nEnd of Request\nResponse:\n  200 OK \n  Accept-CH: Sec-CH-UA-Arch, Sec-CH-UA-Bitness, Sec-CH-UA-Full-Version-List, Sec-CH-UA-Model, Sec-CH-UA-Platform-Version, Downlink, ECT, RTT, Save-Data, Device-Memory, Sec-CH-Prefers-Reduced-Motion, Sec-CH-Prefers-Color-Scheme\n  Critical-CH: Sec-CH-UA-Arch, Sec-CH-UA-Bitness, Sec-CH-UA-Full-Version-List, Sec-CH-UA-Model, Sec-CH-UA-Platform-Version, Downlink, ECT, RTT, Save-Data, Device-Memory, Sec-CH-Prefers-Reduced-Motion, Sec-CH-Prefers-Color-Scheme\n  Vary: Origin, Sec-CH-UA-Arch, Sec-CH-UA-Bitness, Sec-CH-UA-Full-Version-List, Sec-CH-UA-Model, Sec-CH-UA-Platform-Version, Save-Data, Device-Memory, Upgrade-Insecure-Requests, Sec-CH-Prefers-Reduced-Motion, Sec-CH-Prefers-Color-Scheme\n  X-Content-Type-Options: nosniff\n  Accept-Ranges: bytes\n  X-Frame-Options: SAMEORIGIN\n  Date-Unix-Epoch-Nanos: {}\n  Cache-Control: no-store, no-cache, private, max-age=0, must-revalidate, proxy-revalidate\n  Referrer-Policy: strict-origin-when-cross-origin\n  Permissions-Policy: geolocation=(), microphone=(), camera=()\n  Content-Security-Policy: default-src 'self'\n\n  Body: 1 part(s), {} byte(s) total\nEnd of Response", timestamp.value, file_content.len());

    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let log = Log::request_response(&request, &response, &peer_addr);

    assert_eq!(expected_log, log);
}

#[test]
fn usage_info() {
    let expected_info = "Usage:\n\n  RWS_CONFIG_PORT environment variable\n  -p or --port as command line line argument\n  Port\n\n  RWS_CONFIG_IP environment variable\n  -i or --ip as command line line argument\n  IP or domain\n\n  RWS_CONFIG_THREAD_COUNT environment variable\n  -t or --thread-count as command line line argument\n  Number of threads\n\n  RWS_CONFIG_CORS_ALLOW_ALL environment variable\n  -a or --cors-allow-all as command line line argument\n  If set to true, will allow all CORS requests, other CORS properties will be ignored\n\n  RWS_CONFIG_CORS_ALLOW_ORIGINS environment variable\n  -o or --cors-allow-origins as command line line argument\n  Comma separated list of allowed origins, example: https://foo.example,https://bar.example\n\n  RWS_CONFIG_CORS_ALLOW_METHODS environment variable\n  -m or --cors-allow-methods as command line line argument\n  Comma separated list of allowed methods, example: POST,PUT\n\n  RWS_CONFIG_CORS_ALLOW_HEADERS environment variable\n  -h or --cors-allow-headers as command line line argument\n  Comma separated list of allowed request headers, in lowercase, example: content-type,x-custom-header\n\n  RWS_CONFIG_CORS_ALLOW_CREDENTIALS environment variable\n  -c or --cors-allow-credentials as command line line argument\n  If set to true, will allow to transmit credentials via CORS requests\n\n  RWS_CONFIG_CORS_EXPOSE_HEADERS environment variable\n  -e or --cors-expose-headers as command line line argument\n  Comma separated list of allowed response headers, in lowercase, example: content-type,x-custom-header\n\n  RWS_CONFIG_CORS_MAX_AGE environment variable\n  -g or --cors-max-age as command line line argument\n  How long results of preflight requests can be cached (in seconds)\n\n  RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES environment variable\n  -r or --request-allocation-size-in-bytes as command line line argument\n  In bytes, how much memory to allocate for each request\n\n  RWS_CONFIG_TLS_CERT_FILE environment variable\n  -s or --tls-cert-file as command line line argument\n  Path to TLS certificate PEM file (enables HTTPS and HTTP/2)\n\n  RWS_CONFIG_TLS_KEY_FILE environment variable\n  -k or --tls-key-file as command line line argument\n  Path to TLS private key PEM file (enables HTTPS and HTTP/2)\n\nEnd of usage section\n\n".to_string();
    let actual_info = Log::usage_information();
    assert_eq!(expected_info, actual_info)
}

#[test]
fn info() {
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


    let expected_info = format!("HTTP to HTTPS with LetsEncrypt HTTP verification server\nVersion:           {}\nAuthors:           {}\nRepository:        {}\nDesciption:        {}\nRust Version:      {}\nLicense:           {}\nUser:              {}\nWorking Directory: {}\n",
        VERSION,
        AUTHORS,
        REPOSITORY,
        DESCRIPTION,
        RUST_VERSION,
        LICENSE,
        user,
        working_directory
    ).to_string();
    let actual_info = Log::info("HTTP to HTTPS with LetsEncrypt HTTP verification server");
    assert_eq!(expected_info, actual_info)
}

#[test]
fn combined_log_format_includes_clf_fields() {
    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/static/test.txt".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let (response, request) = App::handle_request(request);
    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n200_ok.status_code);

    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 4567);
    let log = Log::combined(&request, &response, &peer_addr);

    // IP address
    assert!(log.starts_with("192.168.1.1 - - ["), "log should start with IP: {}", log);

    // Timestamp bracket: "IP - - [DD/Mon/YYYY:HH:MM:SS +0000]"
    let open = log.find('[').unwrap();
    let close = log.find(']').unwrap();
    let timestamp = &log[open + 1..close];
    // format: DD/Mon/YYYY:HH:MM:SS +0000  (26 chars)
    assert_eq!(timestamp.len(), 26, "unexpected timestamp length: {}", timestamp);
    assert!(timestamp.ends_with("+0000"), "timestamp should end with +0000: {}", timestamp);

    // Request line, status, size
    assert!(log.contains("\"GET /static/test.txt HTTP/1.1\""), "missing request line: {}", log);
    assert!(log.contains(" 200 "), "missing status 200: {}", log);
}

#[test]
fn combined_log_format_empty_body_uses_dash() {
    use crate::core::New;
    use crate::response::Response;

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/nonexistent".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let response = Response::new();
    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 80);
    let log = Log::combined(&request, &response, &peer_addr);

    // Empty body must appear as "-" per CLF spec
    assert!(log.ends_with(" -"), "empty body should end with ' -': {}", log);
}

// ── Log::json ─────────────────────────────────────────────────────────────────

fn make_request(method: &str, uri: &str) -> Request {
    Request {
        method: method.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    }
}

fn ok_response_with_body(body: &[u8]) -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r.content_range_list = vec![Range::get_content_range(body.to_vec(), MimeType::TEXT_PLAIN.to_string())];
    r
}

#[test]
fn json_contains_all_required_keys() {
    let req  = make_request("GET", "/api/users");
    let resp = ok_response_with_body(b"hello");
    let peer = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3)), 8080);

    let line = Log::json(&req, &resp, &peer);

    assert!(line.contains("\"time\""),        "missing time field");
    assert!(line.contains("\"remote_addr\""), "missing remote_addr field");
    assert!(line.contains("\"method\""),      "missing method field");
    assert!(line.contains("\"path\""),        "missing path field");
    assert!(line.contains("\"protocol\""),    "missing protocol field");
    assert!(line.contains("\"status\""),      "missing status field");
    assert!(line.contains("\"bytes\""),       "missing bytes field");
}

#[test]
fn json_values_match_request_and_response() {
    let req  = make_request("POST", "/submit");
    let resp = ok_response_with_body(b"ok");
    let peer = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)), 9000);

    let line = Log::json(&req, &resp, &peer);

    assert!(line.contains("\"remote_addr\":\"1.2.3.4\""), "wrong IP: {}", line);
    assert!(line.contains("\"method\":\"POST\""),          "wrong method: {}", line);
    assert!(line.contains("\"path\":\"/submit\""),         "wrong path: {}", line);
    assert!(line.contains("\"status\":200"),               "wrong status: {}", line);
    assert!(line.contains("\"bytes\":2"),                  "wrong byte count: {}", line);
}

#[test]
fn json_empty_body_reports_zero_bytes() {
    let req  = make_request("GET", "/empty");
    let resp = Response::new();
    let peer = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);

    let line = Log::json(&req, &resp, &peer);
    assert!(line.contains("\"bytes\":0"), "expected 0 bytes: {}", line);
}

#[test]
fn json_timestamp_is_iso8601_format() {
    let req  = make_request("GET", "/");
    let resp = Response::new();
    let peer = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);

    let line = Log::json(&req, &resp, &peer);

    // Extract the value of "time" from the JSON string
    let time_start = line.find("\"time\":\"").expect("no time field") + 8;
    let time_end   = line[time_start..].find('"').expect("time field not closed") + time_start;
    let ts = &line[time_start..time_end];

    // Must match YYYY-MM-DDThh:mm:ssZ (20 chars)
    assert_eq!(20, ts.len(), "unexpected timestamp length: {}", ts);
    assert!(ts.ends_with('Z'),           "timestamp must end with Z: {}", ts);
    assert_eq!(b'T', ts.as_bytes()[10],  "expected T separator at pos 10: {}", ts);
}

#[test]
fn json_escapes_special_chars_in_method_and_path() {
    // A method or path containing a quote or backslash must be escaped.
    let mut req = make_request("GET", "/path/with\"quote");
    req.method = "G\"ET".to_string();
    let resp = Response::new();
    let peer = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);

    let line = Log::json(&req, &resp, &peer);
    // The output must be valid JSON — raw unescaped `"` would break it.
    assert!(line.contains("\\\""), "expected escaped quote in output: {}", line);
}

#[test]
fn json_output_is_a_single_line() {
    let req  = make_request("DELETE", "/items/1");
    let resp = ok_response_with_body(b"deleted");
    let peer = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9)), 443);

    let line = Log::json(&req, &resp, &peer);
    assert!(!line.contains('\n'), "JSON log must be a single line: {}", line);
    assert!(line.starts_with('{'), "must start with {{");
    assert!(line.ends_with('}'),   "must end with }}");
}

// ── Log::log_access ───────────────────────────────────────────────────────────

#[test]
fn log_access_uses_combined_by_default() {
    // Ensure the env var is unset so we get combined format.
    std::env::remove_var(crate::entry_point::Config::RWS_CONFIG_LOG_FORMAT);

    let req  = make_request("GET", "/ping");
    let resp = ok_response_with_body(b"pong");
    let peer = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);

    // log_access writes to stdout — we just verify it doesn't panic.
    Log::log_access(&req, &resp, &peer);
}

#[test]
fn log_access_uses_json_when_env_var_set() {
    std::env::set_var(crate::entry_point::Config::RWS_CONFIG_LOG_FORMAT, "json");

    let req  = make_request("GET", "/ping");
    let resp = ok_response_with_body(b"pong");
    let peer = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);

    // Must not panic; JSON path is exercised by the env var.
    Log::log_access(&req, &resp, &peer);

    std::env::remove_var(crate::entry_point::Config::RWS_CONFIG_LOG_FORMAT);
}

// ── Log::server_url_thread_count ──────────────────────────────────────────────

#[test]
fn server_url_thread_count_contains_protocol_and_address() {
    let msg = Log::server_url_thread_count("https", &"127.0.0.1:7878".to_string(), 4);
    assert!(msg.contains("https://127.0.0.1:7878"), "missing URL: {}", msg);
    assert!(msg.contains("4"),                       "missing thread count: {}", msg);
}

#[test]
fn server_url_thread_count_two_lines() {
    let msg = Log::server_url_thread_count("http", &"0.0.0.0:80".to_string(), 8);
    let lines: Vec<&str> = msg.lines().collect();
    assert_eq!(2, lines.len(), "expected exactly 2 lines: {:?}", lines);
    assert!(lines[0].contains("http://0.0.0.0:80"));
    assert!(lines[1].contains("8"));
}