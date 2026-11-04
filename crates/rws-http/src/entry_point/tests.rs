use crate::entry_point::{bootstrap, get_ip_port_thread_count, get_max_body_size, Config};

#[test]
fn base(){
    let _g = crate::test_env::lock();
    bootstrap();
    get_ip_port_thread_count();
}

#[test]
fn get_max_body_size_defaults_to_unlimited() {
    let _g = crate::test_env::lock();
    std::env::remove_var(Config::RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES);
    assert_eq!(0, get_max_body_size());
}

#[test]
fn get_max_body_size_reads_env_var() {
    let _g = crate::test_env::lock();
    std::env::set_var(Config::RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES, "1048576");
    assert_eq!(1048576, get_max_body_size());
    std::env::remove_var(Config::RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES);
}

#[test]
fn get_max_body_size_falls_back_to_unlimited_on_unparseable_value() {
    let _g = crate::test_env::lock();
    std::env::set_var(Config::RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES, "not-a-number");
    assert_eq!(0, get_max_body_size());
    std::env::remove_var(Config::RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES);
}
#[test]
fn parse_spa_fallback_routes_sorts_longest_first_and_skips_bad_entries() {
    let routes = crate::entry_point::parse_spa_fallback_routes(
        " /cloud = cloud/index.html, /cloud/admin/=admin.html, nope, relative=x.html, /empty=, /=root.html",
    );
    assert_eq!(
        routes,
        vec![
            ("/cloud/admin".to_string(), "admin.html".to_string()),
            ("/cloud".to_string(), "cloud/index.html".to_string()),
            ("/".to_string(), "root.html".to_string()),
        ]
    );
    assert!(crate::entry_point::parse_spa_fallback_routes("").is_empty());
}
