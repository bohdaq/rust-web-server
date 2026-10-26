//! JSON Schema for Rust types, for MCP tool `inputSchema`/`outputSchema`.
//!
//! [`JsonSchema::json_schema`] returns a type's schema as JSON text. It is
//! implemented for the primitives, `String`, `Option`, `Vec`, sets, maps
//! keyed by strings and smart pointers; `#[derive(JsonSchema)]` (`macros`
//! feature) implements it for a struct with named fields or an enum of unit
//! variants, following the type's `#[serde(...)]` renames so the schema
//! describes what `serde_json` actually reads and writes.
//!
//! The schemas use the subset of JSON Schema 2020-12 MCP clients rely on:
//! `type`, `properties`, `required`, `items`, `additionalProperties`, `enum`,
//! `anyOf` (for `Option`), `minimum`, `description`.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use crate::mcp::json_escape;

/// A type with a JSON Schema.
pub trait JsonSchema {
    /// The schema, as a JSON object.
    fn json_schema() -> String;

    /// `true` when a struct field of this type may be left out (`Option`).
    fn is_optional() -> bool {
        false
    }
}

/// Nesting deeper than this yields `{}` (any value): a type that contains
/// itself (`struct Node { children: Vec<Node> }`) would otherwise recurse
/// forever.
pub const MAX_DEPTH: u32 = 32;

thread_local! {
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// `T`'s schema, or `{}` past [`MAX_DEPTH`] levels of nesting. Used for every
/// field and element type, so recursive types end in "any value".
pub fn nested<T: JsonSchema + ?Sized>() -> String {
    let depth = DEPTH.with(|d| d.get());
    if depth >= MAX_DEPTH {
        return "{}".to_string();
    }
    DEPTH.with(|d| d.set(depth + 1));
    let schema = T::json_schema();
    DEPTH.with(|d| d.set(depth));
    schema
}

/// `schema` with `"description"` added (`schema` must be a JSON object).
pub fn with_description(schema: &str, description: &str) -> String {
    if description.is_empty() {
        return schema.to_string();
    }
    let rest = schema.trim_start().strip_prefix('{').unwrap_or("}").trim_start();
    let separator = if rest.starts_with('}') { "" } else { "," };
    format!(r#"{{"description":"{}"{separator}{rest}"#, json_escape(description))
}

/// Builds an object schema property by property — what `#[derive(JsonSchema)]`
/// expands to.
pub struct ObjectSchema {
    description: String,
    properties: Vec<String>,
    required: Vec<String>,
}

impl ObjectSchema {
    /// An object with `description` (may be empty).
    pub fn new(description: &str) -> Self {
        ObjectSchema { description: description.to_string(), properties: Vec::new(), required: Vec::new() }
    }

    /// Add property `name` with `schema`; `required` lists it in `required`.
    pub fn property(&mut self, name: &str, schema: &str, description: &str, required: bool) {
        self.properties.push(format!(r#""{}":{}"#, json_escape(name), with_description(schema, description)));
        if required {
            self.required.push(format!(r#""{}""#, json_escape(name)));
        }
    }

    /// The finished schema.
    pub fn finish(self) -> String {
        let required = if self.required.is_empty() {
            String::new()
        } else {
            format!(r#","required":[{}]"#, self.required.join(","))
        };
        let schema = format!(r#"{{"type":"object","properties":{{{}}}{required}}}"#, self.properties.join(","));
        with_description(&schema, &self.description)
    }
}

/// A string schema allowing only `values` — a unit-variant enum.
pub fn string_enum(description: &str, values: &[&str]) -> String {
    let values: Vec<String> = values.iter().map(|v| format!(r#""{}""#, json_escape(v))).collect();
    with_description(&format!(r#"{{"type":"string","enum":[{}]}}"#, values.join(",")), description)
}

/// `true` when `schema` describes a JSON object — what MCP requires of an
/// `outputSchema`.
pub fn is_object_schema(schema: &str) -> bool {
    crate::mcp::json_rpc::object_members(schema)
        .and_then(|members| members.into_iter().find(|(k, _)| k == "type"))
        .is_some_and(|(_, v)| v == r#""object""#)
}

macro_rules! fixed_schema {
    ($schema:expr => $($t:ty),*) => {
        $(impl JsonSchema for $t {
            fn json_schema() -> String { $schema.to_string() }
        })*
    };
}

fixed_schema!(r#"{"type":"boolean"}"# => bool);
fixed_schema!(r#"{"type":"integer"}"# => i8, i16, i32, i64, i128, isize);
fixed_schema!(r#"{"type":"integer","minimum":0}"# => u8, u16, u32, u64, u128, usize);
fixed_schema!(r#"{"type":"number"}"# => f32, f64);
fixed_schema!(r#"{"type":"string"}"# => String, str, char);
fixed_schema!(r#"{"type":"null"}"# => ());

impl<T: JsonSchema> JsonSchema for Option<T> {
    fn json_schema() -> String {
        format!(r#"{{"anyOf":[{},{{"type":"null"}}]}}"#, nested::<T>())
    }
    fn is_optional() -> bool {
        true
    }
}

macro_rules! array_schema {
    ($($t:ident),*) => {
        $(impl<T: JsonSchema> JsonSchema for $t<T> {
            fn json_schema() -> String { format!(r#"{{"type":"array","items":{}}}"#, nested::<T>()) }
        })*
    };
}

array_schema!(Vec, VecDeque, BTreeSet);

impl<T: JsonSchema, S> JsonSchema for HashSet<T, S> {
    fn json_schema() -> String {
        format!(r#"{{"type":"array","items":{}}}"#, nested::<T>())
    }
}

impl<T: JsonSchema> JsonSchema for [T] {
    fn json_schema() -> String {
        format!(r#"{{"type":"array","items":{}}}"#, nested::<T>())
    }
}

impl<T: JsonSchema, const N: usize> JsonSchema for [T; N] {
    fn json_schema() -> String {
        format!(r#"{{"type":"array","items":{},"minItems":{N},"maxItems":{N}}}"#, nested::<T>())
    }
}

impl<T: JsonSchema, S> JsonSchema for HashMap<String, T, S> {
    fn json_schema() -> String {
        format!(r#"{{"type":"object","additionalProperties":{}}}"#, nested::<T>())
    }
}

impl<T: JsonSchema> JsonSchema for BTreeMap<String, T> {
    fn json_schema() -> String {
        format!(r#"{{"type":"object","additionalProperties":{}}}"#, nested::<T>())
    }
}

macro_rules! pointer_schema {
    ($($t:ident),*) => {
        $(impl<T: JsonSchema + ?Sized> JsonSchema for $t<T> {
            fn json_schema() -> String { T::json_schema() }
            fn is_optional() -> bool { T::is_optional() }
        })*
    };
}

pointer_schema!(Box, Arc, Rc);

impl<T: JsonSchema + ?Sized> JsonSchema for &T {
    fn json_schema() -> String {
        T::json_schema()
    }
    fn is_optional() -> bool {
        T::is_optional()
    }
}

/// Any JSON value.
#[cfg(feature = "serde")]
impl JsonSchema for serde_json::Value {
    fn json_schema() -> String {
        "{}".to_string()
    }
}

#[cfg(test)]
mod tests;
