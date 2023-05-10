use crate::symbol::SYMBOL;

pub struct URL;

impl URL {
    pub fn encode_uri_component(component: &str) -> String {
        let mut _result = component.replace(SYMBOL.whitespace, "%20");
        _result = component.replace(SYMBOL.new_line, "%0A");
        _result = component.replace(SYMBOL.carriage_return, "%0D");

        return _result
    }
}