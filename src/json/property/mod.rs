use crate::json::{JSON_TYPE};
use std::fmt::{Display, Formatter};
use crate::core::New;
use crate::null::Null;
use crate::symbol::SYMBOL;

#[cfg(test)]
mod tests;

pub struct JSONProperty {
    pub property_name: String,
    pub property_type: String,
}

pub struct JSONValue {
    pub f64: Option<f64>,
    pub i128: Option<i128>,
    pub string: Option<String>,
    pub object: Option<String>,
    pub array: Option<String>,
    pub bool: Option<bool>,
    pub null: Option<Null>,
}

impl New for JSONValue {
    fn new() -> JSONValue {
        JSONValue {
            f64: None,
            i128: None,
            string: None,
            object: None,
            array: None,
            bool: None,
            null: None,
        }
    }
}

impl JSONValue {
    pub fn float_number_with_precision(&self, number_of_digits: u8) -> String {
        let number = self.f64.as_ref().unwrap();
        let formatted = format!("{0:.1$}", number, number_of_digits as usize);
        formatted.to_string()
    }
}

impl Display for JSONValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.f64.is_some() {
            let f64 = self.f64.unwrap();
            let formatted : String = format!("{:.13}", f64);
            return f.write_str(formatted.as_str());
        }

        if self.i128.is_some() {
            let formatted = self.i128.unwrap().to_string();
            return f.write_str(formatted.as_str());
        }

        if self.string.is_some() {
            let formatted = self.string.as_ref().unwrap();
            return f.write_str(formatted.as_str());
        }

        if self.array.is_some() {
            let formatted = self.array.as_ref().unwrap();
            return f.write_str(formatted.as_str());
        }

        if self.null.is_some() {
            return f.write_str("null");
        }

        if self.object.is_some() {
            let formatted = self.object.as_ref().unwrap();
            return f.write_str(formatted.as_str());
        }

        if self.bool.is_some() {
            let formatted = self.bool.as_ref().unwrap();
            return f.write_str(formatted.to_string().as_str());
        }

        f.write_str("Something Went Wrong. There is no value for any type.")

    }
}

impl JSONProperty {
    pub fn parse(raw_string: &str) -> Result<(JSONProperty, JSONValue), String> {
        let mut property = JSONProperty { property_name: "".to_string(), property_type: "".to_string() };
        let mut value = JSONValue {
            f64: None,
            i128: None,
            string: None,
            object: None,
            array: None,
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
        let is_boolean = (_value == "true") || (_value == "false");
        let is_number = !is_string && !is_null && !is_array && !is_object && !is_boolean;

        if !is_null && !is_string && !is_array && !is_object && !is_number && !is_boolean {
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
            value.string = Some(_value.replace(SYMBOL.quotation_mark, SYMBOL.empty_string).to_string());
        }

        if is_number {
            let boxed_i128_parse = _value.parse::<i128>();
            if boxed_i128_parse.is_err() {
                let boxed_f64_parse = _value.parse::<f64>();
                if boxed_f64_parse.is_err() {
                    let message = format!("unable to parse number: {}: {}", _key, _value);
                    return Err(message);
                } else {
                    property.property_type = JSON_TYPE.number.to_string();
                    property.property_name = _key.replace(SYMBOL.quotation_mark, SYMBOL.empty_string).to_string();
                    let f64 = boxed_f64_parse.unwrap();
                    value.f64 = Some(f64);
                }
            } else {
                property.property_type = JSON_TYPE.integer.to_string();
                property.property_name = _key.replace(SYMBOL.quotation_mark, SYMBOL.empty_string).to_string();
                let i128 = boxed_i128_parse.unwrap();
                value.i128 = Some(i128);
            }
        }

        if is_array {
            property.property_type = JSON_TYPE.array.to_string();
            property.property_name = _key.replace(SYMBOL.quotation_mark, SYMBOL.empty_string).to_string();
            value.array = Some(_value.to_string());
        }

        if is_object {
            property.property_type = JSON_TYPE.object.to_string();
            property.property_name = _key.replace(SYMBOL.quotation_mark, SYMBOL.empty_string).to_string();
            value.object = Some(_value.to_string());
        }

        if is_boolean {
            let is_true = _value == "true";
            property.property_type = JSON_TYPE.boolean.to_string();
            property.property_name = _key.replace(SYMBOL.quotation_mark, SYMBOL.empty_string).to_string();
            value.bool = Some(is_true);
        }

        Ok((property, value))
    }
}



