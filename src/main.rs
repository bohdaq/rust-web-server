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
            response = generate_response("HTTP/1.1 404 NOT FOUND".to_string(), &contents);
        } else {
            response = generate_response("HTTP/1.1 200 OK".to_string(), &contents);
        }


    }

    if request.request_uri == "/" {
        let contents = fs::read_to_string("index.html").unwrap();
        response = generate_response("HTTP/1.1 200 OK".to_string(), &contents);
    }

    println!("{}", response);
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

struct Response {
    http_version: String,
    status_code: String,
    reason_phrase: String,
    headers: Vec<Header>,
    message_body: String
}

impl std::fmt::Display for Response {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "Response http version {} and status_code {} and reason_phrase {}", self.http_version, self.status_code, self.reason_phrase)
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
    let strings: Vec<&str> = request.split("\r\n").collect();

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

fn parse_response(response: String) -> Response {
    let strings: Vec<&str> = response.split("\r\n").collect();

    // parsing http_version, status_code and reason phrase
    let http_version_status_code_reason_phrase = strings[0].to_string();
    let split_http_version_status_code_reason_phrase: Vec<&str> = http_version_status_code_reason_phrase.split(" ").collect();

    let http_version = split_http_version_status_code_reason_phrase[0].to_string();
    let status_code = split_http_version_status_code_reason_phrase[1].to_string();
    let reason_phrase = split_http_version_status_code_reason_phrase[2].to_string();

    // parsing headers
    let mut headers = vec![];
    let mut headers_end_position = 999999;
    for (pos, e) in strings.iter().enumerate() {
        // stop when headers end
        if e.len() == 1 {
            headers_end_position = pos;
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

    let mut message_body = "".to_string();
    // parsing message body
    for (pos, e) in strings.iter().enumerate() {
        // start when headers end
        if pos > headers_end_position {
            message_body.push_str(e);
        }
    }

    Response {
        http_version,
        status_code,
        reason_phrase,
        headers,
        message_body,
    }
}

fn generate_response(status: String, contents: &String) -> String {
    let response = format!(
        "{}\r\nContent-Length: {}\r\n\r\n{}",
        status,
        contents.len(),
        contents
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_generates_successful_response_with_index_html() {
        let html_file= fs::read_to_string("index.html").unwrap();
        let content_length = html_file.len();

        let http_version = "HTTP/1.1";
        let status_code = "200";
        let reason_phrase = "OK";
        let http_version_status_code_reason_phrase = [http_version, status_code, reason_phrase].join(" ").to_string();

        let raw_response =
            generate_response(http_version_status_code_reason_phrase, &html_file);

        let response = parse_response(raw_response);
        println!("{}", response);
        for header in response.headers {
            println!("{}" , header);
        }
        println!("{}", response.message_body);

        assert_eq!(http_version, response.http_version);
        assert_eq!(status_code, response.status_code);
        assert_eq!(reason_phrase, response.reason_phrase);

        assert_eq!(html_file, response.message_body);

    }
}
