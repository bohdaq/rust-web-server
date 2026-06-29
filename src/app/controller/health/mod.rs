#[cfg(test)]
mod tests;

use crate::controller::Controller;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

/// Liveness probe endpoint: `GET /healthz`.
///
/// Returns `200 OK` whenever the process is alive.
/// Register this in your Kubernetes `livenessProbe`.
pub struct HealthController;

impl Controller for HealthController {
    fn is_matching(request: &Request, _connection: &ConnectionInfo) -> bool {
        request.method == METHOD.get && request.request_uri == "/healthz"
    }

    fn process(_request: &Request, mut response: Response, _connection: &ConnectionInfo) -> Response {
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        response.content_range_list = vec![
            Range::get_content_range(b"OK".to_vec(), MimeType::TEXT_PLAIN.to_string())
        ];
        response
    }
}

impl HealthController {
    pub fn is_matching_request(request: &Request) -> bool {
        request.method == METHOD.get && request.request_uri == "/healthz"
    }

    pub fn process_request(_request: &Request, mut response: Response) -> Response {
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        response.content_range_list = vec![
            Range::get_content_range(b"OK".to_vec(), MimeType::TEXT_PLAIN.to_string())
        ];
        response
    }
}
