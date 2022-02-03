use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::{env, fs};

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

    let request: Request = parse_request(request_string);

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
            response = format!(
                "{}\r\nContent-Length: {}\r\n\r\n{}",
                "HTTP/1.1 404 NOT FOUND",
                contents.len(),
                contents
            );
        } else {
            response = format!(
                "{}\r\nContent-Length: {}\r\n\r\n{}",
                "HTTP/1.1 200 OK",
                contents.len(),
                contents
            );
        }


    }

    if request.request_uri == "/" {
        let contents = fs::read_to_string("index.html").unwrap();

        response = format!(
            "{}\r\nContent-Length: {}\r\n\r\n{}",
            "HTTP/1.1 200 OK",
            contents.len(),
            contents
        );
    }

    stream.write(response.as_bytes()).unwrap();
    stream.flush().unwrap();

}

struct Request {
    method: String,
    request_uri: String,
    http_version: String,
    headers: Vec<Header>,
}

impl std::fmt::Display for Request {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "Request method {} and request uri {} and http_version {}", self.method, self.request_uri, self.http_version)
    }
}

struct Header {
    header_name: String,
    header_value: String,
}

impl std::fmt::Display for Header {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "Header name {} and value {}", self.header_name, self.header_value)
    }
}

fn parse_request(request: String) ->  Request {
    let strings: Vec<&str> = request.split("\n").collect();

    // parsing method request_uri and http_version
    let method_request_uri_http_version = strings[0].to_string();
    let split_method_request_uri_http_version: Vec<&str> = method_request_uri_http_version.split(" ").collect();

    let method = split_method_request_uri_http_version[0];
    let request_uri = split_method_request_uri_http_version[1];
    let http_version = split_method_request_uri_http_version[2];


    let mut headers = vec![];
    // parsing headers
    for (pos, e) in strings.iter().enumerate() {
        // stop when headers end
        if e.len() == 1 {
            break;
        }

        // skip method_request_uri_http_version
        if pos != 0  {
            let header_parts: Vec<&str> = e.split(": ").collect();

            let header = Header {
                header_name: header_parts[0].to_string(),
                header_value: header_parts[1].to_string()
            };

            headers.push(header);

        }
    }

    Request {
        method: method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: http_version.to_string(),
        headers,
    }
}
