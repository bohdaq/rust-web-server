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
    pub const RWS_CONFIG_PORT: &'static str = "RWS_CONFIG_PORT";
    pub const RWS_CONFIG_THREAD_COUNT: &'static str = "RWS_CONFIG_THREAD_COUNT";

    pub const RWS_CONFIG_CORS_ALLOW_ALL: &'static str = "RWS_CONFIG_CORS_ALLOW_ALL";
    pub const RWS_CONFIG_CORS_ALLOW_ORIGINS: &'static str = "RWS_CONFIG_CORS_ALLOW_ORIGINS";
    pub const RWS_CONFIG_CORS_ALLOW_CREDENTIALS: &'static str = "RWS_CONFIG_CORS_ALLOW_CREDENTIALS";
    pub const RWS_CONFIG_CORS_ALLOW_HEADERS: &'static str = "RWS_CONFIG_CORS_ALLOW_HEADERS";
    pub const RWS_CONFIG_CORS_ALLOW_METHODS: &'static str = "RWS_CONFIG_CORS_ALLOW_METHODS";
    pub const RWS_CONFIG_CORS_EXPOSE_HEADERS: &'static str = "RWS_CONFIG_CORS_EXPOSE_HEADERS";
    pub const RWS_CONFIG_CORS_MAX_AGE: &'static str = "RWS_CONFIG_CORS_MAX_AGE";

    pub const RWS_DEFAULT_IP: &'static str = "127.0.0.1";
    pub const RWS_DEFAULT_PORT: &'static i32 = &7878;
    pub const RWS_DEFAULT_THREAD_COUNT: &'static i32 = &200;

}

pub fn bootstrap() {
    read_system_environment_variables();
    override_environment_variables_from_config(None);
    override_environment_variables_from_command_line_args();
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
        port = boxed_port.unwrap().parse().unwrap()
    }

    let boxed_thread_count = env::var(Config::RWS_CONFIG_THREAD_COUNT);
    if boxed_thread_count.is_ok() {
        thread_count = boxed_thread_count.unwrap().parse().unwrap()
    }

    (ip, port, thread_count)
}

