use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::{env, fs};

use crate::request::Request;
use crate::response::Response;
use crate::app::App;
use crate::Config;


pub struct Server {}


impl Server {
    pub(crate) fn handle_connection(s: TcpStream, config: Config) {
        let mut buffer = [0; 1024];

        let mut stream = s;

        stream.read(&mut buffer).unwrap();

        let request_string = String::from_utf8_lossy(&buffer[..]).to_string();

        let response = Server::process_request(request_string, config);

        stream.write(response.as_bytes()).unwrap();
        stream.flush().unwrap();

    }

    pub(crate) fn process_request(request_string: String, config: Config) -> String {
        let request: Request = Request::parse_request(&request_string);

        let is_get = request.method == "GET";

        let mut static_directories = vec![];
        let static_directories_vec_str: Vec<&str> = config.static_dirs.split(",").collect();
        for dir in &static_directories_vec_str {
            &static_directories.push(dir.to_string());
        }

        let mut is_static_content_read_attempt = false;
        for static_dir in static_directories {
            if request.request_uri.starts_with(&static_dir) {
                is_static_content_read_attempt = true;
            }
        }

        // by default we assume route or static asset is not found
        let contents = fs::read_to_string("404.html").unwrap();
        let response = Response {
            http_version: "HTTP/1.1".to_string(),
            status_code: "404".to_string(),
            reason_phrase: "NOT FOUND".to_string(),
            headers: vec![],
            message_body: contents
        };
        let mut raw_response = Response::generate_response(response);

        if  is_get && is_static_content_read_attempt {
            let dir = env::current_dir().unwrap();
            let working_directory = dir.as_path().to_str().unwrap();
            let static_filepath = [working_directory, request.request_uri.as_str()].join("");

            let unwrapped_contents = fs::read_to_string(static_filepath);

            let mut is_content_readable = true;
            let contents = match unwrapped_contents {
                Ok(file) => file,
                Err(error) => {
                    // 'No such file or directory' or 'Is a directory' errors etc.
                    is_content_readable = false;
                    error.to_string()
                },
            };

            if is_content_readable {
                let response = Response {
                    http_version: "HTTP/1.1".to_string(),
                    status_code: "200".to_string(),
                    reason_phrase: "OK".to_string(),
                    headers: vec![],
                    message_body: contents
                };
                raw_response = Response::generate_response(response);
            }

        }

        if !is_static_content_read_attempt {
            let response = App::handle_request(request);
            raw_response = Response::generate_response(response);
        }

        raw_response
    }
}


