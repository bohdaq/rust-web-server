#[cfg(test)]
pub mod tests;

use std::io::prelude::*;
use std::borrow::Borrow;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::app::App;
use crate::core::{Application, New};
use crate::entry_point::{get_request_allocation_size};
use crate::header::Header;
use crate::log::Log;
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};
use crate::symbol::SYMBOL;

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

    pub fn process(mut stream: impl Read + Write + Unpin,
                   connection: ConnectionInfo,
                   app: impl Application + New) -> Result<(), String> {

        let request_allocation_size = connection.request_size;
        let mut buffer = vec![0; request_allocation_size as usize];
        let boxed_read = stream.read(&mut buffer);
        if boxed_read.is_err() {
            let read_message = boxed_read.err().unwrap().to_string();
            let raw_response = Server::bad_request_response(read_message.clone());
            let boxed_stream = stream.write(raw_response.borrow());
            if boxed_stream.is_ok() {
                stream.flush().unwrap();
            } else {
                let write_message = boxed_stream.err().unwrap().to_string();
                let combined_error = [read_message.clone(), SYMBOL.comma.to_string(), write_message].join(SYMBOL.empty_string);
                return Err(combined_error);
            };

            return Err(read_message);
        }

        boxed_read.unwrap();
        let request : &[u8] = &buffer;

        // let raw_request = String::from_utf8(Vec::from(request)).unwrap();
        // println!("\n\n______{}______\n\n", raw_request);


        let boxed_request = Request::parse(request);
        if boxed_request.is_err() {
            let message = boxed_request.err().unwrap();

            let raw_response = Server::bad_request_response(message.clone());
            let boxed_stream = stream.write(raw_response.borrow());
            if boxed_stream.is_ok() {
                stream.flush().unwrap();
            } else {
                let write_message = boxed_stream.err().unwrap().to_string();
                let combined_error = [message, SYMBOL.comma.to_string(), write_message].join(SYMBOL.empty_string);
                return Err(combined_error);
            };
            return Err(message);
        }


        let request: Request = boxed_request.unwrap();

        let response = app.execute(request.clone(), connection.clone()).unwrap();


        let client = connection.client;
        let client_addr = SocketAddr::new(IpAddr::from_str(client.ip.as_str()).unwrap(), client.port as u16);
        let log_request_response = Log::request_response(&request, &response, &client_addr);
        println!("{}", log_request_response);

        let raw_response = Response::generate_response(response, request);

        let boxed_stream = stream.write(raw_response.borrow());
        if boxed_stream.is_ok() {
            stream.flush().unwrap();
        } else {
            let write_message = boxed_stream.err().unwrap().to_string();
            return Err(write_message);
        };

        Ok(())
    }

}

#[derive(Clone)]
pub struct ConnectionInfo {
    pub client: Address,
    pub server: Address,
    pub request_size: i64
}

#[derive(Clone)]
pub struct Address {
    pub ip: String,
    pub port: i32
}


