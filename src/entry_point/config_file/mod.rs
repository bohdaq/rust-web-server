use std::{io};
use std::io::{BufRead, Cursor};
use crate::entry_point::command_line_args::{CommandLineArgument};
use file_ext::FileExt;
use crate::symbol::SYMBOL;

pub fn read_config_file(
    cursor: Cursor<&[u8]>,
    mut prefix: String) -> Result<bool, String> {

    let mut argument_list : Vec<String> = vec![];
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
            continue;
        }

        let arg: String;
        let (unparsed_key, unparsed_value) = boxed_split.unwrap();
        let value = unparsed_value
            .replace(SYMBOL.single_quote, SYMBOL.empty_string)
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
                SYMBOL.hyphen,
                &key,
                SYMBOL.equals,
                &value].join("");
        }

        argument_list.push(arg);
    }
    let params = CommandLineArgument::get_command_line_arg_list();
    CommandLineArgument::_parse(argument_list, params);

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

    let path: &str;
    if filepath.is_none() {
        path = "/rws.config.toml";
    } else {
        path = filepath.unwrap();
    }

    let boxed_static_filepath = FileExt::get_static_filepath(path);
    if boxed_static_filepath.is_err() {
        eprintln!("{}", boxed_static_filepath.err().unwrap());
        return;
    }

    let static_filepath = boxed_static_filepath.unwrap();
    let boxed_content = std::fs::read_to_string(static_filepath);

    if boxed_content.is_err() {
        eprintln!("    Unable to parse rws.config.toml: {}", boxed_content.err().unwrap());
        println!("  End of Config Section");
        return;
    } else {
        let content = boxed_content.unwrap();
        let cursor = io::Cursor::new(content.as_bytes());
        let _ = read_config_file(cursor, SYMBOL.empty_string.to_string());
    }

    println!("  End of Config Section");
}