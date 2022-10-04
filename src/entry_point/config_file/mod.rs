use std::{env, io};
use std::io::{BufRead, Cursor};
use crate::entry_point::Config;
use crate::ext::file_ext::FileExt;
use crate::symbol::SYMBOL;

pub fn read_config_file(
    cursor: Cursor<&[u8]>,
    mut prefix: String) -> Result<bool, String> {

    let lines = cursor.lines().into_iter();
    for boxed_line in lines {
        let line = boxed_line.unwrap();
        let without_comment = strip_comment(line);
        let without_whitespaces = strip_whitespaces(without_comment.to_string());
        let is_table = without_whitespaces.starts_with(SYMBOL.opening_square_bracket);
        if is_table {
            prefix = without_whitespaces
                .replace(SYMBOL.opening_square_bracket, SYMBOL.empty_string)
                .replace(SYMBOL.closing_square_bracket, SYMBOL.empty_string)
                .to_string();
        }

        let boxed_split = without_whitespaces.split_once(SYMBOL.equals);
        if boxed_split.is_none() { // empty line as an example
            prefix = "".to_string();
            continue;
        }

        let arg: String;
        let (unparsed_key, unparsed_value) = boxed_split.unwrap();
        let value = unparsed_value
            .replace(SYMBOL.quotation_mark, SYMBOL.empty_string)
            .replace(SYMBOL.closing_square_bracket, SYMBOL.empty_string)
            .replace(SYMBOL.opening_square_bracket, SYMBOL.empty_string);
        let key = unparsed_key
            .replace(SYMBOL.underscore, SYMBOL.hyphen);


        if prefix.chars().count() == 0 {
            arg = [
                SYMBOL.hyphen,
                SYMBOL.hyphen,
                &key,
                SYMBOL.equals,
                &value
            ].join("");
        } else {
            arg = [
                SYMBOL.hyphen,
                SYMBOL.hyphen,
                &prefix.to_string(),
                unparsed_key,
                SYMBOL.equals,
                &value].join("");
        }

        println!("{}", arg );

    }

    Ok(true)
}

fn strip_comment(line: String) -> String {
    let boxed_split = line.split_once(SYMBOL.number_sign);
    if boxed_split.is_none() {
        return line;
    }

    let (without_comment, _) = boxed_split.unwrap();

    without_comment.trim().to_string()
}

fn strip_whitespaces(line: String) -> String {
    let without_whitespaces = line.replace(SYMBOL.whitespace, SYMBOL.empty_string);

    without_whitespaces
}

pub fn override_environment_variables_from_config(filepath: Option<&str>) {
    println!("\n  Start of Config Section");

    let config: Config;

    let path: &str;
    if filepath.is_none() {
        path = "/rws.config.toml";
    } else {
        path = filepath.unwrap();
    }
    let static_filepath = FileExt::get_static_filepath(path);
    let boxed_content = std::fs::read_to_string(static_filepath);

    if boxed_content.is_err() {
        eprintln!("    Unable to parse rws.config.toml\n{}", boxed_content.err().unwrap());
        println!("  End of Config Section");
        return;
    } else {
        let content = boxed_content.unwrap();
        config = toml::from_str(content.as_str()).unwrap();
        let cursor = io::Cursor::new(content.as_bytes());
        let _ = read_config_file(cursor, SYMBOL.empty_string.to_string());
    }

    env::set_var(Config::RWS_CONFIG_IP, config.ip.to_string());
    println!("    Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_IP, config.ip.to_string());

    env::set_var(Config::RWS_CONFIG_PORT, config.port.to_string());
    println!("    Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_PORT, config.port.to_string());

    env::set_var(Config::RWS_CONFIG_THREAD_COUNT, config.thread_count.to_string());
    println!("    Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_THREAD_COUNT, config.thread_count.to_string());

    env::set_var(Config::RWS_CONFIG_CORS_ALLOW_ALL, config.cors.allow_all.to_string());
    println!("    Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_ALLOW_ALL, config.cors.allow_all.to_string());

    env::set_var(Config::RWS_CONFIG_CORS_ALLOW_ORIGINS, config.cors.allow_origins.join(","));
    println!("    Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_ALLOW_ORIGINS, config.cors.allow_origins.join(","));

    env::set_var(Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS, config.cors.allow_credentials.to_string());
    println!("    Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS, config.cors.allow_credentials.to_string());

    env::set_var(Config::RWS_CONFIG_CORS_ALLOW_HEADERS, config.cors.allow_headers.join(","));
    println!("    Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_ALLOW_HEADERS, config.cors.allow_headers.join(",").to_lowercase());

    env::set_var(Config::RWS_CONFIG_CORS_ALLOW_METHODS, config.cors.allow_methods.join(","));
    println!("    Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_ALLOW_METHODS, config.cors.allow_methods.join(","));

    env::set_var(Config::RWS_CONFIG_CORS_EXPOSE_HEADERS, config.cors.expose_headers.join(","));
    println!("    Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_EXPOSE_HEADERS, config.cors.expose_headers.join(",").to_lowercase());

    env::set_var(Config::RWS_CONFIG_CORS_MAX_AGE, &config.cors.max_age);
    println!("    Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_MAX_AGE, config.cors.max_age);

    println!("  End of Config Section");
}