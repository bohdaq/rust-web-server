use std::io::prelude::*;
use std::{env};
use std::borrow::Borrow;

use crate::request::Request;
use crate::response::Response;
use crate::app::App;
use crate::{Config, CONSTANTS};
pub struct Server {}
impl Server {
    pub(crate) fn process_request(mut s: impl Read + Write + Unpin) -> Vec<u8> {
        let mut buffer :[u8; 1024] = [0; 1024];
        let mut stream = s;
        stream.read(&mut buffer).unwrap();
        let request :  &[u8] = &buffer;
        let request: Request = Request::parse_request(request).unwrap();
        let (response, request) = App::handle_request(request);
        let raw_response = Response::generate_response(response, request);
        let boxed_stream = stream.write(raw_response.borrow());
        if boxed_stream.is_ok() {
            stream.flush().unwrap();
        };

        raw_response
    }

    pub(crate) fn truncate_new_line_carriage_return(str: &str) -> String {
        str.replace("\r", "").replace("\n", "")
    }

    pub(crate) fn get_static_filepath(request_uri: &str) -> String {
        let dir = env::current_dir().unwrap();
        let working_directory = dir.as_path().to_str().unwrap();
        [working_directory, request_uri].join(CONSTANTS.EMPTY_STRING)
    }
}
