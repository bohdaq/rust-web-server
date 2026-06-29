#[cfg(test)]
mod tests;

use crate::controller::Controller;
use crate::range::Range;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

/// Prometheus metrics endpoint: `GET /metrics`.
///
/// Returns request counters and active-connection gauge in Prometheus
/// text exposition format (`text/plain; version=0.0.4`).
pub struct MetricsController;

const PROMETHEUS_CONTENT_TYPE: &str = "text/plain; version=0.0.4";

impl Controller for MetricsController {
    fn is_matching(request: &Request, _connection: &ConnectionInfo) -> bool {
        request.method == METHOD.get && request.request_uri == "/metrics"
    }

    fn process(_request: &Request, mut response: Response, _connection: &ConnectionInfo) -> Response {
        MetricsController::fill_response(&mut response);
        response
    }
}

impl MetricsController {
    fn fill_response(response: &mut Response) {
        let body = crate::metrics::prometheus_text().into_bytes();
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        response.content_range_list = vec![
            Range::get_content_range(body, PROMETHEUS_CONTENT_TYPE.to_string())
        ];
    }

    pub fn is_matching_request(request: &Request) -> bool {
        request.method == METHOD.get && request.request_uri == "/metrics"
    }

    pub fn process_request(_request: &Request, mut response: Response) -> Response {
        MetricsController::fill_response(&mut response);
        response
    }
}
