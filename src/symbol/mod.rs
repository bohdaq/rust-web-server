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
    pub number_sign: &'static str,
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
    number_sign: "#",
};