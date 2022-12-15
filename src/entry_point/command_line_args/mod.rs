use std::env;
use crate::entry_point::Config;

#[cfg(test)]
mod tests;

pub fn override_environment_variables_from_command_line_args() {
    println!("\n  Start of Reading Command Line Arguments Section");

    let args = env::args().collect();
    let params = CommandLineArgument::get_command_line_arg_list();
    CommandLineArgument::_parse(args, params);

    println!("  End of Reading Command Line Arguments\n");
}

pub struct CommandLineArgument {
    pub short_form: String,
    pub long_form: String,
    pub environment_variable: String,
    pub _hint: Option<String>,
}

impl CommandLineArgument {
    pub fn get_command_line_arg_list() -> Vec<CommandLineArgument> {
        let mut argument_list : Vec<CommandLineArgument> = vec![];

        let argument = CommandLineArgument {
            short_form: "p".to_string(),
            long_form: "port".to_string(),
            environment_variable: Config::RWS_CONFIG_PORT.to_string(),
            _hint: Some("Port".to_string())
        };
        argument_list.push(argument);

        let argument = CommandLineArgument {
            short_form: "i".to_string(),
            long_form: "ip".to_string(),
            environment_variable: Config::RWS_CONFIG_IP.to_string(),
            _hint: Some("IP or domain".to_string())
        };
        argument_list.push(argument);

        let argument = CommandLineArgument {
            short_form: "t".to_string(),
            long_form: "thread-count".to_string(),
            environment_variable: Config::RWS_CONFIG_THREAD_COUNT.to_string(),
            _hint: Some("Number of threads".to_string())
        };
        argument_list.push(argument);


        let argument = CommandLineArgument {
            short_form: "a".to_string(),
            long_form: "cors-allow-all".to_string(),
            environment_variable: Config::RWS_CONFIG_CORS_ALLOW_ALL.to_string(),
            _hint: Some("If set to true, will allow all CORS requests, other CORS properties will be ignored".to_string())
        };
        argument_list.push(argument);

        let argument = CommandLineArgument {
            short_form: "o".to_string(),
            long_form: "cors-allow-origins".to_string(),
            environment_variable: Config::RWS_CONFIG_CORS_ALLOW_ORIGINS.to_string(),
            _hint: Some("Comma separated list of allowed origins, example: https://foo.example,https://bar.example".to_string())
        };
        argument_list.push(argument);

        let argument = CommandLineArgument {
            short_form: "m".to_string(),
            long_form: "cors-allow-methods".to_string(),
            environment_variable: Config::RWS_CONFIG_CORS_ALLOW_METHODS.to_string(),
            _hint: Some("Comma separated list of allowed methods, example: POST,PUT".to_string())
        };
        argument_list.push(argument);

        let argument = CommandLineArgument {
            short_form: "h".to_string(),
            long_form: "cors-allow-headers".to_string(),
            environment_variable: Config::RWS_CONFIG_CORS_ALLOW_HEADERS.to_string(),
            _hint: Some("Comma separated list of allowed request headers, in lowercase, example: content-type,x-custom-header".to_string())
        };
        argument_list.push(argument);

        let argument = CommandLineArgument {
            short_form: "c".to_string(),
            long_form: "cors-allow-credentials".to_string(),
            environment_variable: Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS.to_string(),
            _hint: Some("If set to true, will allow to transmit credentials via CORS requests".to_string())
        };
        argument_list.push(argument);

        let argument = CommandLineArgument {
            short_form: "e".to_string(),
            long_form: "cors-expose-headers".to_string(),
            environment_variable: Config::RWS_CONFIG_CORS_EXPOSE_HEADERS.to_string(),
            _hint: Some("Comma separated list of allowed response headers, in lowercase, example: content-type,x-custom-header".to_string())
        };
        argument_list.push(argument);

        let argument = CommandLineArgument {
            short_form: "g".to_string(),
            long_form: "cors-max-age".to_string(),
            environment_variable: Config::RWS_CONFIG_CORS_MAX_AGE.to_string(),
            _hint: Some("How long results of preflight requests can be cached (in seconds)".to_string())
        };
        argument_list.push(argument);

        let argument = CommandLineArgument {
            short_form: "r".to_string(),
            long_form: "request-allocation-size-in-bytes".to_string(),
            environment_variable: Config::RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES.to_string(),
            _hint: Some("In bytes, how much memory to allocate for each request".to_string())
        };
        argument_list.push(argument);

        argument_list
    }

    pub fn _parse(args: Vec<String>, argument_list : Vec<CommandLineArgument>) -> Vec<CommandLineArgument> {
        for unparsed_argument in args.iter() {

            let boxed_split = unparsed_argument.split_once('=');
            if boxed_split.is_some() {

                let (parameter, value) = boxed_split.unwrap();
                println!("\n    {}={}", parameter, value);
                let boxed_predefined_argument =
                    argument_list
                        .iter()
                        .find(
                            |predefined_argument| {
                                let short_form = predefined_argument.short_form.to_string();
                                let long_form = predefined_argument.long_form.to_string();
                                let _param_short_form = ["-", &short_form].join("");
                                let _param_long_form = ["--", &long_form].join("");

                                parameter.eq(_param_short_form.as_str()) ||
                                    parameter.eq(_param_long_form.as_str())
                });
                if boxed_predefined_argument.is_some() {
                    let predefined_argument = boxed_predefined_argument.unwrap();
                    CommandLineArgument::set_environment_variable(predefined_argument, value.to_string());
                }

            }
        }
        argument_list
    }

    pub fn set_environment_variable(argument: &CommandLineArgument, value: String) {
        env::set_var(&argument.environment_variable, &value);
        println!("    Set env variable '{}' to value '{}'", argument.environment_variable, &value);
    }
}
