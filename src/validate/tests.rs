use super::*;
use crate::core::New;
use crate::extract::FromRequest;
use crate::http::VERSION;
use crate::request::Request;
use crate::response::Response;

fn make_req(body: &[u8]) -> Request {
    Request {
        method: "POST".to_string(),
        request_uri: "/".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: body.to_vec(),
    }
}

// ── Manual Validate impl used by Validated<T> tests ──────────────────────────

struct Name(String);

impl FromRequest for Name {
    fn from_request(req: &Request) -> Result<Self, Response> {
        Ok(Name(String::from_utf8_lossy(&req.body).to_string()))
    }
}

impl Validate for Name {
    fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();
        if self.0.is_empty() {
            errors.add("name", "must not be empty");
        }
        if self.0.chars().count() > 50 {
            errors.add("name", "must be at most 50 characters long");
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}

// ── ValidationErrors ──────────────────────────────────────────────────────────

#[test]
fn new_errors_is_empty() {
    assert!(ValidationErrors::new().is_empty());
}

#[test]
fn add_records_field_and_message() {
    let mut e = ValidationErrors::new();
    e.add("name", "too short");
    assert!(!e.is_empty());
    assert_eq!(1, e.errors().len());
    assert_eq!("name", e.errors()[0].field);
    assert_eq!("too short", e.errors()[0].message);
}

#[test]
fn add_multiple_errors() {
    let mut e = ValidationErrors::new();
    e.add("name", "too short");
    e.add("email", "invalid");
    assert_eq!(2, e.errors().len());
}

#[test]
fn into_json_single_error() {
    let mut e = ValidationErrors::new();
    e.add("email", "invalid format");
    let json = e.into_json();
    assert!(json.starts_with('{'));
    assert!(json.contains("\"errors\""));
    assert!(json.contains("\"field\":\"email\""));
    assert!(json.contains("\"message\":\"invalid format\""));
}

#[test]
fn into_json_multiple_errors() {
    let mut e = ValidationErrors::new();
    e.add("name", "too short");
    e.add("age", "out of range");
    let json = e.into_json();
    assert!(json.contains("\"name\""));
    assert!(json.contains("\"age\""));
}

#[test]
fn into_json_escapes_backslash_and_quotes() {
    let mut e = ValidationErrors::new();
    e.add("f\\ield", "has \"quotes\"");
    let json = e.into_json();
    assert!(json.contains("f\\\\ield"));
    assert!(json.contains("\\\"quotes\\\""));
}

// ── is_email ──────────────────────────────────────────────────────────────────

#[test]
fn valid_emails_pass() {
    assert!(is_email("user@example.com"));
    assert!(is_email("a@b.co"));
    assert!(is_email("user+tag@sub.domain.org"));
}

#[test]
fn missing_at_sign_rejected() {
    assert!(!is_email("nodomain.com"));
}

#[test]
fn empty_local_part_rejected() {
    assert!(!is_email("@example.com"));
}

#[test]
fn domain_with_no_dot_rejected() {
    assert!(!is_email("user@localhost"));
}

#[test]
fn domain_starting_with_dot_rejected() {
    assert!(!is_email("user@.example.com"));
}

#[test]
fn domain_ending_with_dot_rejected() {
    assert!(!is_email("user@example.com."));
}

#[test]
fn empty_string_rejected() {
    assert!(!is_email(""));
}

// ── is_url ────────────────────────────────────────────────────────────────────

#[test]
fn http_url_accepted() {
    assert!(is_url("http://example.com"));
    assert!(is_url("http://example.com/path?q=1#frag"));
}

#[test]
fn https_url_accepted() {
    assert!(is_url("https://example.com"));
}

#[test]
fn ftp_and_bare_domain_rejected() {
    assert!(!is_url("ftp://example.com"));
    assert!(!is_url("example.com"));
    assert!(!is_url(""));
}

// ── Validated<T> ──────────────────────────────────────────────────────────────

#[test]
fn validated_ok_when_extraction_and_validation_succeed() {
    let req = make_req(b"alice");
    assert!(Validated::<Name>::from_request(&req).is_ok());
}

#[test]
fn validated_returns_422_when_validation_fails() {
    let req = make_req(b""); // empty → validation error
    let result = Validated::<Name>::from_request(&req);
    assert!(result.is_err());
    match result {
        Err(resp) => assert_eq!(422, resp.status_code),
        Ok(_) => panic!("expected Err"),
    }
}

#[test]
fn validated_422_body_is_json_with_errors_key() {
    let req = make_req(b"");
    match Validated::<Name>::from_request(&req) {
        Err(resp) => {
            let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
            assert!(body.contains("\"errors\""));
            assert!(body.contains("\"field\":\"name\""));
        }
        Ok(_) => panic!("expected Err"),
    }
}

#[test]
fn validated_extraction_error_returns_400_not_422() {
    // BodyText fails on invalid UTF-8 before validation even runs
    struct MustBeUtf8(String);

    impl FromRequest for MustBeUtf8 {
        fn from_request(req: &Request) -> Result<Self, Response> {
            String::from_utf8(req.body.clone())
                .map(MustBeUtf8)
                .map_err(|_| {
                    let mut r = Response::new();
                    r.status_code = 400;
                    r
                })
        }
    }

    impl Validate for MustBeUtf8 {
        fn validate(&self) -> Result<(), ValidationErrors> { Ok(()) }
    }

    let req = make_req(&[0xFF, 0xFE]);
    match Validated::<MustBeUtf8>::from_request(&req) {
        Err(resp) => assert_eq!(400, resp.status_code),
        Ok(_) => panic!("expected Err"),
    }
}

// ── #[derive(Validate)] ───────────────────────────────────────────────────────

#[cfg(feature = "macros")]
mod derive {
    use crate::validate::Validate;

    #[derive(rust_web_server::Validate)]
    struct CreateUser {
        #[validate(length(min = 1, max = 50))]
        name: String,
        #[validate(email)]
        email: String,
        #[validate(range(min = 0, max = 150))]
        age: u8,
    }

    fn valid_user() -> CreateUser {
        CreateUser {
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
            age: 30,
        }
    }

    #[test]
    fn all_valid_fields_ok() {
        assert!(valid_user().validate().is_ok());
    }

    #[test]
    fn empty_name_fails_length_min() {
        let u = CreateUser { name: "".to_string(), ..valid_user() };
        let err = u.validate().unwrap_err();
        assert!(err.errors().iter().any(|e| e.field == "name"));
    }

    #[test]
    fn name_over_max_fails_length_max() {
        let u = CreateUser { name: "a".repeat(51), ..valid_user() };
        let err = u.validate().unwrap_err();
        assert!(err.errors().iter().any(|e| e.field == "name"));
    }

    #[test]
    fn invalid_email_fails_email_validator() {
        let u = CreateUser { email: "not-an-email".to_string(), ..valid_user() };
        let err = u.validate().unwrap_err();
        assert!(err.errors().iter().any(|e| e.field == "email"));
    }

    #[test]
    fn age_over_150_fails_range_max() {
        // u8 max is 255, so 200 is a valid u8 but should fail the range(max=150) check
        let u = CreateUser { age: 200, ..valid_user() };
        let err = u.validate().unwrap_err();
        assert!(err.errors().iter().any(|e| e.field == "age"));
    }

    #[test]
    fn multiple_invalid_fields_accumulate_all_errors() {
        let u = CreateUser {
            name: "".to_string(),
            email: "bad".to_string(),
            age: 200,
        };
        let err = u.validate().unwrap_err();
        assert!(err.errors().len() >= 2);
    }

    #[derive(rust_web_server::Validate)]
    struct UrlPayload {
        #[validate(url)]
        homepage: String,
    }

    #[test]
    fn url_validator_accepts_https() {
        let p = UrlPayload { homepage: "https://example.com".to_string() };
        assert!(p.validate().is_ok());
    }

    #[test]
    fn url_validator_rejects_bare_domain() {
        let p = UrlPayload { homepage: "example.com".to_string() };
        let err = p.validate().unwrap_err();
        assert!(err.errors().iter().any(|e| e.field == "homepage"));
    }

    #[derive(rust_web_server::Validate)]
    struct RequiredPayload {
        #[validate(required)]
        value: String,
    }

    #[test]
    fn required_rejects_empty_string() {
        let p = RequiredPayload { value: "".to_string() };
        let err = p.validate().unwrap_err();
        assert!(err.errors().iter().any(|e| e.field == "value"));
    }

    #[test]
    fn required_accepts_non_empty_string() {
        let p = RequiredPayload { value: "hello".to_string() };
        assert!(p.validate().is_ok());
    }

    #[derive(rust_web_server::Validate)]
    struct Empty {}

    #[test]
    fn empty_struct_always_validates_ok() {
        assert!(Empty {}.validate().is_ok());
    }
}
