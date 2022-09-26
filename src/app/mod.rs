#[cfg(test)]
mod tests;

pub mod controller;

use std::{env};
use crate::app::controller::index::IndexController;
use crate::app::controller::not_found::NotFoundController;
use crate::app::controller::static_resource::StaticResourceController;
use crate::cors::Cors;
use crate::entry_point::Config;
use crate::header::Header;

use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::symbol::SYMBOL;


pub struct App {}

impl App {
    pub fn handle_request(request: Request) -> (Response, Request) {

        let mut response: Response = Response::get_response(
            STATUS_CODE_REASON_PHRASE.n501_not_implemented,
            None,
            None
        ).unwrap();

        // by default we assume route or static asset is not found
        if NotFoundController::is_matching_request(&request) {
            response = NotFoundController::process_request(&request, response)
        }


        // index controller
        if IndexController::is_matching_request(&request) {
            response = IndexController::process_request(&request, response)
        }

        // static resources controller
        if StaticResourceController::is_matching_request(&request) {
            response = StaticResourceController::process_request(&request, response)
        }


        // cors wildcard controller
        if request.request_uri != SYMBOL.slash && request.method == METHOD.post {

            response = Response::get_response(
                STATUS_CODE_REASON_PHRASE.n200_ok,
                None,
                None
            ).unwrap();

        }

        let mut cors_header_list : Vec<Header>;
        let is_cors_set_to_allow_all_requests : bool = env::var(Config::RWS_CONFIG_CORS_ALLOW_ALL).unwrap().parse().unwrap();
        if is_cors_set_to_allow_all_requests {
            cors_header_list = Cors::allow_all(&request).unwrap();
        } else {
            cors_header_list = Cors::process_using_default_config(&request).unwrap();
        }

        response.headers.append(&mut cors_header_list);


        (response, request)
    }

}