use std::env;
use crate::entry_point::command_line_args::{CommandLineArgument};
use crate::entry_point::Config;

#[test]
fn command_line_arg_port() {
    let command_line_arg_list = CommandLineArgument::get_command_line_arg_list();

    let argument = command_line_arg_list.get(0).unwrap();
    let hint = argument.hint.as_ref().unwrap();
    assert_eq!(argument.short_form, "p");
    assert_eq!(argument.long_form, "port");
    assert_eq!(argument.environment_variable, Config::RWS_CONFIG_PORT.to_string());
    assert_eq!(hint, "Port");

    CommandLineArgument::set_environment_variable(argument, "8888".to_string());

    let env_var = env::var(&argument.environment_variable).unwrap();
    assert_eq!(env_var, "8888");
}

#[test]
fn command_line_arg_ip() {
    let command_line_arg_list = CommandLineArgument::get_command_line_arg_list();

    let argument = command_line_arg_list.get(1).unwrap();
    let hint = argument.hint.as_ref().unwrap();
    assert_eq!(argument.short_form, "i");
    assert_eq!(argument.long_form, "ip");
    assert_eq!(argument.environment_variable, Config::RWS_CONFIG_IP.to_string());
    assert_eq!(hint, "IP or domain");

    CommandLineArgument::set_environment_variable(argument, "localhost".to_string());

    let env_var = env::var(&argument.environment_variable).unwrap();
    assert_eq!(env_var, "localhost");
}

#[test]
fn command_line_arg_thread_count() {
    let command_line_arg_list = CommandLineArgument::get_command_line_arg_list();

    let argument = command_line_arg_list.get(2).unwrap();
    let hint = argument.hint.as_ref().unwrap();
    assert_eq!(argument.short_form, "t");
    assert_eq!(argument.long_form, "threads");
    assert_eq!(argument.environment_variable, Config::RWS_CONFIG_THREAD_COUNT.to_string());
    assert_eq!(hint, "Number of threads");

    CommandLineArgument::set_environment_variable(argument, "200".to_string());

    let env_var = env::var(&argument.environment_variable).unwrap();
    assert_eq!(env_var, "200");
}

#[test]
fn command_line_arg_thread_cors_allow_all() {
    let command_line_arg_list = CommandLineArgument::get_command_line_arg_list();

    let argument = command_line_arg_list.get(3).unwrap();
    let hint = argument.hint.as_ref().unwrap();
    assert_eq!(argument.short_form, "a");
    assert_eq!(argument.long_form, "cors-allow-all");
    assert_eq!(argument.environment_variable, Config::RWS_CONFIG_CORS_ALLOW_ALL.to_string());
    assert_eq!(hint, "If set to true, will allow all CORS requests, other CORS properties will be ignored");

    CommandLineArgument::set_environment_variable(argument, "true".to_string());

    let env_var = env::var(&argument.environment_variable).unwrap();
    assert_eq!(env_var, "true");
}

#[test]
fn command_line_arg_thread_cors_allow_origins() {
    let command_line_arg_list = CommandLineArgument::get_command_line_arg_list();

    let argument = command_line_arg_list.get(4).unwrap();
    let hint = argument.hint.as_ref().unwrap();
    assert_eq!(argument.short_form, "o");
    assert_eq!(argument.long_form, "cors-allow-origins");
    assert_eq!(argument.environment_variable, Config::RWS_CONFIG_CORS_ALLOW_ORIGINS.to_string());
    assert_eq!(hint, "Comma separated list of allowed origins, example: https://foo.example,https://bar.example");

    CommandLineArgument::set_environment_variable(argument, "https://foo.example,https://bar.example".to_string());

    let env_var = env::var(&argument.environment_variable).unwrap();
    assert_eq!(env_var, "https://foo.example,https://bar.example");
}

#[test]
fn command_line_arg_thread_cors_allow_methods() {
    let command_line_arg_list = CommandLineArgument::get_command_line_arg_list();

    let argument = command_line_arg_list.get(5).unwrap();
    let hint = argument.hint.as_ref().unwrap();
    assert_eq!(argument.short_form, "m");
    assert_eq!(argument.long_form, "cors-allow_methods");
    assert_eq!(argument.environment_variable, Config::RWS_CONFIG_CORS_ALLOW_METHODS.to_string());
    assert_eq!(hint, "Comma separated list of allowed methods, example: POST,PUT");

    CommandLineArgument::set_environment_variable(argument, "POST,PUT".to_string());

    let env_var = env::var(&argument.environment_variable).unwrap();
    assert_eq!(env_var, "POST,PUT");
}

#[test]
fn command_line_arg_thread_cors_allow_headers() {
    let command_line_arg_list = CommandLineArgument::get_command_line_arg_list();

    let argument = command_line_arg_list.get(6).unwrap();
    let hint = argument.hint.as_ref().unwrap();
    assert_eq!(argument.short_form, "h");
    assert_eq!(argument.long_form, "cors-allow-headers");
    assert_eq!(argument.environment_variable, Config::RWS_CONFIG_CORS_ALLOW_HEADERS.to_string());
    assert_eq!(hint, "Comma separated list of allowed request headers, in lowercase, example: content-type,x-custom-header");

    CommandLineArgument::set_environment_variable(argument, "content-type,x-custom-header".to_string());

    let env_var = env::var(&argument.environment_variable).unwrap();
    assert_eq!(env_var, "content-type,x-custom-header");
}

#[test]
fn command_line_arg_thread_cors_allow_credentials() {
    let command_line_arg_list = CommandLineArgument::get_command_line_arg_list();

    let argument = command_line_arg_list.get(7).unwrap();
    let hint = argument.hint.as_ref().unwrap();
    assert_eq!(argument.short_form, "c");
    assert_eq!(argument.long_form, "cors-allow-credentials");
    assert_eq!(argument.environment_variable, Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS.to_string());
    assert_eq!(hint, "If set to true, will allow to transmit credentials via CORS requests");

    CommandLineArgument::set_environment_variable(argument, "true".to_string());

    let env_var = env::var(&argument.environment_variable).unwrap();
    assert_eq!(env_var, "true");
}

#[test]
fn command_line_arg_thread_cors_expose_headers() {
    let command_line_arg_list = CommandLineArgument::get_command_line_arg_list();

    let argument = command_line_arg_list.get(8).unwrap();
    let hint = argument.hint.as_ref().unwrap();
    assert_eq!(argument.short_form, "e");
    assert_eq!(argument.long_form, "cors-expose-headers");
    assert_eq!(argument.environment_variable, Config::RWS_CONFIG_CORS_EXPOSE_HEADERS.to_string());
    assert_eq!(hint, "Comma separated list of allowed response headers, in lowercase, example: content-type,x-custom-header");

    CommandLineArgument::set_environment_variable(argument, "content-type,x-custom-header".to_string());

    let env_var = env::var(&argument.environment_variable).unwrap();
    assert_eq!(env_var, "content-type,x-custom-header");
}

#[test]
fn command_line_arg_thread_cors_max_age() {
    let command_line_arg_list = CommandLineArgument::get_command_line_arg_list();

    let argument = command_line_arg_list.get(9).unwrap();
    let hint = argument.hint.as_ref().unwrap();
    assert_eq!(argument.short_form, "g");
    assert_eq!(argument.long_form, "cors-max-age");
    assert_eq!(argument.environment_variable, Config::RWS_CONFIG_CORS_MAX_AGE.to_string());
    assert_eq!(hint, "How long results of preflight requests can be cached (in seconds)");

    CommandLineArgument::set_environment_variable(argument, "99999".to_string());

    let env_var = env::var(&argument.environment_variable).unwrap();
    assert_eq!(env_var, "99999");
}

#[test]
fn parse() {
    let args_vec_as_str : Vec<&str> = "-i=127.0.0.1 -p=7777 -t=100 -a=false -o=http://localhost:7887,http://localhost:8668 -m=GET,POST,PUT,DELETE -h=content-type,x-custom-header -c=true -e=content-type,x-custom-header -g=5555"
        .split_whitespace()
        .collect::<Vec<&str>>();

    let _args_vec_as_string : Vec<String> = args_vec_as_str.iter().map(|str| str.to_string()).collect::<Vec<String>>();

    let _args_list : Vec<CommandLineArgument> = CommandLineArgument::_parse(_args_vec_as_string);

    let env_var = env::var(Config::RWS_CONFIG_CORS_MAX_AGE).unwrap();
    assert_eq!(env_var, "5555");

    let env_var = env::var(Config::RWS_CONFIG_CORS_EXPOSE_HEADERS).unwrap();
    assert_eq!(env_var, "content-type,x-custom-header");

    let env_var = env::var(Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS).unwrap();
    assert_eq!(env_var, "true");

    let env_var = env::var(Config::RWS_CONFIG_CORS_ALLOW_HEADERS).unwrap();
    assert_eq!(env_var, "content-type,x-custom-header");

    let env_var = env::var(Config::RWS_CONFIG_CORS_ALLOW_METHODS).unwrap();
    assert_eq!(env_var, "GET,POST,PUT,DELETE");

    let env_var = env::var(Config::RWS_CONFIG_CORS_ALLOW_ORIGINS).unwrap();
    assert_eq!(env_var, "http://localhost:7887,http://localhost:8668");

    let env_var = env::var(Config::RWS_CONFIG_CORS_ALLOW_ALL).unwrap();
    assert_eq!(env_var, "false");

    let env_var = env::var(Config::RWS_CONFIG_THREAD_COUNT).unwrap();
    assert_eq!(env_var, "100");

    let env_var = env::var(Config::RWS_CONFIG_PORT).unwrap();
    assert_eq!(env_var, "7777");

    let env_var = env::var(Config::RWS_CONFIG_IP).unwrap();
    assert_eq!(env_var, "127.0.0.1");
}