use crate::app::App;
use crate::application::Application;
use crate::client_hint::ClientHint;
use crate::cors::Cors;
use crate::entry_point::config_file::override_environment_variables_from_config;
use crate::header::Header;
use crate::request::{METHOD, Request};
use crate::http::VERSION;
use crate::response::STATUS_CODE_REASON_PHRASE;
use crate::server::{Address, ConnectionInfo};
use crate::server_config::ServerConfig;

#[test]
fn not_found() {
    let _g = crate::test_env::lock();
    override_environment_variables_from_config(Some("/src/test/app/rws.config.toml"));

    let request = Request {
        method: METHOD.post.to_string(),
        request_uri: "/some/path".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: Default::default(),
    };

    let (response, _request) = App::handle_request(request);
    for header in response.headers {
        println!("{:?}", header);
    }
    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n404_not_found.status_code);
}

#[test]
fn static_file() {
    let _g = crate::test_env::lock();
    override_environment_variables_from_config(Some("/src/test/app/rws.config.toml"));

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/static/content.png".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: Default::default(),
    };

    let (response, _request) = App::handle_request(request);
    for header in response.headers {
        println!("{:?}", header);
    }
    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n200_ok.status_code);
}

#[test]
fn index() {
    let _g = crate::test_env::lock();
    override_environment_variables_from_config(Some("/src/test/app/rws.config.toml"));

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: Default::default(),
    };

    let (response, _request) = App::handle_request(request);
    for header in response.headers {
        println!("{:?}", header);
    }
    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n200_ok.status_code);
}

#[test]
fn static_file_cors_options_preflight_request_client_hints() {
    let _g = crate::test_env::lock();
    override_environment_variables_from_config(Some("/src/test/app/rws.config.toml"));

    let origin_value = "origin-value.com";
    let custom_header = "X-CUSTOM-HEADER";

    let expected_allow_headers = format!("{},{}", Header::_CONTENT_TYPE, custom_header);


    let request = Request {
        method: METHOD.options.to_string(),
        request_uri: "/static/content.png".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![
            Header {
                name: Header::_ORIGIN.into(),
                value: origin_value.to_string().into()
            },
            Header {
                name: Header::_ACCESS_CONTROL_REQUEST_METHOD.into(),
                value: METHOD.post.to_string().into()
            },
            Header {
                name: Header::_ACCESS_CONTROL_REQUEST_HEADERS.into(),
                value: expected_allow_headers.into()
            },
        ],
        body: Default::default(),
    };

    let (response, _request) = App::handle_request(request);

    for header in &response.headers {
        println!("{:?}", header);
    }

    let allow_origins = response._get_header(Header::_ACCESS_CONTROL_ALLOW_ORIGIN.to_string()).unwrap();
    assert_eq!(origin_value, allow_origins.value);

    let allow_headers = response._get_header(Header::_ACCESS_CONTROL_ALLOW_HEADERS.to_string()).unwrap();
    let expected_allow_headers = format!("{},{}", Header::_CONTENT_TYPE.to_lowercase(), custom_header.to_lowercase());
    assert_eq!(expected_allow_headers, allow_headers.value);

    let allow_credentials = response._get_header(Header::_ACCESS_CONTROL_ALLOW_CREDENTIALS.to_string()).unwrap();
    assert_eq!("true", allow_credentials.value);

    let expose_headers = response._get_header(Header::_ACCESS_CONTROL_EXPOSE_HEADERS.to_string()).unwrap();
    let expected_expose_headers = format!("{},{}", Header::_CONTENT_TYPE.to_lowercase(), custom_header.to_lowercase());
    assert_eq!(expected_expose_headers, expose_headers.value);

    let max_age = response._get_header(Header::_ACCESS_CONTROL_MAX_AGE.to_string()).unwrap();
    assert_eq!(Cors::MAX_AGE, max_age.value);

    // Client hints are opt-in since 18.0.0 (RWS_CONFIG_CLIENT_HINTS).
    let vary_header = response._get_header(Header::_VARY.to_string()).unwrap();
    assert_eq!(vary_header.value, "Origin");
    assert!(response._get_header(ClientHint::ACCEPT_CLIENT_HINTS.to_string()).is_none());
    assert!(response._get_header(ClientHint::CRITICAL_CLIENT_HINTS.to_string()).is_none());


    // Opt-in since 18.0.0 (RWS_CONFIG_SECURITY_HEADERS).
    assert!(response._get_header(Header::_X_FRAME_OPTIONS.to_string()).is_none());

    for header in &response.headers {
        println!("{:?}", header);
    }
    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n204_no_content.status_code);
}

#[test]
fn static_file_cors_off_options_preflight_request_client_hints() {
    // Uses App::with_config — no env writes, no test_env::lock needed.
    let origin_value = "origin-value.com";
    let custom_header = "X-CUSTOM-HEADER";
    let expected_allow_headers = format!("{},{}", Header::_CONTENT_TYPE, custom_header);

    let config = ServerConfig {
        cors_allow_all: false,
        cors_allow_origins: String::new(),   // no allowed origins → CORS denied
        ..ServerConfig::default()
    };
    let app = App::with_config(config);
    let conn = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 16000,
        sni_hostname: None,
    };
    let request = Request {
        method: METHOD.options.to_string(),
        request_uri: "/static/content.png".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![
            Header { name: Header::_ORIGIN.into(), value: origin_value.to_string().into() },
            Header { name: Header::_ACCESS_CONTROL_REQUEST_METHOD.into(), value: METHOD.post.to_string().into() },
            Header { name: Header::_ACCESS_CONTROL_REQUEST_HEADERS.into(), value: expected_allow_headers.into() },
        ],
        body: Default::default(),
    };
    let response = app.execute(&request, &conn).unwrap();

    // No CORS headers should be present (origin not in allowed list)
    assert!(response._get_header(Header::_ACCESS_CONTROL_ALLOW_ORIGIN.to_string()).is_none());

    // Client hints are opt-in since 18.0.0 (RWS_CONFIG_CLIENT_HINTS).
    let vary_header = response._get_header(Header::_VARY.to_string()).unwrap();
    assert_eq!(vary_header.value, "Origin");
    assert!(response._get_header(ClientHint::ACCEPT_CLIENT_HINTS.to_string()).is_none());
    assert!(response._get_header(ClientHint::CRITICAL_CLIENT_HINTS.to_string()).is_none());


    // Opt-in since 18.0.0 (RWS_CONFIG_SECURITY_HEADERS).
    assert!(response._get_header(Header::_X_FRAME_OPTIONS.to_string()).is_none());

    for header in response.headers {
        println!("{:?}", header);
    }
    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n204_no_content.status_code);
}


// ── Builder method tests ───────────────────────────────────────────────────────

mod builder_tests {
    use crate::app::App;
    use crate::application::Application;
    use crate::core::New;
    use crate::http::VERSION;
    use crate::middleware::{Middleware, RateLimitLayer};
    use crate::mime_type::MimeType;
    use crate::range::Range;
    use crate::request::{METHOD, Request};
    use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
    use crate::server::{Address, ConnectionInfo};

    fn conn() -> ConnectionInfo {
        ConnectionInfo {
            client: Address { ip: "10.0.0.1".to_string(), port: 0 },
            server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
            request_size: 16000,
        sni_hostname: None,
        }
    }

    fn get(uri: &str) -> Request {
        Request {
            method: METHOD.get.to_string(),
            request_uri: uri.to_string(),
            http_version: VERSION.http_1_1.to_string(),
            headers: vec![],
            body: Default::default(),
        }
    }

    fn ok_text(s: &str) -> Response {
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        r.content_range_list = vec![Range::get_content_range(s.as_bytes().to_vec(), MimeType::TEXT_PLAIN.to_string())];
        r
    }

    #[test]
    fn app_wrap_applies_middleware() {
        struct AddHeader;
        impl Middleware for AddHeader {
            fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
                let mut resp = next.execute(request, connection)?;
                resp.headers.push(crate::header::Header { name: "X-Test".into(), value: "yes".into() });
                Ok(resp)
            }
        }

        let app = App::new().wrap(AddHeader);
        let resp = app.execute(&get("/healthz"), &conn()).unwrap();
        assert_eq!(200, resp.status_code);
        assert!(resp.headers.iter().any(|h| h.name == "X-Test" && h.value == "yes"));
    }

    #[test]
    fn app_wrap_chains_multiple_layers() {
        use std::sync::{Arc, Mutex};
        let log: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(vec![]));

        struct Mark { label: &'static str, log: Arc<Mutex<Vec<&'static str>>> }
        impl Middleware for Mark {
            fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
                self.log.lock().unwrap().push(self.label);
                next.execute(request, connection)
            }
        }

        let app = App::new()
            .wrap(Mark { label: "A", log: Arc::clone(&log) })
            .wrap(Mark { label: "B", log: Arc::clone(&log) });

        app.execute(&get("/healthz"), &conn()).unwrap();
        assert_eq!(*log.lock().unwrap(), vec!["A", "B"]);
    }

    #[test]
    fn app_with_state_returns_correct_response() {
        struct State { msg: String }
        let app = App::with_state(State { msg: "from state".to_string() })
            .get("/hello", |_, _, _, state| ok_text(&state.msg));

        let resp = app.execute(&get("/hello"), &conn()).unwrap();
        assert_eq!(200, resp.status_code);
        let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
        assert_eq!("from state", body);
    }

    #[test]
    fn app_with_state_falls_through_to_builtin_app() {
        let app = App::with_state(()).get("/custom", |_, _, _, _| ok_text("custom"));
        let resp = app.execute(&get("/healthz"), &conn()).unwrap();
        assert_eq!(200, resp.status_code);
    }

    #[test]
    fn app_with_state_then_wrap_composes() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        struct Flag(Arc<AtomicBool>);
        impl Middleware for Flag {
            fn handle(&self, req: &Request, conn: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
                self.0.store(true, Ordering::Relaxed);
                next.execute(req, conn)
            }
        }

        let ran = Arc::new(AtomicBool::new(false));
        let app = App::with_state(())
            .get("/ping", |_, _, _, _| ok_text("pong"))
            .wrap(Flag(Arc::clone(&ran)));

        let resp = app.execute(&get("/ping"), &conn()).unwrap();
        assert_eq!(200, resp.status_code);
        assert!(ran.load(Ordering::Relaxed));
    }

    #[test]
    fn rate_limit_layer_allows_first_request() {
        let app = App::new().wrap(RateLimitLayer);
        let resp = app.execute(&get("/healthz"), &conn()).unwrap();
        assert_ne!(429, resp.status_code);
    }

    // issue #22: a rejected request carries Retry-After naming the window.
    // RateLimitLayer reads the process-wide limiter, so the test drives it
    // through set_limits with an IP of its own.
    #[test]
    fn rate_limit_429_carries_retry_after_within_the_window() {
        // The process-wide limiter is shared with every test that goes
        // through RateLimitLayer: hold the lock and restore its limits.
        let _g = crate::test_env::lock();
        let saved = crate::rate_limit::global().limits();
        struct Restore((u32, u64));
        impl Drop for Restore {
            fn drop(&mut self) {
                crate::rate_limit::global().set_limits(self.0 .0, self.0 .1);
            }
        }
        let _restore = Restore(saved);
        let ip = "203.0.113.77";
        crate::rate_limit::global().reset(ip);
        crate::rate_limit::global().set_limits(1, 30);
        crate::rate_limit::global().check(ip); // burn the one allowed request
        let app = App::new().wrap(RateLimitLayer);
        let conn = ConnectionInfo {
            client: Address { ip: ip.to_string(), port: 0 },
            server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
            request_size: 16000,
            sni_hostname: None,
        };
        let resp = app.execute(&get("/healthz"), &conn).unwrap();
        assert_eq!(429, resp.status_code);
        let header = resp
            .headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case("Retry-After"))
            .unwrap_or_else(|| panic!("no Retry-After header in {:?}", resp.headers));
        let seconds: u64 = header.value.parse().expect("Retry-After must be an integer");
        assert!(seconds > 0, "Retry-After must be positive");
        assert!(seconds <= 30, "Retry-After must not exceed the window");
    }
}

#[test]
fn builtin_controllers_match_path_without_query_where_they_always_did() {
    use crate::app::controller::file::initiate::FileUploadInitiateController;
    use crate::app::controller::form::get_method::FormGetMethodController;
    use crate::app::controller::directory_listing::DirectoryListingAssetsController;
    use crate::controller::Controller;
    let conn = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 16000,
        sni_hostname: None,
    };
    let request = |method: &str, uri: &str| Request {
        method: method.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: Default::default(),
    };

    // These two compare the parsed path: a query or fragment doesn't matter.
    assert!(FormGetMethodController::is_matching(&request(METHOD.get, "/form-get-method?a=1"), &conn));
    assert!(FormGetMethodController::is_matching(&request(METHOD.get, "/form-get-method#x"), &conn));
    assert!(!FormGetMethodController::is_matching(&request(METHOD.post, "/form-get-method"), &conn));
    assert!(!FormGetMethodController::is_matching(&request(METHOD.get, "/form-get-method/"), &conn));
    assert!(!FormGetMethodController::is_matching(&request(METHOD.get, "*"), &conn));
    assert!(FileUploadInitiateController::is_matching(&request(METHOD.post, "/file-upload/initiate?x"), &conn));
    assert!(!FileUploadInitiateController::is_matching(&request(METHOD.get, "/file-upload/initiate"), &conn));
    assert!(!FileUploadInitiateController::is_matching(&request(METHOD.post, "*"), &conn));

    // The directory-listing assets compare the whole target.
    assert!(!DirectoryListingAssetsController::is_matching(&request(METHOD.get, "/rws-directory-listing.css?v=1"), &conn));
    assert!(!DirectoryListingAssetsController::is_matching(&request(METHOD.get, "rws-directory-listing.css"), &conn));
}

#[test]
fn static_files_after_builtins_fast_and_general_paths() {
    let request = |method: &str, uri: &str, headers: Vec<Header>| Request {
        method: method.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers,
        body: Default::default(),
    };
    let app = App::with_config(ServerConfig::default());
    let conn = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 16000,
        sni_hostname: None,
    };

    // Built-in routes still win.
    let health = app.execute(&request("GET", "/healthz", vec![]), &conn).unwrap();
    assert_eq!(200, health.status_code);
    assert!(health.header(Header::_ETAG).is_none());

    // A regular file: the one-pass path (ETag, body).
    let full = app.execute(&request("GET", "/static/test.json", vec![]), &conn).unwrap();
    assert_eq!(200, full.status_code);
    assert!(full.header(Header::_ETAG).is_some());
    assert_eq!(1, full.content_range_list.len());

    // Range and OPTIONS decline the fast path and still match the file.
    let partial = app.execute(&request("GET", "/static/test.json", vec![Header::new(Header::_RANGE, "bytes=0-1")]), &conn).unwrap();
    assert_eq!(206, partial.status_code);
    let options = app.execute(&request("OPTIONS", "/static/test.json", vec![]), &conn).unwrap();
    assert_eq!(204, options.status_code);

    // A directory goes to the general path; a missing file to 404.
    assert_eq!(200, app.execute(&request("GET", "/static/", vec![]), &conn).unwrap().status_code);
    assert_eq!(404, app.execute(&request("GET", "/static/missing.json", vec![]), &conn).unwrap().status_code);
}

// The static path sends the cached gzip form; it must be exactly what
// compression::apply_gzip makes of the plain response.
#[test]
fn static_gzip_matches_apply_gzip() {
    let request = |accept: Option<&str>| Request {
        method: "GET".to_string(),
        request_uri: "/static/style.css".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: accept.map(|a| vec![Header::new(Header::_ACCEPT_ENCODING, a.to_string())]).unwrap_or_default(),
        body: Default::default(),
    };
    let app = App::with_config(ServerConfig::default());
    let conn = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 16000,
        sni_hostname: None,
    };
    let plain = app.execute(&request(None), &conn).unwrap();
    if plain.content_range_list.first().map_or(0, |cr| cr.body.len()) < crate::compression::MIN_COMPRESS_SIZE {
        return; // too small to be compressed at all
    }
    let mut expected = plain;
    crate::compression::apply_gzip(&request(Some("gzip")), &mut expected);
    for _ in 0..2 {
        let mut served = app.execute(&request(Some("gzip")), &conn).unwrap();
        // what the connection writer then does — must be a no-op now
        crate::compression::apply_gzip(&request(Some("gzip")), &mut served);
        assert_eq!(expected.content_range_list[0].body, served.content_range_list[0].body);
        let names = |r: &crate::response::Response| r.headers.iter().filter(|h| h.name != Header::_DATE_UNIX_EPOCH_NANOS && h.name != Header::_DATE).map(|h| format!("{}: {}", h.name, h.value)).collect::<Vec<_>>();
        let (mut a, mut b) = (names(&expected), names(&served));
        a.sort(); b.sort();
        assert_eq!(a, b);
    }
    let refused = app.execute(&request(Some("gzip;q=0")), &conn).unwrap();
    assert!(refused.header(Header::_CONTENT_ENCODING).is_none());
}

// 18.1.0: a browser's Accept-Encoding gets brotli for a static file (its
// smaller output wins once it is cached), with its own ETag and a 304 for it.
#[test]
fn static_files_negotiate_br_and_zstd_with_their_own_etags() {
    let request = |headers: Vec<Header>| Request {
        method: "GET".to_string(),
        request_uri: "/static/style.css".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers,
        body: Default::default(),
    };
    let accept = |a: &str| Header::new(Header::_ACCEPT_ENCODING, a.to_string());
    let app = App::with_config(ServerConfig::default());
    let conn = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 16000,
        sni_hostname: None,
    };
    let plain = app.execute(&request(vec![]), &conn).unwrap();
    let plain_body = plain.content_range_list.first().map(|cr| cr.body.clone()).unwrap_or_default();
    if plain_body.len() < crate::compression::MIN_COMPRESS_SIZE {
        return;
    }
    let plain_etag = plain.header(Header::_ETAG).unwrap().value.to_string();
    for (accepted, encoding) in [
        ("gzip, deflate, br, zstd", crate::compression::Encoding::Brotli),
        ("zstd, gzip;q=0.5", crate::compression::Encoding::Zstd),
        ("gzip", crate::compression::Encoding::Gzip),
    ] {
        if !encoding.available() {
            continue;
        }
        let served = app.execute(&request(vec![accept(accepted)]), &conn).unwrap();
        assert_eq!(Some(encoding.token()), served.header(Header::_CONTENT_ENCODING).map(|h| &*h.value), "{accepted}");
        assert_eq!(
            crate::compression::encode(encoding, &plain_body, crate::compression::Level::Static).unwrap(),
            served.content_range_list[0].body,
            "{accepted}"
        );
        let etag = served.header(Header::_ETAG).unwrap().value.to_string();
        assert_eq!(format!("{}-{}\"", plain_etag.trim_end_matches('"'), encoding.token()), etag);
        // Revalidating with that ETag is a 304; with the plain one it isn't.
        let again = app.execute(&request(vec![accept(accepted), Header::new(Header::_IF_NONE_MATCH, etag.clone())]), &conn).unwrap();
        assert_eq!(304, again.status_code, "{accepted}");
        let other = app.execute(&request(vec![accept(accepted), Header::new(Header::_IF_NONE_MATCH, plain_etag.clone())]), &conn).unwrap();
        assert_eq!(200, other.status_code, "{accepted}");
    }
}

#[test]
fn client_hints_are_opt_in() {
    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/healthz".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: Default::default(),
    };
    let conn = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 16000,
        sni_hostname: None,
    };
    let on = App::with_config(ServerConfig { client_hints: true, ..ServerConfig::default() })
        .execute(&request, &conn).unwrap();
    assert_eq!(ClientHint::get_client_hint_list(), on.header(ClientHint::ACCEPT_CLIENT_HINTS).unwrap().value);
    assert_eq!(ClientHint::get_client_hint_list(), on.header(ClientHint::CRITICAL_CLIENT_HINTS).unwrap().value);
    assert_eq!(
        "Origin, Sec-CH-UA-Arch, Sec-CH-UA-Bitness, Sec-CH-UA-Full-Version-List, Sec-CH-UA-Model, Sec-CH-UA-Platform-Version, Save-Data, Device-Memory, Upgrade-Insecure-Requests, Sec-CH-Prefers-Reduced-Motion, Sec-CH-Prefers-Color-Scheme",
        on.header(Header::_VARY).unwrap().value
    );

    let off = App::with_config(ServerConfig::default()).execute(&request, &conn).unwrap();
    assert!(off.header(ClientHint::ACCEPT_CLIENT_HINTS).is_none());
    assert!(off.header(ClientHint::CRITICAL_CLIENT_HINTS).is_none());
    assert_eq!("Origin", off.header(Header::_VARY).unwrap().value);
    // RFC 9110 §6.6.1: an origin server with a clock sends Date.
    assert!(crate::http::date::parse(&off.header(Header::_DATE).unwrap().value).is_some());
}
