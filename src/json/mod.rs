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
    pub string: Option<String>,
    pub boolean: Option<bool>,
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
        string: None,
        boolean: None,
        null: None,
    };

    Ok((property, value))
}

