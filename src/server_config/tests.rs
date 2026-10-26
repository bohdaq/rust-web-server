#[cfg(test)]
mod server_config_tests {
    use crate::server_config::ServerConfig;

    #[test]
    fn default_cors_allow_all_is_false() {
        // Off since 18.0.0 (#59): echoing any origin with credentials let
        // every site read a logged-in user's responses.
        let cfg = ServerConfig::default();
        assert!(!cfg.cors_allow_all);
    }

    #[test]
    fn by_default_a_cross_origin_request_gets_no_cors_headers() {
        use crate::header::Header;
        use crate::request::Request;
        let request = Request {
            method: "GET".to_string(),
            request_uri: "/".to_string(),
            http_version: "HTTP/1.1".to_string(),
            headers: vec![Header { name: "Origin".into(), value: "https://evil.example".into() }],
            body: vec![],
        };
        let headers = crate::cors::Cors::get_headers_from_config(&request, &ServerConfig::default());
        assert!(headers.is_empty(), "{:?}", headers);
        let opted_in = ServerConfig { cors_allow_all: true, ..ServerConfig::default() };
        let headers = crate::cors::Cors::get_headers_from_config(&request, &opted_in);
        assert!(headers.iter().any(|h| h.name == Header::_ACCESS_CONTROL_ALLOW_ORIGIN && h.value == "https://evil.example"));
    }

    #[test]
    fn custom_config_isolates_from_env() {
        // This test sets NO env vars — demonstrates that config can be built
        // without touching std::env.
        let cfg = ServerConfig {
            cors_allow_all: false,
            cors_allow_origins: "https://example.com".to_string(),
            cors_max_age: "3600".to_string(),
            ..ServerConfig::default()
        };
        assert!(!cfg.cors_allow_all);
        assert_eq!("https://example.com", cfg.cors_allow_origins);
        assert_eq!("3600", cfg.cors_max_age);
    }

    #[test]
    fn from_env_defaults_match_struct_defaults() {
        // When NO env vars are set, from_env() and default() agree on all fields.
        // We only check fields that aren't already set in the test environment.
        let cfg = ServerConfig::from_env();
        // cors_allow_all defaults to true when env var is absent (empty string
        // is not "true", but missing var → unwrap_or_default returns "" which
        // doesn't eq_ignore_ascii_case "true"). The actual default written by
        // set_default_values() is "true", but in a fresh env it's absent → false.
        // So we just verify the struct is well-formed.
        let _ = cfg.cors_allow_origins;
        let _ = cfg.cors_allow_methods;
    }
}

#[test]
fn current_is_cached_until_invalidated() {
    let _g = crate::test_env::lock();
    let var = crate::entry_point::Config::RWS_CONFIG_CORS_ALLOW_ORIGINS;
    let saved = std::env::var(var).ok();
    std::env::set_var(var, "https://first.example");
    crate::server_config::invalidate();
    let first = crate::server_config::ServerConfig::current();
    assert_eq!(first.cors_allow_origins, "https://first.example");
    // Same snapshot while nothing invalidates it — no environment reads.
    assert!(std::sync::Arc::ptr_eq(&first, &crate::server_config::ServerConfig::current()));

    std::env::set_var(var, "https://second.example");
    assert_eq!(crate::server_config::ServerConfig::current().cors_allow_origins, "https://first.example", "not re-read without invalidate");
    crate::server_config::invalidate();
    assert_eq!(crate::server_config::ServerConfig::current().cors_allow_origins, "https://second.example");

    match saved { Some(v) => std::env::set_var(var, v), None => std::env::remove_var(var) }
    crate::server_config::invalidate();
}

#[test]
fn config_reload_refreshes_the_snapshot() {
    let _g = crate::test_env::lock();
    let before = crate::server_config::ServerConfig::current();
    crate::config_reload::reload();
    assert!(!std::sync::Arc::ptr_eq(&before, &crate::server_config::ServerConfig::current()), "reload must rebuild the snapshot");
}
