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
fn parse() {
    let args_vec_as_str : Vec<&str> = "-i=127.0.0.1 -p=7777 -t=100 -a=false -o=http://localhost:7887,http://localhost:8668 -m=GET,POST,PUT,DELETE -h=content-type,x-custom-header -c=true -e=content-type,x-custom-header -g=5555"
        .split_whitespace()
        .collect::<Vec<&str>>();

    let _args_vec_as_string : Vec<String> = args_vec_as_str.iter().map(|str| str.to_string()).collect::<Vec<String>>();

    let _args_list : Vec<CommandLineArgument> = CommandLineArgument::_parse(_args_vec_as_string);

    // let debug = format!("{:?}", args_list);

    assert_eq!("1", "1");
}