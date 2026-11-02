//! Tests of rws-server's `app` that read repository fixtures (`static/`,
//! `index.html`): a crate's tests run in the crate's directory.
#![allow(unused_imports, dead_code)]

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
        tls: false,
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
        tls: false,
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
