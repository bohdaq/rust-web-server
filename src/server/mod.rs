#[cfg(test)]
pub mod tests;

use std::io::prelude::*;
use std::borrow::Borrow;

use crate::request::Request;
use crate::response::Response;
use crate::app::App;

pub struct Server {}
impl Server {
    pub fn process_request(mut stream: impl Read + Write + Unpin) -> Vec<u8> {
        let mut buffer :[u8; 1024] = [0; 1024];
        stream.read(&mut buffer).unwrap();
        let request : &[u8] = &buffer;


        let boxed_request = Request::parse_request(request);
        if boxed_request.is_err() {
            eprintln!("unable to parse request: {}", boxed_request.err().unwrap());
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

}
