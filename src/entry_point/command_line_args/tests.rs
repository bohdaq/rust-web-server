use std::env;
use crate::entry_point::command_line_args::{CommandLineArgument, CommandLineArgumentValue};
use crate::entry_point::Config;

#[test]
fn command_line_arg_list() {
    let command_line_arg_list = CommandLineArgument::get_command_line_arg_list();

    let mut argument = command_line_arg_list.get(0).unwrap();
    let hint = argument.hint.as_ref().unwrap();
    assert_eq!(argument.short_form, "p");
    assert_eq!(argument.long_form, "port");
    assert_eq!(argument.environment_variable, Config::RWS_CONFIG_PORT.to_string());
    assert_eq!(hint, "Port");

    let env_var = env::var(&argument.environment_variable).unwrap();
    assert_eq!(env_var, "7887");

    CommandLineArgument::set_environment_variable(argument, "8888".to_string());

    let env_var = env::var(&argument.environment_variable).unwrap();
    assert_eq!(env_var, "8888");
}

#[test]
fn parse() {
    let args_vec_as_str : Vec<&str> = "-i=127.0.0.1 -p=7777 -t=100 -a=false -o=http://localhost:7887,http://localhost:8668 -m=GET,POST,PUT,DELETE -h=content-type,x-custom-header -c=true -e=content-type,x-custom-header -g=5555"
        .split_whitespace()
        .collect::<Vec<&str>>();

    let args_vec_as_string : Vec<String> = args_vec_as_str.iter().map(|str| str.to_string()).collect::<Vec<String>>();

    let args_list : Vec<CommandLineArgument> = CommandLineArgument::parse(args_vec_as_string);

    // let debug = format!("{:?}", args_list);

    assert_eq!("1", "1");
}