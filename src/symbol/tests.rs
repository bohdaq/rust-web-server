use crate::symbol::SYMBOL;

#[test]
fn symbol_check() {
    assert_eq!(SYMBOL.new_line, "\n");
    assert_eq!(SYMBOL.carriage_return, "\r");
    assert_eq!(SYMBOL.new_line_carriage_return, "\r\n");
    assert_eq!(SYMBOL.empty_string, "");
    assert_eq!(SYMBOL.whitespace, " ");
    assert_eq!(SYMBOL.equals, "=");
    assert_eq!(SYMBOL.comma, ",");
    assert_eq!(SYMBOL.hyphen, "-");
    assert_eq!(SYMBOL.slash, "/");
    assert_eq!(SYMBOL.semicolon, ";");
    assert_eq!(SYMBOL.colon, ":");
}