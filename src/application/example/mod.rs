use crate::app::controller::index::IndexController;
use crate::application::Application;
use crate::controller::Controller;
use crate::core::New;
use crate::header::Header;

use crate::request::{Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

#[derive(Copy, Clone)]
pub struct App {}

impl New for App {
    fn new() -> Self {
        App{}
    }
}

impl Application for App {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        // predefined set of headers for response, includes client-hints, cors, vary, security and timestamp
        let header_list = Header::get_header_list(request);

        let mut response: Response = Response::get_response(
            STATUS_CODE_REASON_PHRASE.n501_not_implemented,
            Some(header_list),
            None
        );


        if IndexController::is_matching(request, connection) {
            response = IndexController::process(&request, response, connection);
            return Ok(response)
        }


        Ok(response)
    }
}
