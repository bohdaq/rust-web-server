use crate::app::App;
use crate::client_hint::ClientHint;
use crate::cors::Cors;
use crate::entry_point::config_file::override_environment_variables_from_config;
use crate::header::Header;
use crate::request::{METHOD, Request};
use crate::http::VERSION;
use crate::response::STATUS_CODE_REASON_PHRASE;

#[test]
fn not_found() {
    override_environment_variables_from_config(Some("/src/test/app/rws.config.toml"));

    let request = Request {
        method: METHOD.post.to_string(),
        request_uri: "/some/path".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let (response, _request) = App::handle_request(request);
    for header in response.headers {
        println!("{:?}", header);
    }
    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n404_not_found.status_code);
}

#[test]
fn static_file() {
    override_environment_variables_from_config(Some("/src/test/app/rws.config.toml"));

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/static/content.png".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let (response, _request) = App::handle_request(request);
    for header in response.headers {
        println!("{:?}", header);
    }
    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n200_ok.status_code);
}

#[test]
fn index() {
    override_environment_variables_from_config(Some("/src/test/app/rws.config.toml"));

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let (response, _request) = App::handle_request(request);
    for header in response.headers {
        println!("{:?}", header);
    }
    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n200_ok.status_code);
}

#[test]
fn static_file_cors_options_preflight_request_client_hints() {
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
                name: Header::_ORIGIN.to_string(),
                value: origin_value.to_string()
            },
            Header {
                name: Header::_ACCESS_CONTROL_REQUEST_METHOD.to_string(),
                value: METHOD.post.to_string()
            },
            Header {
                name: Header::_ACCESS_CONTROL_REQUEST_HEADERS.to_string(),
                value: expected_allow_headers
            },
        ],
        body: vec![],
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

    let vary_header = response._get_header(Header::_VARY.to_string()).unwrap();
    assert_eq!(
        vary_header.value,
        "Origin, Sec-CH-UA-Arch, Sec-CH-UA-Bitness, Sec-CH-UA-Full-Version-List, Sec-CH-UA-Model, Sec-CH-UA-Platform-Version, Save-Data, Device-Memory, Upgrade-Insecure-Requests, Sec-CH-Prefers-Reduced-Motion, Sec-CH-Prefers-Color-Scheme"
    );

    let client_hints = response._get_header(ClientHint::ACCEPT_CLIENT_HINTS.to_string()).unwrap();
    assert_eq!(
        client_hints.value,
        ClientHint::get_client_hint_list()
    );

    let x_frame_options = response._get_header(Header::_X_FRAME_OPTIONS.to_string()).unwrap();
    assert_eq!(Header::_X_FRAME_OPTIONS_VALUE_SAME_ORIGIN, x_frame_options.value);

    for header in &response.headers {
        println!("{:?}", header);
    }
    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n204_no_content.status_code);
}

#[test]
fn static_file_cors_off_options_preflight_request_client_hints() {
    override_environment_variables_from_config(Some("/src/test/app/rws.config_cors_off.toml"));

    let origin_value = "origin-value.com";
    let custom_header = "X-CUSTOM-HEADER";

    let expected_allow_headers = format!("{},{}", Header::_CONTENT_TYPE, custom_header);


    let request = Request {
        method: METHOD.options.to_string(),
        request_uri: "/static/content.png".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![
            Header {
                name: Header::_ORIGIN.to_string(),
                value: origin_value.to_string()
            },
            Header {
                name: Header::_ACCESS_CONTROL_REQUEST_METHOD.to_string(),
                value: METHOD.post.to_string()
            },
            Header {
                name: Header::_ACCESS_CONTROL_REQUEST_HEADERS.to_string(),
                value: expected_allow_headers
            },
        ],
        body: vec![],
    };

    let (response, _request) = App::handle_request(request);


    let vary_header = response._get_header(Header::_VARY.to_string()).unwrap();
    assert_eq!(
        vary_header.value,
        "Origin, Sec-CH-UA-Arch, Sec-CH-UA-Bitness, Sec-CH-UA-Full-Version-List, Sec-CH-UA-Model, Sec-CH-UA-Platform-Version, Save-Data, Device-Memory, Upgrade-Insecure-Requests, Sec-CH-Prefers-Reduced-Motion, Sec-CH-Prefers-Color-Scheme"
    );

    let client_hints = response._get_header(ClientHint::ACCEPT_CLIENT_HINTS.to_string()).unwrap();
    assert_eq!(
        client_hints.value,
        ClientHint::get_client_hint_list()
    );

    let x_frame_options = response._get_header(Header::_X_FRAME_OPTIONS.to_string()).unwrap();
    assert_eq!(Header::_X_FRAME_OPTIONS_VALUE_SAME_ORIGIN, x_frame_options.value);

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
            body: vec![],
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
                resp.headers.push(crate::header::Header { name: "X-Test".to_string(), value: "yes".to_string() });
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
}
