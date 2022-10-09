#[cfg(test)]
pub mod tests;

use std::io::prelude::*;
use std::borrow::Borrow;

use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::app::App;
use crate::header::Header;

pub struct Server {}
impl Server {
    pub fn process_request(mut stream: impl Read + Write + Unpin) -> Vec<u8> {
        let mut buffer :[u8; 1024] = [0; 1024];
        let boxed_read = stream.read(&mut buffer);
        if boxed_read.is_err() {
            eprintln!("unable to read TCP stream {}", boxed_read.err().unwrap());

            let raw_response = Server::bad_request_response();
            let boxed_stream = stream.write(raw_response.borrow());
            if boxed_stream.is_ok() {
                stream.flush().unwrap();
            };
            return vec![];
        }

        boxed_read.unwrap();
        let request : &[u8] = &buffer;


        let boxed_request = Request::parse_request(request);
        if boxed_request.is_err() {
            eprintln!("unable to parse request: {}", boxed_request.err().unwrap());

            let raw_response = Server::bad_request_response();
            let boxed_stream = stream.write(raw_response.borrow());
            if boxed_stream.is_ok() {
                stream.flush().unwrap();
            };
            return vec![];
        }


        let request: Request = boxed_request.unwrap();
        let (response, request) = App::handle_request(request);
        let raw_response = Response::generate_response(response, request);

        let boxed_stream = stream.write(raw_response.borrow());
        if boxed_stream.is_ok() {
            stream.flush().unwrap();
        };

        raw_response
    }

    pub fn bad_request_response() -> Vec<u8> {
        let error_request = Request {
            method: METHOD.head.to_string(),
            request_uri: "".to_string(),
            http_version: "".to_string(),
            headers: vec![]
        };

        let header_list = Header::get_header_list(&error_request);
        let error_response: Response = Response::get_response(
            STATUS_CODE_REASON_PHRASE.n400_bad_request,
            Some(header_list),
            None
        );

        let response = Response::generate_response(error_response, error_request);
        return response;
    }

}
