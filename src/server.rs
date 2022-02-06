

use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::{env, fs};

use crate::request::Request;
use crate::response::Response;


pub struct Server {
    pub(crate) ip_addr: String,
    pub(crate) port: i32,
}

pub trait HandleConnection {
    fn handle_connection(&self, s: TcpStream);
}

pub trait ProcessRequest {
    fn process_request(&self, request_string: String) -> String;
}

impl HandleConnection for Server {
    fn handle_connection(&self, s: TcpStream) {
        let mut buffer = [0; 1024];

        let mut stream = s;

        stream.read(&mut buffer).unwrap();

        let request_string = String::from_utf8_lossy(&buffer[..]).to_string();
        println!("{}", request_string);

        let response = self.process_request(request_string);
        println!("{}", response);

        stream.write(response.as_bytes()).unwrap();
        stream.flush().unwrap();

    }
}

impl ProcessRequest for Server {
    fn process_request(&self, request_string: String) -> String {
        let request: Request = Request::parse_request(&request_string);

        println!("{}" , request);
        for header in request.headers {
            println!("{}" , header);
        }

        let is_get = request.method == "GET";
        let is_static_content_read_attempt = request.request_uri.starts_with("/static/");


        // by default we assume route or static asset is not found
        let contents = fs::read_to_string("404.html").unwrap();
        let mut response = Response::generate_response("HTTP/1.1 404 NOT FOUND".to_string(), &contents);

        if  is_get && is_static_content_read_attempt {
            let dir = env::current_dir().unwrap();
            let working_directory = dir.as_path().to_str().unwrap();
            let static_filepath = [working_directory, request.request_uri.as_str()].join("");

            let unwrapped_contents = fs::read_to_string(static_filepath);

            let contents = match unwrapped_contents {
                Ok(file) => file,
                Err(error) => {
                    error.to_string()
                },
            };

            if !contents.starts_with("No such file or directory") {
                response = Response::generate_response("HTTP/1.1 200 OK".to_string(), &contents);
            }

        }

        if request.request_uri == "/" {
            let contents = fs::read_to_string("index.html").unwrap();
            response = Response::generate_response("HTTP/1.1 200 OK".to_string(), &contents);
        }

        response
    }
}


