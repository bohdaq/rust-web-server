use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::fs;

fn main() {
    println!("Hello, rust-web-server!");
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();

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
    println!("Raw Request: {}", request_string);

    let request: Request = parseRequest(request_string);
    println!("{}" , request);


    let get = b"GET / HTTP/1.1\r\n";

    let (status_line, filename) = if buffer.starts_with(get) {
        ("HTTP/1.1 200 OK", "index.html")
    } else {
        ("HTTP/1.1 404 NOT FOUND", "404.html")
    };

    let contents = fs::read_to_string(filename).unwrap();

    let response = format!(
        "{}\r\nContent-Length: {}\r\n\r\n{}",
        status_line,
        contents.len(),
        contents
    );

    stream.write(response.as_bytes()).unwrap();
    stream.flush().unwrap();
}

struct Request {
    method: String,
    request_uri: String,
    http_version: String,
}

impl std::fmt::Display for Request {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "HTTP request method {} and request uri {} and http_version {}", self.method, self.request_uri, self.http_version)
    }
}

fn parseRequest(request: String) ->  Request {
    let strings: Vec<&str> = request.split("\n").collect();


    let method_request_uri_http_version = strings[0].to_string();
    let strings: Vec<&str> = method_request_uri_http_version.split(" ").collect();

    let method = strings[0];
    let request_uri = strings[1];
    let http_version = strings[2];

    Request {
        method: method.to_string(),
        request_uri: request_uri.to_string(),
        http_version: http_version.to_string()
    }
}
