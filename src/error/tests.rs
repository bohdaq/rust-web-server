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
