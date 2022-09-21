pub struct Constants {
    pub(crate) new_line_separator: &'static str,
    pub(crate) new_line: &'static str,
    pub(crate) empty_string: &'static str,
    pub(crate) whitespace: &'static str,
    pub(crate) equals: &'static str,
    pub(crate) comma: &'static str,
    pub(crate) hyphen: &'static str,
    pub(crate) slash: &'static str,
    pub(crate) charset: &'static str,
    pub(crate) utf_8: &'static str,
    pub(crate) semicolon: &'static str,
}

pub const CONSTANTS: Constants = Constants {
    new_line: "\n",
    new_line_separator: "\r\n",
    empty_string: "",
    whitespace: " ",
    equals: "=",
    comma: ",",
    hyphen: "-",
    slash: "/",
    charset: "charset",
    utf_8: "UTF-8",
    semicolon: ";",
};