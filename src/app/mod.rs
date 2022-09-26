#[cfg(test)]
mod tests;

pub mod controller;

use crate::app::controller::index::IndexController;
use crate::app::controller::not_found::NotFoundController;
use crate::app::controller::static_resource::StaticResourceController;
use crate::cors::Cors;
use crate::header::Header;

use crate::request::{Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};


pub struct App {}

impl App {
    pub fn handle_request(request: Request) -> (Response, Request) {

        let cors_header_list : Vec<Header> = Cors::get_headers(&request);

        let mut response: Response = Response::get_response(
            STATUS_CODE_REASON_PHRASE.n501_not_implemented,
            Some(cors_header_list),
            None
        ).unwrap();



        if IndexController::is_matching_request(&request) {
            response = IndexController::process_request(&request, response);
            return (response, request)
        }

        if StaticResourceController::is_matching_request(&request) {
            response = StaticResourceController::process_request(&request, response);
            return (response, request)
        }

        if NotFoundController::is_matching_request(&request) {
            response = NotFoundController::process_request(&request, response);
            return (response, request)
        }


        (response, request)
    }

}