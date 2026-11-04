//! Names and field types: what `rws gen` accepts, and the spellings it
//! derives (`BlogPost` → `blog_post` → `blog_posts`).

use std::fmt;

use super::GenError;

/// The longest name `rws gen` accepts (a resource, model, migration or
/// field).
pub const MAX_NAME_LEN: usize = 64;

/// Rust's keywords (2015–2024 editions, strict and reserved): never a
/// module, field or function name.
const KEYWORDS: [&str; 52] = [
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop",
    "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final", "macro", "override",
    "priv", "typeof", "unsized", "virtual", "yield", "try", "gen",
];

/// A column type: one of the fixed list, optionally `Option<…>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaseType {
    I64,
    I32,
    F64,
    Bool,
    String,
}

impl BaseType {
    fn parse(text: &str) -> Option<BaseType> {
        match text {
            "i64" => Some(BaseType::I64),
            "i32" => Some(BaseType::I32),
            "f64" => Some(BaseType::F64),
            "bool" => Some(BaseType::Bool),
            "String" => Some(BaseType::String),
            _ => None,
        }
    }

    /// The Rust spelling.
    pub fn rust(self) -> &'static str {
        match self {
            BaseType::I64 => "i64",
            BaseType::I32 => "i32",
            BaseType::F64 => "f64",
            BaseType::Bool => "bool",
            BaseType::String => "String",
        }
    }
}

/// A field's type: `i64`, `i32`, `f64`, `bool`, `String`, or `Option<T>` of
/// one of those.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldType {
    pub base: BaseType,
    pub optional: bool,
}

/// The types a field may have, as the error message lists them.
pub const FIELD_TYPES: &str = "i64, i32, f64, bool, String, Option<T> (T one of those)";

impl FieldType {
    /// `"i64"`, `"Option<String>"`, … — anything else is an error naming
    /// the list.
    pub fn parse(text: &str) -> Result<FieldType, String> {
        let text = text.trim();
        if let Some(base) = BaseType::parse(text) {
            return Ok(FieldType { base, optional: false });
        }
        let inner = text.strip_prefix("Option<").and_then(|rest| rest.strip_suffix('>')).map(str::trim);
        match inner.and_then(BaseType::parse) {
            Some(base) => Ok(FieldType { base, optional: true }),
            None => Err(format!("unknown type `{text}`: use one of {FIELD_TYPES}")),
        }
    }

    /// The Rust spelling (`Option<String>`).
    pub fn rust(&self) -> String {
        if self.optional {
            format!("Option<{}>", self.base.rust())
        } else {
            self.base.rust().to_string()
        }
    }
}

impl fmt::Display for FieldType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.rust())
    }
}

/// One `name:type` field of a model or a route's resource.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    /// The field (and column) name, snake_case.
    pub name: String,
    pub ty: FieldType,
}

impl Field {
    /// `"title:String"`, `"note:Option<String>"`. The name must be a
    /// snake_case identifier other than `id` (every model and resource
    /// gets an `id`).
    pub fn parse(spec: &str) -> Result<Field, GenError> {
        let Some((name, ty)) = spec.split_once(':') else {
            return Err(GenError::usage(format!("field `{spec}`: write name:type, e.g. title:String")));
        };
        let name = name.trim();
        check_snake(name, "field")?;
        if name == "id" {
            return Err(GenError::usage("field `id`: every model and resource gets an `id: i64` already; leave it out"));
        }
        let ty = FieldType::parse(ty).map_err(|e| GenError::usage(format!("field `{name}`: {e}")))?;
        Ok(Field { name: name.to_string(), ty })
    }

    /// The command-line spelling, `name:Type`.
    pub fn spec(&self) -> String {
        format!("{}:{}", self.name, self.ty)
    }
}

/// Parse every `name:type`; names must not repeat.
pub fn parse_fields<S: AsRef<str>>(specs: &[S]) -> Result<Vec<Field>, GenError> {
    let mut fields: Vec<Field> = Vec::new();
    for spec in specs {
        let field = Field::parse(spec.as_ref())?;
        if fields.iter().any(|f| f.name == field.name) {
            return Err(GenError::usage(format!("field `{}` is given twice", field.name)));
        }
        fields.push(field);
    }
    Ok(fields)
}

/// An ASCII identifier: a letter, then letters, digits and `_`.
fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic()) && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A snake_case name a module, file, table or field can have.
pub fn check_snake(name: &str, what: &str) -> Result<(), GenError> {
    if name.is_empty() {
        return Err(GenError::usage(format!("the {what} name is missing")));
    }
    if name.len() > MAX_NAME_LEN {
        let start: String = name.chars().take(MAX_NAME_LEN).collect();
        return Err(GenError::usage(format!("{what} `{start}…` is longer than {MAX_NAME_LEN} characters")));
    }
    if !is_identifier(name) || name.chars().any(|c| c.is_ascii_uppercase()) || name.contains("__") || name.ends_with('_') {
        return Err(GenError::usage(format!(
            "{what} `{name}`: use a snake_case identifier (letters, digits and single `_`, starting with a letter) — it becomes a file name and a Rust identifier"
        )));
    }
    if KEYWORDS.contains(&name) {
        return Err(GenError::usage(format!("{what} `{name}` is a Rust keyword; pick another name")));
    }
    Ok(())
}

/// `BlogPost`, `blogPost` or `blog_post` → `blog_post`; then checked
/// like [`check_snake`].
pub fn to_snake(name: &str, what: &str) -> Result<String, GenError> {
    let name = name.trim();
    if name.is_empty() || name.len() > MAX_NAME_LEN || !is_identifier(name) {
        check_snake(name, what)?;
    }
    let mut out = String::with_capacity(name.len() + 4);
    let mut previous_lower = false;
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            if previous_lower {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
            previous_lower = false;
        } else {
            out.push(c);
            previous_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        }
    }
    check_snake(&out, what)?;
    Ok(out)
}

/// `blog_post` → `BlogPost`.
pub fn to_camel(snake: &str) -> String {
    snake
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// English plural of a snake_case noun, last word only (`blog_post` →
/// `blog_posts`, `category` → `categories`, `box` → `boxes`).
pub fn pluralize(snake: &str) -> String {
    let consonant_y = snake.len() > 1
        && snake.ends_with('y')
        && !matches!(snake.as_bytes()[snake.len() - 2], b'a' | b'e' | b'i' | b'o' | b'u');
    if consonant_y {
        format!("{}ies", &snake[..snake.len() - 1])
    } else if ["s", "x", "z", "ch", "sh"].iter().any(|end| snake.ends_with(end)) {
        format!("{snake}es")
    } else {
        format!("{snake}s")
    }
}

/// English singular of a snake_case plural (`categories` → `category`,
/// `boxes` → `box`, `posts` → `post`); a word that doesn't look plural
/// stays as it is.
pub fn singularize(snake: &str) -> String {
    if snake.len() > 3 && snake.ends_with("ies") {
        return format!("{}y", &snake[..snake.len() - 3]);
    }
    for end in ["ses", "xes", "zes", "ches", "shes"] {
        if snake.len() > end.len() && snake.ends_with(end) {
            return snake[..snake.len() - 2].to_string();
        }
    }
    if snake.len() > 1 && snake.ends_with('s') && !snake.ends_with("ss") && !snake.ends_with("us") {
        return snake[..snake.len() - 1].to_string();
    }
    snake.to_string()
}

/// Type names the generated modules import or use; a model or resource
/// type can't be one of them.
pub const RESERVED_TYPES: [&str; 21] = [
    "Self", "Option", "Result", "String", "Vec", "Box", "Json", "Path", "Value", "AppError", "Mutex", "MutexGuard", "BTreeMap",
    "Serialize", "Deserialize", "StatusCodeReasonPhrase", "AppWithState", "AsyncAppWithState", "Model", "DbPool", "Repository",
];

/// Refuse a type name the generated code would clash with.
pub fn check_type_name(camel: &str, what: &str) -> Result<(), GenError> {
    if RESERVED_TYPES.contains(&camel) {
        return Err(GenError::usage(format!(
            "{what} type `{camel}` would clash with a name the generated code uses ({}); pick another name",
            RESERVED_TYPES.join(", ")
        )));
    }
    Ok(())
}
