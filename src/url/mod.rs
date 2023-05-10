#[cfg(test)]
mod tests;

use crate::symbol::SYMBOL;

pub struct URL;

impl URL {
    pub fn encode_uri_component(component: &str) -> String {
        let mut _result = component.replace(SYMBOL.percent, "%25");
        _result = _result.replace(SYMBOL.whitespace, "%20");
        _result = _result.replace(SYMBOL.carriage_return, "%0D");
        _result = _result.replace(SYMBOL.new_line, "%0A");
        _result = _result.replace(SYMBOL.exclamation_mark, "%21");
        _result = _result.replace(SYMBOL.quotation_mark, "%22");
        _result = _result.replace(SYMBOL.number_sign, "%23");
        _result = _result.replace(SYMBOL.dollar, "%24");
        _result = _result.replace(SYMBOL.ampersand, "%26");
        _result = _result.replace(SYMBOL.single_quote, "%27");
        _result = _result.replace(SYMBOL.opening_bracket, "%28");
        _result = _result.replace(SYMBOL.closing_bracket, "%29");
        _result = _result.replace(SYMBOL.asterisk, "%2A");
        _result = _result.replace(SYMBOL.plus, "%2B");
        _result = _result.replace(SYMBOL.comma, "%2C");


        return _result
    }

    pub fn decode_uri_component(component: &str) -> String {
        let mut _result = component.replace( "%20", SYMBOL.whitespace);
        _result = _result.replace("%0A", SYMBOL.new_line);
        _result = _result.replace ("%0D", SYMBOL.carriage_return);
        _result = _result.replace ("%21", SYMBOL.exclamation_mark);
        _result = _result.replace ("%22", SYMBOL.quotation_mark);
        _result = _result.replace ("%23", SYMBOL.number_sign);
        _result = _result.replace ("%24", SYMBOL.dollar);
        _result = _result.replace ("%25", SYMBOL.percent);
        _result = _result.replace ("%26", SYMBOL.ampersand);
        _result = _result.replace ("%27", SYMBOL.single_quote);
        _result = _result.replace ("%28", SYMBOL.opening_bracket);
        _result = _result.replace ("%29", SYMBOL.closing_bracket);
        _result = _result.replace ("%2A", SYMBOL.asterisk);
        _result = _result.replace ("%2B", SYMBOL.plus);
        _result = _result.replace ("%2C", SYMBOL.comma);

        return _result
    }
}