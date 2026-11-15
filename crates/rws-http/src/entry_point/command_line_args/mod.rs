use std::env;
use crate::entry_point::Config;

#[cfg(test)]
mod tests;

pub fn override_environment_variables_from_command_line_args() {
    crate::entry_point::note("\n  Start of Reading Command Line Arguments Section");

    let args = joined_value_forms(&env::args().collect::<Vec<_>>());
    let params = CommandLineArgument::get_command_line_arg_list();
    CommandLineArgument::_parse(args, params);

    crate::entry_point::note("  End of Reading Command Line Arguments\n");
}

/// `-p 8080` / `--port 8080` written as `-p=8080` / `--port=8080`, for every
/// flag in [`CommandLineArgument::get_command_line_arg_list`] (since
/// 18.37.0). A flag is joined with the next argument only when that one
/// doesn't start with `-`. `-h` stays as it is: alone it asks for help.
pub fn joined_value_forms(args: &[String]) -> Vec<String> {
    let flags = CommandLineArgument::get_command_line_arg_list();
    let mut out = Vec::with_capacity(args.len());
    let mut iter = args.iter().peekable();
    if let Some(program) = iter.next() {
        out.push(program.clone());
    }
    while let Some(arg) = iter.next() {
        let flag = flags.iter().find(|f| {
            (!f.short_form.is_empty() && f.short_form != "h" && *arg == format!("-{}", f.short_form))
                || *arg == format!("--{}", f.long_form)
        });
        match (flag, iter.peek()) {
            (Some(flag), Some(next)) if !next.starts_with('-') => {
                out.push(format!("--{}={}", flag.long_form, next));
                iter.next();
            }
            _ => out.push(arg.clone()),
        }
    }
    out
}

/// Arguments the `rws` binary takes besides the settings flags.
const BINARY_ARGS: &[&str] = &["--check", "--format=json", "--verbose", "--version", "-V", "--help", "-h", "help", "--manifests"];
const BINARY_ARG_PREFIXES: &[&str] = &["--check=", "--windows-service=", "--format="];

/// Check the `rws` binary's arguments (program name first) before it starts
/// a server: every argument must be a settings flag with a value
/// (`--port=8080`, `-p 8080`) or one of the binary's own (`--check`,
/// `--verbose`, …). `Err` is the message to print, for exit code 2. Only the
/// binary calls this: an application built on the library may take
/// arguments of its own.
pub fn check_binary_args(args: &[String]) -> Result<(), String> {
    let flags = CommandLineArgument::get_command_line_arg_list();
    let joined = joined_value_forms(args);
    let mut iter = joined.iter().skip(1).peekable();
    while let Some(arg) = iter.next() {
        if BINARY_ARGS.contains(&arg.as_str()) || BINARY_ARG_PREFIXES.iter().any(|p| arg.starts_with(p)) {
            // `--check <file>`: the file is its value.
            if arg == "--check" && iter.peek().is_some_and(|n| !n.starts_with('-')) {
                iter.next();
            }
            continue;
        }
        let (name, has_value) = match arg.split_once('=') {
            Some((name, _)) => (name, true),
            None => (arg.as_str(), false),
        };
        let flag = flags.iter().find(|f| {
            (!f.short_form.is_empty() && name == format!("-{}", f.short_form)) || name == format!("--{}", f.long_form)
        });
        match flag {
            Some(_) if has_value => {}
            Some(flag) => {
                return Err(format!("rws: {arg} needs a value: --{}=<value>\nrws --help lists every option.", flag.long_form));
            }
            None => {
                let hint = closest_flag(name, &flags).map(|f| format!(" Did you mean --{f}?")).unwrap_or_default();
                return Err(format!("rws: unknown argument '{arg}'.{hint}\nrws --help lists every option."));
            }
        }
    }
    Ok(())
}

/// The flag closest to `name` (edit distance at most 2, or a prefix of one).
fn closest_flag(name: &str, flags: &[CommandLineArgument]) -> Option<String> {
    let bare = name.trim_start_matches('-');
    if bare.len() < 2 {
        return None;
    }
    let mut best: Option<(usize, &str)> = None;
    for f in flags {
        let long = f.long_form.as_str();
        let d = if long.starts_with(bare) { 1 } else { crate::entry_point::edit_distance(bare, long) };
        if d <= 2 && best.is_none_or(|(b, _)| d < b) {
            best = Some((d, long));
        }
    }
    best.map(|(_, f)| f.to_string())
}

/// Returns the text to print for an informational flag — `--version` / `-V`
/// or `--help` / `-h` / `help` — if `args` (as from `env::args()`, program
/// name first) contains one, so the binary can print it and exit before
/// starting a server. `None` means start normally. `-h` alone is help since
/// 18.37.0; `-h=<list>` is still `--cors-allow-headers`.
pub fn informational_flag_output(args: &[String]) -> Option<String> {
    let flags = args.iter().skip(1);
    for arg in flags {
        match arg.as_str() {
            "--version" | "-V" => {
                // A FIPS build says so (18.23.0); others print just the version.
                return Some(format!("rws {}{}\n", env!("CARGO_PKG_VERSION"), if cfg!(feature = "fips") { " (FIPS)" } else { "" }));
            }
            "--help" | "-h" => return Some(help_text()),
            _ => {}
        }
    }
    if args.get(1).map(String::as_str) == Some("help") && args.len() == 2 {
        return Some(help_text());
    }
    None
}

fn help_text() -> String {
    let mut text = format!(
        "rws {} — web server, reverse proxy and automatic HTTPS\n\n\
         Usage:\n\
         \x20 rws [OPTIONS]                 Serve the working directory (http://localhost:7878)\n\
         \x20 rws --check[=<file>]          Validate rws.config.toml (or <file>); exit 0 if it has no errors\n\
         \x20 rws ctl <command>             Drive a running server's admin API (rws ctl help; also rwsctl)\n\
         \x20 rws gen model|route|migration|test <name>\n\
         \x20                               Generate code into the project (rws gen --help)\n\
         \x20 rws db <command>              Run the project's migrations and seeds (rws db --help)\n\
         \x20 rws trust [--print]           Trust rws's local CA (tls = \"internal\"); --print shows it\n\
         \x20 rws untrust                   Take the local CA out of the trust store\n\n\
         An rws.config.toml with [[route]]/[[upstream]] sections in the working\n\
         directory runs rws as a config-driven reverse proxy instead.\n\n\
         Each option is also an environment variable (shown) and a key in rws.config.toml.\n\
         Values: --port=8080 or --port 8080.\n",
        env!("CARGO_PKG_VERSION")
    );
    let list = CommandLineArgument::get_command_line_arg_list();
    let common = ["port", "ip", "thread-count", "tls", "tls-cert-file", "tls-key-file"];
    let groups: [(&str, Box<dyn Fn(&str) -> bool>); 4] = [
        ("Common options", Box::new(move |l: &str| common.contains(&l))),
        ("Automatic HTTPS (ACME)", Box::new(|l: &str| l.starts_with("acme-") || l == "http-redirect-port")),
        ("CORS", Box::new(|l: &str| l.starts_with("cors-"))),
        ("Other", Box::new(move |l: &str| !common.contains(&l) && !l.starts_with("acme-") && l != "http-redirect-port" && !l.starts_with("cors-"))),
    ];
    for (title, wanted) in groups.iter() {
        let mut section = String::new();
        let mut items: Vec<&CommandLineArgument> = list.iter().filter(|a| wanted(&a.long_form)).collect();
        if *title == "Common options" {
            items.sort_by_key(|a| common.iter().position(|c| *c == a.long_form));
        }
        for argument in items {
            let flag = if argument.short_form.is_empty() {
                format!("    --{}=<value>", argument.long_form)
            } else {
                format!("-{}, --{}=<value>", argument.short_form, argument.long_form)
            };
            let hint = argument._hint.clone().unwrap_or_default();
            section.push_str(&format!("  {:<44} {}\n  {:<44} [env: {}]\n", flag, hint, "", argument.environment_variable));
        }
        if !section.is_empty() {
            text.push_str(&format!("\n{title}:\n{section}"));
        }
    }
    text.push_str("\nGeneral:\n");
    text.push_str("      --verbose                                Print every setting and step at startup\n");
    text.push_str("  -V, --version                                Print version and exit\n");
    text.push_str("  -h, --help                                   Print this help and exit\n");
    text.push_str("      --windows-service=<dir>                  (Windows) Run as a service serving <dir>\n\n");
    text.push_str("Docs: https://rws8.tech/docs/ · every setting: https://rws8.tech/docs/configuration/env-vars/\n");
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
                "Where to store the issued certificate (default: tls-cert-file, else .rws/acme/cert.pem)"),
            ("acme-key-path", Config::RWS_CONFIG_ACME_KEY_PATH,
                "Where to store the certificate's private key (default: tls-key-file, else .rws/acme/key.pem)"),
            ("acme-profile", Config::RWS_CONFIG_ACME_PROFILE,
                "Certificate profile to order, as the CA offers them (e.g. classic, tlsserver, shortlived)"),
            ("acme-challenge", Config::RWS_CONFIG_ACME_CHALLENGE,
                "How to prove control of the domains: http-01 (default), tls-alpn-01, dns-01 (wildcards) or dns-persist-01"),
            ("acme-dns-provider", Config::RWS_CONFIG_ACME_DNS_PROVIDER,
                "DNS-01: rfc2136 (DNS UPDATE), cloudflare, route53 or exec (a program)"),
            ("acme-dns-server", Config::RWS_CONFIG_ACME_DNS_SERVER,
                "DNS-01 rfc2136: the primary DNS server, host[:port]"),
            ("acme-dns-zone", Config::RWS_CONFIG_ACME_DNS_ZONE,
                "DNS-01: the zone of the records (found by SOA lookup when not set)"),
            ("acme-dns-tsig-key", Config::RWS_CONFIG_ACME_DNS_TSIG_KEY,
                "DNS-01 rfc2136: TSIG key name (the secret goes in RWS_CONFIG_ACME_DNS_TSIG_SECRET)"),
            ("acme-dns-tsig-algorithm", Config::RWS_CONFIG_ACME_DNS_TSIG_ALGORITHM,
                "DNS-01 rfc2136: hmac-sha256 (default) or hmac-sha512"),
            ("acme-dns-exec", Config::RWS_CONFIG_ACME_DNS_EXEC,
                "DNS-01 exec: program run as <program> present|cleanup <fqdn> <value>"),
            ("acme-dns-propagation-secs", Config::RWS_CONFIG_ACME_DNS_PROPAGATION_SECS,
                "DNS-01: seconds to wait for the record to be visible (default 120)"),
            ("acme-dns-api-url", Config::RWS_CONFIG_ACME_DNS_API_URL,
                "DNS-01 cloudflare/route53: API base URL instead of the provider's (the token goes in RWS_CONFIG_ACME_DNS_API_TOKEN)"),
            ("acme-dns-zone-id", Config::RWS_CONFIG_ACME_DNS_ZONE_ID,
                "DNS-01 cloudflare/route53: the zone's id, instead of looking it up"),
            ("acme-redis", Config::RWS_CONFIG_ACME_REDIS,
                "Redis host:port shared by replicas: one orders each certificate, all install it"),
            ("acme-redis-prefix", Config::RWS_CONFIG_ACME_REDIS_PREFIX,
                "Key prefix in that Redis (default rws:acme)"),
            ("acme-on-demand", Config::RWS_CONFIG_ACME_ON_DEMAND,
                "true: order a certificate at the first handshake for a name the ask endpoint allows"),
            ("acme-on-demand-ask", Config::RWS_CONFIG_ACME_ON_DEMAND_ASK,
                "On-demand TLS: URL asked ?domain=<name> before each order; 2xx allows it"),
        ];
        for (long_form, environment_variable, hint) in long_only {
            argument_list.push(CommandLineArgument {
                short_form: "".to_string(),
                long_form: long_form.to_string(),
                environment_variable: environment_variable.to_string(),
                _hint: Some(hint.to_string()),
            });
        }

        let argument = CommandLineArgument {
            short_form: "".to_string(),
            long_form: "tls".to_string(),
            environment_variable: "RWS_CONFIG_TLS".to_string(),
            _hint: Some("internal: HTTPS from rws's own CA (local development; see rws trust)".to_string())
        };
        argument_list.push(argument);

        argument_list
    }

    pub fn _parse(args: Vec<String>, argument_list : Vec<CommandLineArgument>) -> Vec<CommandLineArgument> {
        for unparsed_argument in args.iter() {

            let boxed_split = unparsed_argument.split_once('=');
            if boxed_split.is_some() {

                let (parameter, value) = boxed_split.unwrap();
                crate::entry_point::note(&format!("\n    {}={}", parameter, crate::entry_point::shown(parameter, value)));
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
        crate::server_config::invalidate();
        crate::entry_point::note(&format!("    Set env variable '{}' to value '{}'", argument.environment_variable, crate::entry_point::shown(&argument.environment_variable, &value)));
    }
}
