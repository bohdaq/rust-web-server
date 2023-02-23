use crate::json::{JSON_TYPE, JSONProperty, JSONValue, Null};
use crate::symbol::SYMBOL;

#[cfg(test)]
mod tests;

pub fn parse_json_property(raw_string: &str) -> Result<(JSONProperty, JSONValue), String> {
    let mut property = JSONProperty { property_name: "".to_string(), property_type: "".to_string() };
    let mut value = JSONValue {
        i8: None,
        u8: None,
        i16: None,
        u16: None,
        i32: None,
        u32: None,
        i64: None,
        u64: None,
        i128: None,
        u128: None,
        usize: None,
        isize: None,
        String: None,
        bool: None,
        null: None,
    };

    let boxed_split = raw_string.trim().split_once(SYMBOL.colon);
    if boxed_split.is_none() {
        let message = format!("Not a valid string as a key-value: {}", raw_string);
        return Err(message);
    }

    let (mut _key, mut _value) = boxed_split.unwrap();
    _key = _key.trim();
    _value = _value.trim();

    let is_null = _value == "null";
    let is_string = _value.starts_with(SYMBOL.quotation_mark) && _value.ends_with(SYMBOL.quotation_mark);
    let is_array = _value.starts_with(SYMBOL.opening_square_bracket) && _value.ends_with(SYMBOL.closing_square_bracket);
    let is_object = _value.starts_with(SYMBOL.opening_curly_bracket) && _value.ends_with(SYMBOL.closing_curly_bracket);
    let is_number = !is_string && !is_null && !is_array && !is_object;

    if !is_null && !is_string && !is_array && !is_object && !is_number {
        let message = format!("Is not valid key value pair: {} {}", _key, _value);
        return Err(message);
    }



    if is_null {
        property.property_type = JSON_TYPE.string.to_string();
        property.property_name = _key.replace(SYMBOL.quotation_mark, SYMBOL.empty_string).to_string();
        value.null = Some(Null{});

    }

    if is_string {
        property.property_type = JSON_TYPE.string.to_string();
        property.property_name = _key.replace(SYMBOL.quotation_mark, SYMBOL.empty_string).to_string();
        value.String = Some(_value.replace(SYMBOL.quotation_mark, SYMBOL.empty_string).to_string());
    }

    if is_number {

    }

    if is_array {

    }

    if is_object {

    }

    Ok((property, value))
}

