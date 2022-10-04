use std::env;
use crate::entry_point::Config;

pub fn read_system_environment_variables() {
    println!("  Start Of System Environment Variables Section");

    let boxed_ip = env::var(Config::RWS_CONFIG_IP);
    if boxed_ip.is_ok() {
        println!("    Set env variable '{}' to value '{}' environment variable",
                 Config::RWS_CONFIG_IP,
                 boxed_ip.unwrap());
    }

    let boxed_port = env::var(Config::RWS_CONFIG_PORT);
    if boxed_port.is_ok() {
        println!("    Set env variable '{}' to value '{}' environment variable",
                 Config::RWS_CONFIG_PORT,
                 boxed_port.unwrap());
    }

    let boxed_thread_count = env::var(Config::RWS_CONFIG_THREAD_COUNT);
    if boxed_thread_count.is_ok() {
        println!("    Set env variable '{}' to value '{}' environment variable",
                 Config::RWS_CONFIG_THREAD_COUNT,
                 boxed_thread_count.unwrap());
    }

    let boxed_cors_allow_all = env::var(Config::RWS_CONFIG_CORS_ALLOW_ALL);
    if boxed_cors_allow_all.is_ok() {
        println!("    Set env variable '{}' to value '{}' environment variable",
                 Config::RWS_CONFIG_CORS_ALLOW_ALL,
                 boxed_cors_allow_all.unwrap());
    }

    let boxed_cors_allow_origins = env::var(Config::RWS_CONFIG_CORS_ALLOW_ORIGINS);
    if boxed_cors_allow_origins.is_ok() {
        println!("    Set env variable '{}' to value '{}' environment variable",
                 Config::RWS_CONFIG_CORS_ALLOW_ORIGINS,
                 boxed_cors_allow_origins.unwrap());
    }

    let boxed_cors_allow_methods = env::var(Config::RWS_CONFIG_CORS_ALLOW_METHODS);
    if boxed_cors_allow_methods.is_ok() {
        println!("    Set env variable '{}' to value '{}' environment variable",
                 Config::RWS_CONFIG_CORS_ALLOW_METHODS,
                 boxed_cors_allow_methods.unwrap());
    }

    let boxed_cors_allow_headers = env::var(Config::RWS_CONFIG_CORS_ALLOW_HEADERS);
    if boxed_cors_allow_headers.is_ok() {
        println!("    Set env variable '{}' to value '{}' environment variable",
                 Config::RWS_CONFIG_CORS_ALLOW_HEADERS,
                 boxed_cors_allow_headers.unwrap());
    }

    let boxed_cors_allow_credentials = env::var(Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS);
    if boxed_cors_allow_credentials.is_ok() {
        println!("    Set env variable '{}' to value '{}' environment variable",
                 Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS,
                 boxed_cors_allow_credentials.unwrap());
    }

    let boxed_cors_expose_headers = env::var(Config::RWS_CONFIG_CORS_EXPOSE_HEADERS);
    if boxed_cors_expose_headers.is_ok() {
        println!("    Set env variable '{}' to value '{}' environment variable",
                 Config::RWS_CONFIG_CORS_EXPOSE_HEADERS,
                 boxed_cors_expose_headers.unwrap());
    }

    let boxed_cors_max_age = env::var(Config::RWS_CONFIG_CORS_MAX_AGE);
    if boxed_cors_max_age.is_ok() {
        println!("    Set env variable '{}' to value '{}' environment variable",
                 Config::RWS_CONFIG_CORS_MAX_AGE,
                 boxed_cors_max_age.unwrap());
    }

    println!("  End of System Environment Variables Section");
}