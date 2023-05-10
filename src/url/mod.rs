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
        _result = _result.replace(SYMBOL.quotation_mark, "%22");


        return _result
    }

    pub fn decode_uri_component(component: &str) -> String {
        let mut _result = component.replace( "%20", SYMBOL.whitespace);
        _result = _result.replace("%0A", SYMBOL.new_line);
        _result = _result.replace ("%0D", SYMBOL.carriage_return);
        _result = _result.replace ("%22", SYMBOL.quotation_mark);
        _result = _result.replace ("%25", SYMBOL.percent);

        return _result
    }
}