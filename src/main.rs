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
    println!("Request: {}", request_string);

    let method_request_uri_http_version = get_method_request_uri_http_version(&request_string);
    println!("method_request_uri_http_version: {}", method_request_uri_http_version);

    let path = get_path_from_method_request_uri_http_version(&method_request_uri_http_version).unwrap();
    println!("path: {}" , path);



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


fn get_method_request_uri_http_version(request: &str) -> String {
    let strings: Vec<&str> = request.split("\n").collect();
    strings[0].to_string()
}

fn get_path_from_method_request_uri_http_version(method_request_uri_http_version: &str) -> Result<String, String> {
    let strings: Vec<&str> = method_request_uri_http_version.split(" ").collect();
    let has_path_value = !strings.is_empty() && strings.len() > 2;
    if has_path_value {
        Ok(strings[1].to_string())
    } else {
        Err("unable to parse path".to_string())
    }
}
