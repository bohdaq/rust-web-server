use std::io::prelude::*;
use std::net::TcpStream;
use std::{env, fs};

use crate::request::Request;
use crate::response::Response;
use crate::app::App;
use crate::CONSTANTS;
use crate::constant::{HTTP_VERSIONS, REQUEST_METHODS, RESPONSE_STATUS_CODE_REASON_PHRASES};


pub struct Server {}


impl Server {
    pub(crate) fn handle_connection(s: TcpStream) {
        let mut buffer = [0; 1024];

        let mut stream = s;

        stream.read(&mut buffer).unwrap();

        let request_string = String::from_utf8_lossy(&buffer[..]).to_string();

        let response = Server::process_request(request_string);

        stream.write(response.as_bytes()).unwrap();
        stream.flush().unwrap();

    }

    pub(crate) fn process_request(request_string: String) -> String {
        let request: Request = Request::parse_request(&request_string);
        let response = App::handle_request(request);
        let raw_response = Response::generate_response(response);

        raw_response
    }
}


