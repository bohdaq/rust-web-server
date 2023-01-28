#[cfg(test)]
pub mod tests;

use std::io::prelude::*;
use std::borrow::Borrow;
use std::net::SocketAddr;

use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::app::App;
use crate::entry_point::get_request_allocation_size;
use crate::header::Header;
use crate::log::Log;
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};

pub struct Server {}
impl Server {
    pub fn process_request(mut stream: impl Read + Write + Unpin, peer_addr: SocketAddr) -> Vec<u8> {
        let request_allocation_size = get_request_allocation_size();
        let mut buffer = vec![0; request_allocation_size as usize];
        let boxed_read = stream.read(&mut buffer);
        if boxed_read.is_err() {
            let message = boxed_read.err().unwrap().to_string();
            eprintln!("unable to read TCP stream {}", &message);

            let raw_response = Server::bad_request_response(message);
            let boxed_stream = stream.write(raw_response.borrow());
            if boxed_stream.is_ok() {
                stream.flush().unwrap();
            };
            return raw_response;
        }

        boxed_read.unwrap();
        let request : &[u8] = &buffer;

        // let raw_request = String::from_utf8(Vec::from(request)).unwrap();
        // println!("\n\n______{}______\n\n", raw_request);


        let boxed_request = Request::parse_request(request);
        if boxed_request.is_err() {
            let message = boxed_request.err().unwrap();
            eprintln!("unable to parse request: {}", &message);

            let raw_response = Server::bad_request_response(message);
            let boxed_stream = stream.write(raw_response.borrow());
            if boxed_stream.is_ok() {
                stream.flush().unwrap();
            };
            return raw_response;
        }


        let request: Request = boxed_request.unwrap();
        let (response, request) = App::handle_request(request);


        let log_request_response = Log::request_response(&request, &response, &peer_addr);
        println!("{}", log_request_response);
        let raw_response = Response::generate_response(response, request);

        let boxed_stream = stream.write(raw_response.borrow());
        if boxed_stream.is_ok() {
            stream.flush().unwrap();
        };

        raw_response
    }

    pub fn bad_request_response(message: String) -> Vec<u8> {
        let error_request = Request {
            method: METHOD.get.to_string(),
            request_uri: "".to_string(),
            http_version: "".to_string(),
            headers: vec![],
            body: vec![],
        };

        let size = message.chars().count() as u64;
        let content_range = ContentRange {
            unit: Range::BYTES.to_string(),
            range: Range { start: 0, end: size },
            size: size.to_string(),
            body: Vec::from(message.as_bytes()),
            content_type: MimeType::TEXT_PLAIN.to_string(),
        };

        let header_list = Header::get_header_list(&error_request);
        let error_response: Response = Response::get_response(
            STATUS_CODE_REASON_PHRASE.n400_bad_request,
            Some(header_list),
            Some(vec![content_range])
        );

        let response = Response::generate_response(error_response, error_request);
        return response;
    }

}
