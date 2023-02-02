use crate::symbol::SYMBOL;

pub struct StringExt;

impl StringExt {
    pub fn truncate_new_line_carriage_return(str: &str) -> String {
        str.replace("\r", "").replace("\n", "")
    }

    pub fn filter_ascii_control_characters(str: &str) -> String {
        str.replace(|x : char | x.is_ascii_control(), SYMBOL.empty_string).trim().to_string()
    }
}