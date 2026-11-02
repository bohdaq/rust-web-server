use crate::symbol::SYMBOL;

pub struct StringExt;

impl StringExt {
    pub fn truncate_new_line_carriage_return(str: &str) -> String {
        str.replace("\r", "").replace("\n", "")
    }

    pub fn filter_ascii_control_characters(str: &str) -> String {
        str.replace(|x : char | x.is_ascii_control(), SYMBOL.empty_string).trim().to_string()
    }

    pub fn float_number_with_precision(number: f64, number_of_digits: u8) -> String {
        let formatted = format!("{0:.1$}", number, number_of_digits as usize);
        formatted.to_string()
    }
}