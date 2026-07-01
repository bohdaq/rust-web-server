use super::{load_optional, load_required, load_with_default, FromEnvStr};

// ── #[derive(Config)] integration ────────────────────────────────────────────

#[cfg(feature = "macros")]
mod derive_config_tests {
    // Each test uses its own isolated env var names to avoid parallel-test races.

    #[derive(crate::Config)]
    struct FullConfig {
        #[config(env = "RWSTEST_DC_A_PORT", default = "9000")]
        port: u16,
        #[config(env = "RWSTEST_DC_A_NAME")]
        name: String,
        #[config(env = "RWSTEST_DC_A_VERBOSE")]
        verbose: Option<bool>,
    }

    #[test]
    fn loads_with_env_vars_set() {
        std::env::set_var("RWSTEST_DC_A_PORT", "1234");
        std::env::set_var("RWSTEST_DC_A_NAME", "test_app");
        std::env::set_var("RWSTEST_DC_A_VERBOSE", "true");

        let cfg = FullConfig::load().unwrap();
        assert_eq!(1234, cfg.port);
        assert_eq!("test_app", cfg.name);
        assert_eq!(Some(true), cfg.verbose);

        std::env::remove_var("RWSTEST_DC_A_PORT");
        std::env::remove_var("RWSTEST_DC_A_NAME");
        std::env::remove_var("RWSTEST_DC_A_VERBOSE");
    }

    #[derive(crate::Config)]
    struct DefaultConfig {
        #[config(env = "RWSTEST_DC_B_PORT", default = "9000")]
        port: u16,
        #[config(env = "RWSTEST_DC_B_NAME")]
        name: String,
        #[config(env = "RWSTEST_DC_B_VERBOSE")]
        verbose: Option<bool>,
    }

    #[test]
    fn uses_default_when_port_absent() {
        std::env::remove_var("RWSTEST_DC_B_PORT");
        std::env::set_var("RWSTEST_DC_B_NAME", "default_test");
        std::env::remove_var("RWSTEST_DC_B_VERBOSE");

        let cfg = DefaultConfig::load().unwrap();
        assert_eq!(9000, cfg.port);
        assert_eq!(None, cfg.verbose);

        std::env::remove_var("RWSTEST_DC_B_NAME");
    }

    #[derive(crate::Config)]
    struct RequiredConfig {
        #[config(env = "RWSTEST_DC_C_NAME")]
        name: String,
        #[config(env = "RWSTEST_DC_C_PORT", default = "8080")]
        port: u16,
    }

    #[test]
    fn required_field_absent_returns_err() {
        std::env::remove_var("RWSTEST_DC_C_NAME");
        std::env::remove_var("RWSTEST_DC_C_PORT");

        let result = RequiredConfig::load();
        assert!(result.is_err());
        assert!(result.err().unwrap().contains("RWSTEST_DC_C_NAME"));
    }

    #[derive(crate::Config)]
    #[config(prefix = "RWSTEST_DC_D_")]
    struct PrefixConfig {
        #[config(env = "FOO", default = "42")]
        foo: u32,
    }

    #[test]
    fn prefix_on_struct() {
        std::env::remove_var("RWSTEST_DC_D_FOO");
        let cfg = PrefixConfig::load().unwrap();
        assert_eq!(42, cfg.foo);

        std::env::set_var("RWSTEST_DC_D_FOO", "99");
        let cfg2 = PrefixConfig::load().unwrap();
        assert_eq!(99, cfg2.foo);
        std::env::remove_var("RWSTEST_DC_D_FOO");
    }
}

// ── FromEnvStr impls ──────────────────────────────────────────────────────────

#[test]
fn string_passthrough() {
    assert_eq!("hello", String::from_env_str("hello").unwrap());
    assert_eq!("", String::from_env_str("").unwrap());
}

#[test]
fn bool_true_variants() {
    for s in &["true", "1", "yes"] {
        assert!(bool::from_env_str(s).unwrap(), "expected true for {:?}", s);
    }
}

#[test]
fn bool_false_variants() {
    for s in &["false", "0", "no"] {
        assert!(!bool::from_env_str(s).unwrap(), "expected false for {:?}", s);
    }
}

#[test]
fn bool_invalid_returns_err() {
    assert!(bool::from_env_str("maybe").is_err());
}

#[test]
fn u32_parse() {
    assert_eq!(42u32, u32::from_env_str("42").unwrap());
    assert_eq!(0u32, u32::from_env_str("0").unwrap());
}

#[test]
fn u32_invalid_returns_err() {
    assert!(u32::from_env_str("abc").is_err());
    assert!(u32::from_env_str("-1").is_err());
}

#[test]
fn i64_parse_negative() {
    assert_eq!(-99i64, i64::from_env_str("-99").unwrap());
}

#[test]
fn f64_parse() {
    let v = f64::from_env_str("3.14").unwrap();
    assert!((v - 3.14).abs() < 1e-9);
}

#[test]
fn usize_parse() {
    assert_eq!(1000usize, usize::from_env_str("1000").unwrap());
}

#[test]
fn parse_trims_whitespace() {
    assert_eq!(7u16, u16::from_env_str("  7  ").unwrap());
    assert!(bool::from_env_str("  true  ").unwrap());
}

// ── load_required ─────────────────────────────────────────────────────────────

#[test]
fn load_required_present() {
    std::env::set_var("_RWS_TEST_REQUIRED_KEY", "42");
    let v: u32 = load_required("_RWS_TEST_REQUIRED_KEY").unwrap();
    assert_eq!(42, v);
    std::env::remove_var("_RWS_TEST_REQUIRED_KEY");
}

#[test]
fn load_required_absent_returns_err() {
    std::env::remove_var("_RWS_TEST_ABSENT_KEY");
    let result: Result<String, _> = load_required("_RWS_TEST_ABSENT_KEY");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("_RWS_TEST_ABSENT_KEY"));
}

#[test]
fn load_required_bad_parse_returns_err() {
    std::env::set_var("_RWS_TEST_BAD_PARSE", "not_a_number");
    let result: Result<u32, _> = load_required("_RWS_TEST_BAD_PARSE");
    assert!(result.is_err());
    std::env::remove_var("_RWS_TEST_BAD_PARSE");
}

// ── load_with_default ─────────────────────────────────────────────────────────

#[test]
fn load_with_default_uses_env_when_set() {
    std::env::set_var("_RWS_TEST_DEFAULT_KEY", "99");
    let v: u32 = load_with_default("_RWS_TEST_DEFAULT_KEY", "0").unwrap();
    assert_eq!(99, v);
    std::env::remove_var("_RWS_TEST_DEFAULT_KEY");
}

#[test]
fn load_with_default_uses_default_when_absent() {
    std::env::remove_var("_RWS_TEST_DEFAULT_ABSENT");
    let v: u32 = load_with_default("_RWS_TEST_DEFAULT_ABSENT", "7").unwrap();
    assert_eq!(7, v);
}

#[test]
fn load_with_default_bad_parse_returns_err() {
    std::env::set_var("_RWS_TEST_DEFAULT_BAD", "oops");
    let result: Result<u16, _> = load_with_default("_RWS_TEST_DEFAULT_BAD", "0");
    assert!(result.is_err());
    std::env::remove_var("_RWS_TEST_DEFAULT_BAD");
}

// ── load_optional ─────────────────────────────────────────────────────────────

#[test]
fn load_optional_present_parses() {
    std::env::set_var("_RWS_TEST_OPT_KEY", "true");
    let v: Option<bool> = load_optional("_RWS_TEST_OPT_KEY").unwrap();
    assert_eq!(Some(true), v);
    std::env::remove_var("_RWS_TEST_OPT_KEY");
}

#[test]
fn load_optional_absent_is_none() {
    std::env::remove_var("_RWS_TEST_OPT_ABSENT");
    let v: Option<String> = load_optional("_RWS_TEST_OPT_ABSENT").unwrap();
    assert_eq!(None, v);
}

#[test]
fn load_optional_empty_string_is_none() {
    std::env::set_var("_RWS_TEST_OPT_EMPTY", "");
    let v: Option<u32> = load_optional("_RWS_TEST_OPT_EMPTY").unwrap();
    assert_eq!(None, v);
    std::env::remove_var("_RWS_TEST_OPT_EMPTY");
}

#[test]
fn load_optional_whitespace_only_is_none() {
    std::env::set_var("_RWS_TEST_OPT_WS", "   ");
    let v: Option<u32> = load_optional("_RWS_TEST_OPT_WS").unwrap();
    assert_eq!(None, v);
    std::env::remove_var("_RWS_TEST_OPT_WS");
}

#[test]
fn load_optional_bad_parse_returns_err() {
    std::env::set_var("_RWS_TEST_OPT_BAD", "not_a_bool");
    let result: Result<Option<bool>, _> = load_optional("_RWS_TEST_OPT_BAD");
    assert!(result.is_err());
    std::env::remove_var("_RWS_TEST_OPT_BAD");
}
