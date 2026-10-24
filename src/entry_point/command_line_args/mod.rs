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

/// Returns the text to print for an informational flag — `--version` / `-V`
/// or `--help` — if `args` (as from `env::args()`, program name first)
/// contains one, so the binary can print it and exit before starting a
/// server. `None` means start normally. `-h` is not a help alias: it's
/// already `--cors-allow-headers`'s short form.
pub fn informational_flag_output(args: &[String]) -> Option<String> {
    let flags = args.iter().skip(1);
    for arg in flags {
        match arg.as_str() {
            "--version" | "-V" => {
                return Some(format!("rws {}\n", env!("CARGO_PKG_VERSION")));
            }
            "--help" => return Some(help_text()),
            _ => {}
        }
    }
    None
}

fn help_text() -> String {
    let mut text = format!(
        "rws {} — HTTP/1.1, HTTP/2 and HTTP/3 web server and reverse proxy\n\n\
         Usage: rws [OPTIONS]\n\n\
         Serves the current directory on http://0.0.0.0:7878 by default. Drop an\n\
         rws.config.toml with [[route]]/[[upstream]] sections in the working\n\
         directory to run as a config-driven reverse proxy instead.\n\n\
         Options (each also settable via the environment variable shown):\n",
        env!("CARGO_PKG_VERSION")
    );
    for argument in CommandLineArgument::get_command_line_arg_list() {
        let flag = if argument.short_form.is_empty() {
            format!("    --{}=<value>", argument.long_form)
        } else {
            format!("-{}, --{}=<value>", argument.short_form, argument.long_form)
        };
        let hint = argument._hint.unwrap_or_default();
        text.push_str(&format!("  {:<44} {}\n  {:<44} [env: {}]\n", flag, hint, "", argument.environment_variable));
    }
    text.push_str("      --check[=<file>]                         Validate rws.config.toml (or <file>) and exit:\n");
    text.push_str("                                               0 if it has no errors, 1 otherwise\n");
    text.push_str("  -V, --version                                Print version and exit\n");
    text.push_str("      --help                                   Print this help and exit\n\n");
    text.push_str("Docs: https://rws8.tech\n");
    text
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

        let argument = CommandLineArgument {
            short_form: "s".to_string(),
            long_form: "tls-cert-file".to_string(),
            environment_variable: Config::RWS_CONFIG_TLS_CERT_FILE.to_string(),
            _hint: Some("Path to TLS certificate PEM file (enables HTTPS and HTTP/2)".to_string())
        };
        argument_list.push(argument);

        let argument = CommandLineArgument {
            short_form: "k".to_string(),
            long_form: "tls-key-file".to_string(),
            environment_variable: Config::RWS_CONFIG_TLS_KEY_FILE.to_string(),
            _hint: Some("Path to TLS private key PEM file (enables HTTPS and HTTP/2)".to_string())
        };
        argument_list.push(argument);

        // Long-form only (empty short_form): every single letter that reads
        // naturally is taken. In rws.config.toml these are `http_redirect_port`
        // at the top level and the keys of an `[acme]` section (`domains`,
        // `email`, ...), which the config-file reader prefixes to `acme-*`.
        let long_only = [
            ("http-redirect-port", Config::RWS_CONFIG_HTTP_REDIRECT_PORT,
                "Plain-HTTP port that 301-redirects to HTTPS (and answers ACME HTTP-01 challenges), e.g. 80"),
            ("acme-domains", Config::RWS_CONFIG_ACME_DOMAINS,
                "Comma separated domains to get a Let's Encrypt certificate for; enables automatic HTTPS"),
            ("acme-email", Config::RWS_CONFIG_ACME_EMAIL,
                "Contact email for the ACME account"),
            ("acme-staging", Config::RWS_CONFIG_ACME_STAGING,
                "If set to true, uses the Let's Encrypt staging environment (untrusted test certificates)"),
            ("acme-directory", Config::RWS_CONFIG_ACME_DIRECTORY,
                "ACME directory URL of a CA other than Let's Encrypt"),
            ("acme-ca-file", Config::RWS_CONFIG_ACME_CA_FILE,
                "PEM file of extra root CAs to trust for the ACME directory (private ACME CAs)"),
            ("acme-challenge-port", Config::RWS_CONFIG_ACME_CHALLENGE_PORT,
                "Port the CA connects to for HTTP-01 challenges (default 80)"),
            ("acme-cert-path", Config::RWS_CONFIG_ACME_CERT_PATH,
                "Where to store the issued certificate (default: tls-cert-file, else cert.pem)"),
            ("acme-key-path", Config::RWS_CONFIG_ACME_KEY_PATH,
                "Where to store the certificate's private key (default: tls-key-file, else key.pem)"),
        ];
        for (long_form, environment_variable, hint) in long_only {
            argument_list.push(CommandLineArgument {
                short_form: "".to_string(),
                long_form: long_form.to_string(),
                environment_variable: environment_variable.to_string(),
                _hint: Some(hint.to_string()),
            });
        }

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

                                (!short_form.is_empty() && parameter.eq(_param_short_form.as_str())) ||
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
        if value.contains('\0') {
            // env::set_var panics on NUL; reachable from rws.config.toml values.
            eprintln!("    Warning: value for --{} contains a NUL byte — ignored", argument.long_form);
            return;
        }
        env::set_var(&argument.environment_variable, &value);
        println!("    Set env variable '{}' to value '{}'", argument.environment_variable, &value);
    }
}
