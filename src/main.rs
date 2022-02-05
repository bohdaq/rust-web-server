mod header;
mod request;
mod response;

extern crate core;

use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::{env, fs};

use crate::header::Header;
use crate::request::Request;
use crate::response::Response;


fn main() {
    let args: Vec<String> = env::args().collect();
    println!("{:?}", args);

    let mut port = 7878;
    if args.len() >= 2 {
        port = (&args[1]).parse().unwrap();
    }

    let mut ip_addr = "127.0.0.1";
    if args.len() >= 3 {
        ip_addr = args[2].as_str();
    }

    let bind_addr = [ip_addr, ":", &port.to_string()].join("");

    println!("Hello, rust-web-server!");

    let listener = TcpListener::bind(bind_addr).unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        println!("Connection established!");
        handle_connection(stream);
    }
}

fn handle_connection(mut stream: TcpStream) {
    let mut buffer = [0; 1024];

    stream.read(&mut buffer).unwrap();

    let request_string = String::from_utf8_lossy(&buffer[..]).to_string();
    println!("{}", request_string);

    let response = process_request(request_string);
    println!("{}", response);

    stream.write(response.as_bytes()).unwrap();
    stream.flush().unwrap();

}

fn process_request(request_string: String) -> String {
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





#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_generates_successful_response_with_index_html() {
        // request test data
        let request_host_header_name = "Host";
        let request_host_header_value = "localhost:7777";
        let request_method = "GET";
        let request_uri = "/";
        let request_http_version = "HTTP/1.1";


        // request part
        let host = Header {
            header_name: request_host_header_name.to_string(),
            header_value: request_host_header_value.to_string()
        };

        let headers = vec![host];
        let request = Request {
            method: request_method.to_string(),
            request_uri: request_uri.to_string(),
            http_version: request_http_version.to_string(),
            headers
        };

        let raw_request = Request::generate_request(request);

        let request: Request = Request::parse_request(&raw_request);
        let host_header = request.get_header(request_host_header_name.to_string()).unwrap();

        assert_eq!(request_host_header_value.to_string(), host_header.header_value);
        assert_eq!(request_method.to_string(), request.method);
        assert_eq!(request_uri.to_string(), request.request_uri);
        assert_eq!(request_http_version.to_string(), request.http_version);

        // response part
        let response_http_version = "HTTP/1.1";
        let response_status_code = "200";
        let response_reason_phrase = "OK";
        let response_filepath = "index.html";
        let response_html_file= fs::read_to_string(response_filepath.to_string()).unwrap();
        let response_content_length_header_name = "Content-Length";
        let response_content_length_header_value = response_html_file.len().to_string();

        let raw_response: String = process_request(raw_request);
        let response = Response::parse_response(raw_response);
        let header = response.get_header(response_content_length_header_name.to_string()).unwrap();

        assert_eq!(response_content_length_header_value, header.header_value);
        assert_eq!(response_http_version, response.http_version);
        assert_eq!(response_status_code, response.status_code);
        assert_eq!(response_reason_phrase, response.reason_phrase);
        assert_eq!(response_html_file, response.message_body);
    }
}
