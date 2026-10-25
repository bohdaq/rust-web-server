//! A dependency-free JSON parser and serializer.
//!
//! You describe a struct and implement [`New`](crate::core::New),
//! [`FromJSON`](object::FromJSON) and [`ToJSON`](object::ToJSON) for it; the trait bodies call the module's helpers
//! for each property. Nested objects and arrays are supported. It is not a
//! validator or formatter, but invalid input is an error, not a panic.
//! Submodules: [`object`], [`array`], [`property`]; examples live in their
//! `tests`/`example_*` modules (objects, nested objects, arrays of every scalar
//! type, arrays of objects).
//!
//! With the `serde` feature, `Json<T>` extracts a `Deserialize` type
//! from a request body and turns a `Serialize` type into an `application/json`
//! response — use that unless you want the explicit, macro-free mapping.
//!
//! # FAQ
//!
//! - **Line endings.** The parser doesn't care about `\n` vs `\r\n`;
//!   `to_json_string` writes `\r\n`, so compare against `\r\n` text in tests.
//! - **No pretty-printing.** `to_json_string` adds line breaks but doesn't indent
//!   nested values.
//! - **Why `New`?** Parsing an array has to create instances of your struct.
//! - **Mixed-type arrays** (`[1, true, "x"]`, tuples) don't map to one struct
//!   type; model them as an array of objects, or split the raw array with
//!   `RawUnprocessedJSONArray::split_into_vector_of_strings` and parse each item.
//! - **Numbers** are `i128` and `f64` (JSON has one number type); cast in your
//!   `FromJSON` if you need smaller types. `f64` has the usual binary rounding
//!   (`4356.257` reads back as `4356.2569999999996`) — store money as integer
//!   cents, or round when formatting.
//! - **Enums** have no JSON form: keep a string/integer property and map it to
//!   your enum in `FromJSON`/`ToJSON`.

#[cfg(test)]
mod tests;
pub mod property;
pub mod array;
pub mod object;

#[cfg(feature = "serde")]
mod extractor;
#[cfg(feature = "serde")]
pub use extractor::Json;

// TODO: wip

pub struct JSONType {
    pub string: &'static str,
    pub boolean: &'static str,
    pub object: &'static str,
    pub array: &'static str,
    pub integer: &'static str,
    pub number: &'static str,
    pub null: &'static str,
}

pub const JSON_TYPE: JSONType = JSONType{
    string: "String",
    boolean: "bool",
    object: "object",
    array: "array",
    integer: "i128",
    number: "f64",
    null: "null",
};








