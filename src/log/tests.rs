use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use crate::app::App;
use crate::header::Header;
use crate::http::VERSION;
use crate::log::Log;
use crate::request::{METHOD, Request};

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
    };

    let (response, request) = App::handle_request(request);

    let timestamp = response._get_header(Header::_DATE_UNIX_EPOCH_NANOS.to_string()).unwrap();
    let expected_log = format!("\n\nRequest (peer address is 0.0.0.0:0):\n  HTTP/1.1 GET /script.js  \n  Host: 127.0.0.1:7878\n  User-Agent: SOME USER AGENT\n  Accept: */*\n  Accept-Language: en-US,en;q=0.5\n  Accept-Encoding: gzip, deflate, br\n  Referer: https://127.0.0.1:7878/\nEnd of Request\nResponse:\n  200 OK \n  Accept-CH: Sec-CH-UA-Arch, Sec-CH-UA-Bitness, Sec-CH-UA-Full-Version-List, Sec-CH-UA-Model, Sec-CH-UA-Platform-Version, Downlink, ECT, RTT, Save-Data, Device-Memory\n  Vary: Origin, Sec-CH-UA-Arch, Sec-CH-UA-Bitness, Sec-CH-UA-Full-Version-List, Sec-CH-UA-Model, Sec-CH-UA-Platform-Version, Save-Data, Device-Memory, Upgrade-Insecure-Requests\n  X-Content-Type-Options: nosniff\n  Accept-Ranges: bytes\n  X-Frame-Options: SAMEORIGIN\n  Date-Unix-Epoch-Nanos: {}\n\n  Body: 1 part(s), 117 byte(s) total\nEnd of Response", timestamp.value);

    let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
    let log = Log::request_response(&request, &response, &peer_addr);

    assert_eq!(expected_log, log);
}

#[test]
fn usage_info() {
    let expected_info = "Usage:\n\n  RWS_CONFIG_PORT environment variable\n  -p or --port as command line line argument\n  Port\n\n  RWS_CONFIG_IP environment variable\n  -i or --ip as command line line argument\n  IP or domain\n\n  RWS_CONFIG_THREAD_COUNT environment variable\n  -t or --thread-count as command line line argument\n  Number of threads\n\n  RWS_CONFIG_CORS_ALLOW_ALL environment variable\n  -a or --cors-allow-all as command line line argument\n  If set to true, will allow all CORS requests, other CORS properties will be ignored\n\n  RWS_CONFIG_CORS_ALLOW_ORIGINS environment variable\n  -o or --cors-allow-origins as command line line argument\n  Comma separated list of allowed origins, example: https://foo.example,https://bar.example\n\n  RWS_CONFIG_CORS_ALLOW_METHODS environment variable\n  -m or --cors-allow-methods as command line line argument\n  Comma separated list of allowed methods, example: POST,PUT\n\n  RWS_CONFIG_CORS_ALLOW_HEADERS environment variable\n  -h or --cors-allow-headers as command line line argument\n  Comma separated list of allowed request headers, in lowercase, example: content-type,x-custom-header\n\n  RWS_CONFIG_CORS_ALLOW_CREDENTIALS environment variable\n  -c or --cors-allow-credentials as command line line argument\n  If set to true, will allow to transmit credentials via CORS requests\n\n  RWS_CONFIG_CORS_EXPOSE_HEADERS environment variable\n  -e or --cors-expose-headers as command line line argument\n  Comma separated list of allowed response headers, in lowercase, example: content-type,x-custom-header\n\n  RWS_CONFIG_CORS_MAX_AGE environment variable\n  -g or --cors-max-age as command line line argument\n  How long results of preflight requests can be cached (in seconds)\n\n  RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES environment variable\n  -r or --request-allocation-size-in-bytes as command line line argument\n  In bytes, how much memory to allocate for each request\n\nEnd of usage section\n\n".to_string();
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


    let expected_info = format!("HTTP to HTTPS with LetsEncrypt HTTP verification server\nVersion:       {}\nAuthors:       {}\nRepository:    {}\nDesciption:    {}\nRust Version:  {}\nLicense:       {}\n",
        VERSION,
        AUTHORS,
        REPOSITORY,
        DESCRIPTION,
        RUST_VERSION,
        LICENSE
    ).to_string();
    let actual_info = Log::info("HTTP to HTTPS with LetsEncrypt HTTP verification server");
    assert_eq!(expected_info, actual_info)
}