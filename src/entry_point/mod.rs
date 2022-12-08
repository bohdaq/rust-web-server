#[cfg(test)]
mod tests;
pub mod command_line_args;
pub mod config_file;
pub mod environment_variables;


use std::{env};

use crate::entry_point::command_line_args::{override_environment_variables_from_command_line_args};
use crate::entry_point::config_file::override_environment_variables_from_config;
use crate::entry_point::environment_variables::read_system_environment_variables;

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Config {}

impl Config {
    pub const RWS_CONFIG_IP: &'static str = "RWS_CONFIG_IP";
    pub const RWS_CONFIG_IP_DEFAULT_VALUE: &'static str = "127.0.0.1";

    pub const RWS_CONFIG_PORT: &'static str = "RWS_CONFIG_PORT";
    pub const RWS_CONFIG_PORT_DEFAULT_VALUE: &'static str = "7887";

    pub const RWS_CONFIG_THREAD_COUNT: &'static str = "RWS_CONFIG_THREAD_COUNT";
    pub const RWS_CONFIG_THREAD_COUNT_DEFAULT_VALUE: &'static str = "200";

    pub const RWS_CONFIG_CORS_ALLOW_ALL: &'static str = "RWS_CONFIG_CORS_ALLOW_ALL";
    pub const RWS_CONFIG_CORS_ALLOW_ALL_DEFAULT_VALUE: &'static str = "true";

    pub const RWS_CONFIG_CORS_ALLOW_ORIGINS: &'static str = "RWS_CONFIG_CORS_ALLOW_ORIGINS";
    pub const RWS_CONFIG_CORS_ALLOW_ORIGINS_DEFAULT_VALUE: &'static str = "";

    pub const RWS_CONFIG_CORS_ALLOW_CREDENTIALS: &'static str = "RWS_CONFIG_CORS_ALLOW_CREDENTIALS";
    pub const RWS_CONFIG_CORS_ALLOW_CREDENTIALS_DEFAULT_VALUE: &'static str = "";

    pub const RWS_CONFIG_CORS_ALLOW_HEADERS: &'static str = "RWS_CONFIG_CORS_ALLOW_HEADERS";
    pub const RWS_CONFIG_CORS_ALLOW_HEADERS_DEFAULT_VALUE: &'static str = "";

    pub const RWS_CONFIG_CORS_ALLOW_METHODS: &'static str = "RWS_CONFIG_CORS_ALLOW_METHODS";
    pub const RWS_CONFIG_CORS_ALLOW_METHODS_DEFAULT_VALUE: &'static str = "";

    pub const RWS_CONFIG_CORS_EXPOSE_HEADERS: &'static str = "RWS_CONFIG_CORS_EXPOSE_HEADERS";
    pub const RWS_CONFIG_CORS_EXPOSE_HEADERS_DEFAULT_VALUE: &'static str = "";

    pub const RWS_CONFIG_CORS_MAX_AGE: &'static str = "RWS_CONFIG_CORS_MAX_AGE";
    pub const RWS_CONFIG_CORS_MAX_AGE_DEFAULT_VALUE: &'static str = "86400";

    pub const RWS_DEFAULT_IP: &'static str = "127.0.0.1";
    pub const RWS_DEFAULT_PORT: &'static i32 = &7878;
    pub const RWS_DEFAULT_THREAD_COUNT: &'static i32 = &200;

}

pub fn bootstrap() {
    read_system_environment_variables();
    override_environment_variables_from_config(None);
    override_environment_variables_from_command_line_args();
}

pub fn set_default_values() {
    println!("  Initializing default values");

    let is_var_set = env::var(Config::RWS_CONFIG_IP).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_IP, Config::RWS_CONFIG_IP_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_IP, Config::RWS_CONFIG_IP_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_IP);
    }


    let is_var_set = env::var(Config::RWS_CONFIG_PORT).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_PORT, Config::RWS_CONFIG_PORT_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_PORT, Config::RWS_CONFIG_PORT_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_PORT);
    }


    let is_var_set = env::var(Config::RWS_CONFIG_THREAD_COUNT).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_THREAD_COUNT, Config::RWS_CONFIG_THREAD_COUNT_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_THREAD_COUNT, Config::RWS_CONFIG_THREAD_COUNT_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_THREAD_COUNT);
    }

    let is_var_set = env::var(Config::RWS_CONFIG_CORS_ALLOW_ALL).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_CORS_ALLOW_ALL, Config::RWS_CONFIG_CORS_ALLOW_ALL_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_CORS_ALLOW_ALL, Config::RWS_CONFIG_CORS_ALLOW_ALL_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_CORS_ALLOW_ALL);
    }


    let is_var_set = env::var(Config::RWS_CONFIG_CORS_ALLOW_ORIGINS).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_CORS_ALLOW_ORIGINS, Config::RWS_CONFIG_CORS_ALLOW_ORIGINS_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_CORS_ALLOW_ORIGINS, Config::RWS_CONFIG_CORS_ALLOW_ORIGINS_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_CORS_ALLOW_ORIGINS);
    }

    let is_var_set = env::var(Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS, Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS, Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS);
    }

    let is_var_set = env::var(Config::RWS_CONFIG_CORS_ALLOW_HEADERS).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_CORS_ALLOW_HEADERS, Config::RWS_CONFIG_CORS_ALLOW_HEADERS_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_CORS_ALLOW_HEADERS, Config::RWS_CONFIG_CORS_ALLOW_HEADERS_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_CORS_ALLOW_HEADERS);
    }


    let is_var_set = env::var(Config::RWS_CONFIG_CORS_ALLOW_METHODS).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_CORS_ALLOW_METHODS, Config::RWS_CONFIG_CORS_ALLOW_METHODS_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_CORS_ALLOW_METHODS, Config::RWS_CONFIG_CORS_ALLOW_METHODS_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_CORS_ALLOW_METHODS);
    }

    let is_var_set = env::var(Config::RWS_CONFIG_CORS_EXPOSE_HEADERS).is_ok();
    if !is_var_set {
        env::set_var(Config::RWS_CONFIG_CORS_EXPOSE_HEADERS, Config::RWS_CONFIG_CORS_EXPOSE_HEADERS_DEFAULT_VALUE);
        println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_CORS_EXPOSE_HEADERS, Config::RWS_CONFIG_CORS_EXPOSE_HEADERS_DEFAULT_VALUE);
    } else {
        println!("    There is an environment variable  for '{}', default value won't be set", Config::RWS_CONFIG_CORS_EXPOSE_HEADERS);
    }

    env::set_var(Config::RWS_CONFIG_CORS_MAX_AGE, Config::RWS_CONFIG_CORS_MAX_AGE_DEFAULT_VALUE);
    println!("    Default value  for '{}' is '{}'", Config::RWS_CONFIG_CORS_MAX_AGE, Config::RWS_CONFIG_CORS_MAX_AGE_DEFAULT_VALUE);


    println!("  End of initializing default values\n");
}


pub fn get_ip_port_thread_count() -> (String, i32, i32) {
    let mut ip : String = Config::RWS_DEFAULT_IP.to_string();
    let mut port: i32 = *Config::RWS_DEFAULT_PORT;
    let mut thread_count: i32 = *Config::RWS_DEFAULT_THREAD_COUNT;

    let boxed_ip = env::var(Config::RWS_CONFIG_IP);
    if boxed_ip.is_ok() {
        ip = boxed_ip.unwrap()
    }

    let boxed_port = env::var(Config::RWS_CONFIG_PORT);
    if boxed_port.is_ok() {
        let _port = boxed_port.unwrap();
        let boxed_parse = _port.parse::<i32>();
        if boxed_parse.is_ok() {
            port = boxed_parse.unwrap();
        } else {
            eprintln!("unable to parse port value, expected number, got {}, variable: {}",
                      _port, Config::RWS_CONFIG_PORT);
        }
    } else {
        eprintln!("unable to parse port value, variable: {}", Config::RWS_CONFIG_PORT);
    }

    let boxed_thread_count = env::var(Config::RWS_CONFIG_THREAD_COUNT);
    if boxed_thread_count.is_ok() {
        let _thread_count = boxed_thread_count.unwrap();
        let boxed_parse = _thread_count.parse();
        if boxed_parse.is_ok() {
            thread_count = boxed_parse.unwrap()
        } else {
            eprintln!("unable to parse thread count value, expected number, got {}, variable: {}",
                      thread_count, Config::RWS_CONFIG_THREAD_COUNT);
        }

    } else {
        eprintln!("unable to parse thread count value, variable: {}", Config::RWS_CONFIG_THREAD_COUNT);
    }

    (ip, port, thread_count)
}

