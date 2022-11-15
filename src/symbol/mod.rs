#[cfg(test)]
mod tests;

pub struct Symbol {
    pub new_line_carriage_return: &'static str,
    pub new_line: &'static str,
    pub carriage_return: &'static str,
    pub empty_string: &'static str,
    pub whitespace: &'static str,
    pub equals: &'static str,
    pub comma: &'static str,
    pub hyphen: &'static str,
    pub slash: &'static str,
    pub semicolon: &'static str,
    pub colon: &'static str,
    pub number_sign: &'static str,
    pub opening_square_bracket: &'static str,
    pub closing_square_bracket: &'static str,
    pub quotation_mark: &'static str,
    pub underscore: &'static str,
    pub single_quote: &'static str,
}

pub const SYMBOL: Symbol = Symbol {
    new_line: "\n",
    carriage_return: "\r",
    new_line_carriage_return: "\r\n",
    empty_string: "",
    whitespace: " ",
    equals: "=",
    comma: ",",
    hyphen: "-",
    slash: "/",
    semicolon: ";",
    colon: ":",
    number_sign: "#",
    opening_square_bracket: "[",
    closing_square_bracket: "]",
    quotation_mark: "\"",
    underscore: "_",
    single_quote: "'",
};