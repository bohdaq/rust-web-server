use crate::error::{AppError, IntoResponse};
use crate::response::STATUS_CODE_REASON_PHRASE;

#[test]
fn bad_request_maps_to_400() {
    let r = AppError::BadRequest("invalid input".to_string()).into_response();
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n400_bad_request.status_code, r.status_code);
}

#[test]
fn unauthorized_maps_to_401() {
    let r = AppError::Unauthorized.into_response();
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n401_unauthorized.status_code, r.status_code);
}

#[test]
fn forbidden_maps_to_403() {
    let r = AppError::Forbidden.into_response();
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n403_forbidden.status_code, r.status_code);
}

#[test]
fn not_found_maps_to_404() {
    let r = AppError::NotFound("item missing".to_string()).into_response();
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n404_not_found.status_code, r.status_code);
}

#[test]
fn conflict_maps_to_409() {
    let r = AppError::Conflict("duplicate".to_string()).into_response();
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n409_conflict.status_code, r.status_code);
}

#[test]
fn unprocessable_entity_maps_to_422() {
    let r = AppError::UnprocessableEntity("bad value".to_string()).into_response();
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n422_unprocessable_entity.status_code, r.status_code);
}

#[test]
fn too_many_requests_maps_to_429() {
    let r = AppError::TooManyRequests.into_response();
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n429_too_many_requests.status_code, r.status_code);
}

#[test]
fn payload_too_large_maps_to_413() {
    let r = AppError::PayloadTooLarge("body exceeds 1048576 bytes".to_string()).into_response();
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n413_payload_too_large.status_code, r.status_code);
    let body = String::from_utf8(r.content_range_list[0].body.clone()).unwrap();
    assert!(body.contains("1048576"));
}

#[test]
fn internal_maps_to_500() {
    let r = AppError::Internal("db crash".to_string()).into_response();
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n500_internal_server_error.status_code, r.status_code);
}

#[test]
fn error_body_contains_message() {
    let r = AppError::NotFound("user 42 not found".to_string()).into_response();
    let body = String::from_utf8(r.content_range_list[0].body.clone()).unwrap();
    assert!(body.contains("user 42 not found"));
}

#[test]
fn response_implements_into_response() {
    use crate::core::New;
    use crate::response::Response;
    let mut resp = Response::new();
    resp.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    let converted = resp.clone().into_response();
    assert_eq!(resp.status_code, converted.status_code);
}

#[test]
fn rws_errors_are_problem_details_unless_text_is_asked_for() {
    let _g = crate::test_env::lock();
    let body = |r: &crate::response::Response| String::from_utf8(r.content_range_list[0].body.clone()).unwrap();
    let content_type = |r: &crate::response::Response| r.content_range_list[0].content_type.clone();

    std::env::remove_var("RWS_CONFIG_ERROR_FORMAT");
    crate::server_config::invalidate();
    let r = crate::error::problem(STATUS_CODE_REASON_PHRASE.n502_bad_gateway, "");
    assert_eq!(502, r.status_code);
    assert_eq!(crate::error::problem::PROBLEM_JSON, content_type(&r));
    assert_eq!(r#"{"type":"about:blank","title":"Bad Gateway","status":502}"#, body(&r));
    let r = crate::error::problem(STATUS_CODE_REASON_PHRASE.n413_payload_too_large, "a \"big\" body\n");
    assert_eq!(r#"{"type":"about:blank","title":"Payload Too Large","status":413,"detail":"a \"big\" body\n"}"#, body(&r));
    let r = AppError::NotFound("no such invoice".to_string()).into_response();
    assert_eq!(r#"{"type":"about:blank","title":"Not Found","status":404,"detail":"no such invoice"}"#, body(&r));
    let r = AppError::Forbidden.into_response();
    assert_eq!(r#"{"type":"about:blank","title":"Forbidden","status":403}"#, body(&r));
    let (b, ct) = crate::error::problem::from_text(STATUS_CODE_REASON_PHRASE.n431_request_header_fields_too_large, "431 Request Header Fields Too Large: the head exceeds 65536 bytes");
    assert_eq!(crate::error::problem::PROBLEM_JSON, ct);
    assert!(String::from_utf8(b).unwrap().ends_with(r#""detail":"the head exceeds 65536 bytes"}"#));

    std::env::set_var("RWS_CONFIG_ERROR_FORMAT", "text");
    crate::server_config::invalidate();
    let r = crate::error::problem(STATUS_CODE_REASON_PHRASE.n502_bad_gateway, "");
    assert_eq!(("502 Bad Gateway".to_string(), "text/plain".to_string()), (body(&r), content_type(&r)));
    let r = crate::error::problem(STATUS_CODE_REASON_PHRASE.n503_service_unavailable, "every backend is out of rotation");
    assert_eq!("503 Service Unavailable: every backend is out of rotation", body(&r));
    assert_eq!("no such invoice", body(&AppError::NotFound("no such invoice".to_string()).into_response()));
    let r = crate::error::problem::problem_with_text(STATUS_CODE_REASON_PHRASE.n501_not_implemented, "x", "");
    assert!(r.content_range_list.is_empty(), "an empty text keeps the bodiless response");

    std::env::remove_var("RWS_CONFIG_ERROR_FORMAT");
    crate::server_config::invalidate();
}

#[test]
fn result_responds_with_whichever_side_it_holds() {
    let ok: Result<String, AppError> = Ok("fine".to_string());
    let r = ok.into_response();
    assert_eq!(200, r.status_code);
    assert_eq!(b"fine".to_vec(), r.content_range_list[0].body);

    let err: Result<String, AppError> = Err(AppError::NotFound("gone".to_string()));
    assert_eq!(404, err.into_response().status_code);
}

#[test]
fn strings_are_plain_text_200s() {
    let r = "hello".into_response();
    assert_eq!(200, r.status_code);
    assert_eq!(crate::mime_type::MimeType::TEXT_PLAIN, r.content_range_list[0].content_type);
    assert_eq!(b"hello".to_vec(), r.content_range_list[0].body);
    assert_eq!(b"owned".to_vec(), "owned".to_string().into_response().content_range_list[0].body);
}

#[test]
fn a_status_tuple_replaces_the_inner_status() {
    let r = (STATUS_CODE_REASON_PHRASE.n201_created, "made").into_response();
    assert_eq!(201, r.status_code);
    assert_eq!("Created", r.reason_phrase);
    assert_eq!(b"made".to_vec(), r.content_range_list[0].body);
    // An error keeps its body but takes the tuple's status.
    let r = (STATUS_CODE_REASON_PHRASE.n409_conflict, AppError::BadRequest("x".to_string())).into_response();
    assert_eq!(409, r.status_code);
}
