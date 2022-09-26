pub struct StringExt;

impl StringExt {
    pub fn truncate_new_line_carriage_return(str: &str) -> String {
        str.replace("\r", "").replace("\n", "")
    }
}