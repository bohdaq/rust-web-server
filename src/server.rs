

use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::{env, fs};

use crate::request::Request;
use crate::response::Response;


pub struct Server {
    pub(crate) bind_addr: String,
    pub(crate) port: i32,
}

impl Server {
    pub(crate) fn handle_connection(mut stream: TcpStream) {
        let mut buffer = [0; 1024];

        stream.read(&mut buffer).unwrap();

        let request_string = String::from_utf8_lossy(&buffer[..]).to_string();
        println!("{}", request_string);

        let response = Server::process_request(request_string);
        println!("{}", response);

        stream.write(response.as_bytes()).unwrap();
        stream.flush().unwrap();

    }

    pub(crate) fn process_request(request_string: String) -> String {
        let request: Request = Request::parse_request(&request_string);

        println!("{}" , request);
        for header in request.headers {
            println!("{}" , header);
        }

        let is_get = request.method == "GET";
        let is_static_content_read_attempt = request.request_uri.starts_with("/static/");

        let mut response = String::from("");
        if  is_get && is_static_content_read_attempt {
            println!("is_get: {} is_static_content_read_attempt: {}", is_get, is_static_content_read_attempt);

            let dir = env::current_dir().unwrap();
            let working_directory = dir.as_path().to_str().unwrap();
            println!("working directory: {}", working_directory);

            let static_filepath = [working_directory, request.request_uri.as_str()].join("");
            println!("filepath: {}", static_filepath);

            let unwrapped_contents = fs::read_to_string(static_filepath);

            let contents = match unwrapped_contents {
                Ok(file) => file,
                Err(error) => {
                    error.to_string()
                },
            };

            if contents.starts_with("No such file or directory") {
                let contents = fs::read_to_string("404.html").unwrap();
                response = Response::generate_response("HTTP/1.1 404 NOT FOUND".to_string(), &contents);
            } else {
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



