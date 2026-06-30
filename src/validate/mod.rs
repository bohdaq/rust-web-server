#[cfg(test)]
mod tests;

use crate::core::New;
use crate::extract::FromRequest;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

/// A single field that failed validation.
#[derive(Debug, Clone)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

/// Collection of per-field validation errors returned by [`Validate::validate`].
///
/// Build one inside a manual `Validate` impl and return it as `Err(errors)` if
/// `!errors.is_empty()`.
#[derive(Debug)]
pub struct ValidationErrors {
    errors: Vec<FieldError>,
}

impl ValidationErrors {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Record a validation failure for `field` with a human-readable `message`.
    pub fn add(&mut self, field: &str, message: &str) {
        self.errors.push(FieldError {
            field: field.to_string(),
            message: message.to_string(),
        });
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn errors(&self) -> &[FieldError] {
        &self.errors
    }

    /// Serialise as `{"errors":[{"field":"…","message":"…"},…]}`.
    pub fn into_json(&self) -> String {
        let entries: Vec<String> = self
            .errors
            .iter()
            .map(|e| {
                format!(
                    "{{\"field\":\"{}\",\"message\":\"{}\"}}",
                    escape(&e.field),
                    escape(&e.message),
                )
            })
            .collect();
        format!("{{\"errors\":[{}]}}", entries.join(","))
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Types that can be validated field-by-field.
///
/// Implement manually or derive with `#[derive(Validate)]`
/// (requires `features = ["macros"]`).
///
/// # Example — manual implementation
///
/// ```rust,no_run
/// use rust_web_server::validate::{Validate, ValidationErrors};
///
/// struct Payload { name: String }
///
/// impl Validate for Payload {
///     fn validate(&self) -> Result<(), ValidationErrors> {
///         let mut errors = ValidationErrors::new();
///         if self.name.is_empty() { errors.add("name", "must not be empty"); }
///         if errors.is_empty() { Ok(()) } else { Err(errors) }
///     }
/// }
/// ```
///
/// # Example — derive macro
///
/// ```ignore
/// use rust_web_server::validate::Validate;
///
/// #[derive(rust_web_server::Validate)]
/// struct CreateUser {
///     #[validate(length(min = 1, max = 50))]
///     name: String,
///     #[validate(email)]
///     email: String,
///     #[validate(range(min = 0, max = 150))]
///     age: u8,
/// }
/// ```
///
/// Supported validators: `email`, `required`, `url`,
/// `length(min = N, max = N)`, `range(min = N, max = N)`.
pub trait Validate {
    fn validate(&self) -> Result<(), ValidationErrors>;
}

/// Wraps a `T: FromRequest + Validate`, extracting and validating in one step.
///
/// Returns `422 Unprocessable Entity` with a JSON error body if validation
/// fails, or the upstream extraction error (typically 400) if extraction fails.
///
/// # Example
///
/// ```rust,no_run
/// use rust_web_server::validate::{Validate, Validated, ValidationErrors};
/// use rust_web_server::extract::{FromRequest, BodyText};
/// use rust_web_server::core::New;
/// use rust_web_server::request::Request;
/// use rust_web_server::response::Response;
///
/// struct Name(String);
///
/// impl FromRequest for Name {
///     fn from_request(req: &Request) -> Result<Self, Response> {
///         let BodyText(s) = BodyText::from_request(req)?;
///         Ok(Name(s))
///     }
/// }
///
/// impl Validate for Name {
///     fn validate(&self) -> Result<(), ValidationErrors> {
///         let mut errors = ValidationErrors::new();
///         if self.0.is_empty() { errors.add("name", "must not be empty"); }
///         if errors.is_empty() { Ok(()) } else { Err(errors) }
///     }
/// }
///
/// fn handle(req: &Request) -> Response {
///     let Validated(name) = match Validated::<Name>::from_request(req) {
///         Ok(v)    => v,
///         Err(res) => return res,  // 400 or 422
///     };
///     Response::new()
/// }
/// ```
pub struct Validated<T>(pub T);

impl<T: FromRequest + Validate> FromRequest for Validated<T> {
    fn from_request(request: &Request) -> Result<Self, Response> {
        let value = T::from_request(request)?;
        value.validate().map_err(validation_error_response)?;
        Ok(Validated(value))
    }
}

fn validation_error_response(errors: ValidationErrors) -> Response {
    let json = errors.into_json();
    let cr = Range::get_content_range(json.into_bytes(), MimeType::APPLICATION_JSON.to_string());
    let mut response = Response::new();
    response.status_code = *STATUS_CODE_REASON_PHRASE.n422_unprocessable_entity.status_code;
    response.reason_phrase = STATUS_CODE_REASON_PHRASE
        .n422_unprocessable_entity
        .reason_phrase
        .to_string();
    response.content_range_list = vec![cr];
    response
}

/// Returns `true` if `s` is a plausible email address.
///
/// Checks for a non-empty local part, exactly one `@`, and a domain that
/// contains at least one `.` and does not start or end with `.`.
pub fn is_email(s: &str) -> bool {
    let mut parts = s.splitn(2, '@');
    let local = parts.next().unwrap_or("");
    let domain = match parts.next() {
        Some(d) => d,
        None => return false,
    };
    !local.is_empty()
        && domain.len() >= 3
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
}

/// Returns `true` if `s` starts with `http://` or `https://`.
pub fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}
