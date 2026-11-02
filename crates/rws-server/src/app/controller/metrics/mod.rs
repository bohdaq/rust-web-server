#[cfg(test)]
mod tests;

use crate::controller::Controller;
use crate::range::Range;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

/// Prometheus metrics endpoint: `GET /metrics`.
///
/// Returns every metric in the Prometheus text exposition format
/// (`text/plain; version=0.0.4`), or the protobuf format — the one that
/// carries native histograms — when the scraper's `Accept` prefers it.
pub struct MetricsController;

const PROMETHEUS_CONTENT_TYPE: &str = "text/plain; version=0.0.4";

impl Controller for MetricsController {
    fn is_matching(request: &Request, _connection: &ConnectionInfo) -> bool {
        request.method == METHOD.get && request.request_uri == "/metrics"
    }

    fn process(request: &Request, mut response: Response, _connection: &ConnectionInfo) -> Response {
        MetricsController::fill_response(request, &mut response);
        response
    }
}

impl MetricsController {
    /// Text format, or protobuf (with native histograms) when `Accept` asks for it.
    fn fill_response(request: &Request, response: &mut Response) {
        use crate::metrics::exposition;
        let (body, content_type) = match request.header("Accept").map(|h| &*h.value) {
            Some(accept) if exposition::accepts_protobuf(accept) => (exposition::protobuf(), exposition::PROTOBUF_CONTENT_TYPE),
            _ => (crate::metrics::prometheus_text().into_bytes(), PROMETHEUS_CONTENT_TYPE),
        };
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        response.content_range_list = vec![
            Range::get_content_range(body, content_type.to_string())
        ];
    }

    pub fn is_matching_request(request: &Request) -> bool {
        request.method == METHOD.get && request.request_uri == "/metrics"
    }

    pub fn process_request(request: &Request, mut response: Response) -> Response {
        MetricsController::fill_response(request, &mut response);
        response
    }
}
