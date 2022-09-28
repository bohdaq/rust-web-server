#[cfg(test)]
mod tests;

pub mod controller;

use crate::app::controller::index::IndexController;
use crate::app::controller::not_found::NotFoundController;
use crate::app::controller::static_resource::StaticResourceController;
use crate::client_hint::ClientHint;
use crate::cors::Cors;
use crate::header::Header;

use crate::request::{Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};


pub struct App {}

impl App {
    pub fn handle_request(request: Request) -> (Response, Request) {
        let mut header_list : Vec<Header>;
        let mut vary_value : Vec<String>;

        let cors_vary = Cors::get_vary_header_value();
        vary_value = vec![cors_vary];
        let cors_header_list: Vec<Header> = Cors::get_headers(&request);
        header_list = cors_header_list;

        let boxed_client_hint_header = ClientHint::get_accept_client_hints_header();
        if boxed_client_hint_header.is_some() {
            let client_hint_vary = ClientHint::get_vary_header_value();
            vary_value.push(client_hint_vary);
            let client_hint_header = boxed_client_hint_header.unwrap();
            header_list.push(client_hint_header);
        }

        let vary_header = Header { name: Header::VARY.to_string(), value: vary_value.join(", ") };
        header_list.push(vary_header);

        let x_content_type_options_header = Header::get_x_content_type_options_header();
        header_list.push(x_content_type_options_header);

        let accept_ranges_header = Header::get_accept_ranges_header();
        header_list.push(accept_ranges_header);

        let mut response: Response = Response::get_response(
            STATUS_CODE_REASON_PHRASE.n501_not_implemented,
            Some(header_list),
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