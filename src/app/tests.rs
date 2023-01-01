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
        headers: vec![]
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
        headers: vec![]
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
        headers: vec![]
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
        ]
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
        ]
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

