use std::io::prelude::*;
use std::net::TcpStream;
use std::{env, fs, io};
use std::borrow::Borrow;
use std::io::BufReader;

use crate::request::Request;
use crate::response::Response;
use crate::app::App;
use crate::CONSTANTS;
use crate::constant::{HTTP_VERSIONS, REQUEST_METHODS, RESPONSE_STATUS_CODE_REASON_PHRASES};


pub struct Server {}


impl Server {
    pub(crate) fn handle_connection(s: TcpStream) {
        let mut buffer :[u8; 1024] = [0; 1024];

        let mut stream = s;

        stream.read(&mut buffer).unwrap();


        let response = Server::process_request(&buffer);

        stream.write(response.borrow()).unwrap();
        stream.flush().unwrap();

    }

    pub(crate) fn process_request(request: &[u8]) -> Vec<u8> {
        let request: Request = Request::parse_request(request);
        let response = App::handle_request(request);
        let raw_response = Response::generate_response(response);

        raw_response
    }

    pub(crate) fn truncate_new_line_carriage_return(str: &str) -> String {
        str.replace("\r", "").replace("\n", "")
    }

    pub(crate) fn get_static_filepath(request_uri: &str) -> String {
        let dir = env::current_dir().unwrap();
        let working_directory = dir.as_path().to_str().unwrap();
        let static_filepath = [working_directory, request_uri].join(CONSTANTS.EMPTY_STRING);

        static_filepath
    }
}


