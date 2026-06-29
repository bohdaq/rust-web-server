#[cfg(test)]
mod tests;

use crate::controller::Controller;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

/// Readiness probe endpoint: `GET /readyz`.
///
/// Returns `200 OK` when [`crate::metrics::SERVER_READY`] is `true`
/// (set after [`crate::server::Server::setup`] completes and cleared on shutdown).
/// Returns `503 Service Unavailable` during startup or graceful drain.
/// Register this in your Kubernetes `readinessProbe`.
pub struct ReadyController;

impl Controller for ReadyController {
    fn is_matching(request: &Request, _connection: &ConnectionInfo) -> bool {
        request.method == METHOD.get && request.request_uri == "/readyz"
    }

    fn process(_request: &Request, mut response: Response, _connection: &ConnectionInfo) -> Response {
        ReadyController::fill_response(&mut response);
        response
    }
}

impl ReadyController {
    fn fill_response(response: &mut Response) {
        use std::sync::atomic::Ordering;
        if crate::metrics::SERVER_READY.load(Ordering::Relaxed) {
            response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
            response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
            response.content_range_list = vec![
                Range::get_content_range(b"OK".to_vec(), MimeType::TEXT_PLAIN.to_string())
            ];
        } else {
            response.status_code = *STATUS_CODE_REASON_PHRASE.n503_service_unavailable.status_code;
            response.reason_phrase = STATUS_CODE_REASON_PHRASE.n503_service_unavailable.reason_phrase.to_string();
            response.content_range_list = vec![
                Range::get_content_range(b"not ready".to_vec(), MimeType::TEXT_PLAIN.to_string())
            ];
        }
    }

    pub fn is_matching_request(request: &Request) -> bool {
        request.method == METHOD.get && request.request_uri == "/readyz"
    }

    pub fn process_request(_request: &Request, mut response: Response) -> Response {
        ReadyController::fill_response(&mut response);
        response
    }
}
