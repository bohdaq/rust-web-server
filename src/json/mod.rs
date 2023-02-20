#[cfg(test)]
mod tests;

// TODO: wip
pub struct JSONProperty {
    pub property_name: String,
    pub property_type: String,
}

pub struct JSONValue {
    pub i32: Option<i32>,
    pub u32: Option<u32>,
    pub i64: Option<i64>,
    pub u64: Option<u64>,
    pub i128: Option<i128>,
    pub u128: Option<u128>,
    pub usize: Option<usize>,
    pub vec_u8: Option<Vec<u8>>,
    pub boolean: Option<bool>,
    pub null: Option<Null>,
}

pub struct Null {}

pub trait FromAndToJSON {
    fn list_properties() -> Vec<JSONProperty>;

    fn get_property(name: String) -> JSONValue;
}

