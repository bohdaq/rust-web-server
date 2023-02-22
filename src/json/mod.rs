use crate::symbol::SYMBOL;

#[cfg(test)]
mod tests;

// TODO: wip

pub struct JSONProperty {
    pub property_name: String,
    pub property_type: String,
}

pub struct JSONValue {
    pub i8: Option<i32>,
    pub u8: Option<u32>,
    pub i16: Option<i32>,
    pub u16: Option<u32>,
    pub i32: Option<i32>,
    pub u32: Option<u32>,
    pub i64: Option<i64>,
    pub u64: Option<u64>,
    pub i128: Option<i128>,
    pub u128: Option<u128>,
    pub usize: Option<usize>,
    pub isize: Option<isize>,
    pub String: Option<String>,
    pub bool: Option<bool>,
    pub null: Option<Null>,
}

pub struct Null {}

pub trait FromAndToJSON {
    fn list_properties() -> Vec<JSONProperty>;

    fn get_property(&self, property_name: String) -> JSONValue;

    fn to_json_string(&self) -> String;

    fn from_json_string(json_string: String) -> Self;
}

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

    let boxed_split = raw_string.trim().split_once(SYMBOL.semicolon);
    if boxed_split.is_none() {
        let message = format!("Not a valid string as a key-value: {}", raw_string);
        return Err(message);
    }

    let (mut _key, mut _value) = boxed_split.unwrap();
    _key = _key.trim();
    _value = _value.trim();

    if !_key.starts_with(SYMBOL.quotation_mark) {
        let message = format!("Key is not properly defined: {}", _key);
        return Err(message);
    }

    let is_null = _value == "null";
    let is_string = _value.starts_with(SYMBOL.semicolon);
    let is_array = _value.starts_with(SYMBOL.opening_square_bracket);
    let is_object = _value.starts_with(SYMBOL.opening_curly_bracket);
    let is_number = !is_string && !is_null && !is_array && !is_object;

    if is_null {

    }

    if is_string {

    }

    if is_number {

    }

    if is_array {

    }

    if is_object {

    }

    Ok((property, value))
}

