#[cfg(test)]
pub mod tests;
#[cfg(test)]
mod example;

use std::io::prelude::*;
use std::borrow::Borrow;
use std::net::{IpAddr, SocketAddr, TcpListener};
use std::str::FromStr;
use std::time::Duration;

use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::app::App;
use crate::application::Application;
use crate::core::{New};
use crate::entry_point::{bootstrap, get_ip_port_thread_count, get_request_allocation_size, set_default_values};
use crate::header::Header;
use crate::log::Log;
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};
use crate::symbol::SYMBOL;
use crate::thread_pool::ThreadPool;

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


        let log_request_response = Log::combined(&request, &response, &peer_addr);
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
                   app: impl Application) -> Result<(), String> {

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

        let app_processing = app.execute(&request, &connection);
        if app_processing.is_err() {
            let message = app_processing.as_ref().err().unwrap().to_string();
            let response = Server::bad_request_response(message);

            let boxed_stream = stream.write(response.borrow());
            if boxed_stream.is_ok() {
                stream.flush().unwrap();
            } else {
                let write_message = boxed_stream.err().unwrap().to_string();
                return Err(write_message);
            };
        }
        let response = app_processing.unwrap();


        let client = connection.client;
        let client_addr = SocketAddr::new(IpAddr::from_str(client.ip.as_str()).unwrap(), client.port as u16);
        let log_request_response = Log::combined(&request, &response, &client_addr);
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

    /// Reads configuration (IP, port, thread count, TLS paths) from the layered config system
    /// and returns a bound `TcpListener` and a sized `ThreadPool`. Call once at startup.
    pub fn setup() -> Result<(TcpListener, ThreadPool), String> {
        let info = Log::info("Rust Web Server");
        println!("{}", info);

        let usage_info = Log::usage_information();
        println!("{}", usage_info);


        println!("RWS Configuration Start: \n");

        set_default_values();
        bootstrap();

        println!("\nRWS Configuration End\n\n");


        let (ip, port, thread_count) = get_ip_port_thread_count();


        let mut ip_readable = ip.to_string();

        if ip.contains(":") {
            ip_readable = [SYMBOL.opening_square_bracket, &ip, SYMBOL.closing_square_bracket].join("");
        }

        let bind_addr = [ip_readable, SYMBOL.colon.to_string(), port.to_string()].join(SYMBOL.empty_string);

        #[cfg(feature = "http2")]
        let protocol = {
            let cert = std::env::var(crate::entry_point::Config::RWS_CONFIG_TLS_CERT_FILE).unwrap_or_default();
            if cert.is_empty() { "http" } else { "https" }
        };
        #[cfg(not(feature = "http2"))]
        let protocol = "http";

        println!("Setting up {}://{}...", protocol, &bind_addr);

        let boxed_listener = TcpListener::bind(&bind_addr);
        if boxed_listener.is_err() {
            let message = format!("unable to set up TCP listener: {}", boxed_listener.err().unwrap());
            return Err(message);
        }

        let listener = boxed_listener.unwrap();
        let pool = ThreadPool::new(thread_count as usize);


        let server_url_thread_count = Log::server_url_thread_count(protocol, &bind_addr, thread_count);
        println!("{}", server_url_thread_count);

        Ok((listener, pool))
    }

    /// Accepts TCP connections in a loop and dispatches each to the thread pool.
    /// Blocks forever (plain HTTP/1.1). For TLS/HTTP2/HTTP3 use [`Server::run_tls`].
    pub fn run(listener : TcpListener,
               pool: ThreadPool,
               app: impl Application + New + Send + 'static + Copy) {
        for boxed_stream in listener.incoming() {
            if boxed_stream.is_err() {
                eprintln!("unable to get TCP stream: {}", boxed_stream.err().unwrap());
                return;
            }

            let stream = boxed_stream.unwrap();

            print!("Connection established, ");

            let boxed_local_addr = stream.local_addr();
            if boxed_local_addr.is_ok() {
                print!("local addr: {}", boxed_local_addr.unwrap())
            } else {
                eprintln!("\nunable to read local addr");
                return;
            }

            let boxed_peer_addr = stream.peer_addr();
            if boxed_peer_addr.is_err() {
                eprintln!("\nunable to read peer addr");
                return;
            }
            let peer_addr = boxed_peer_addr.unwrap();
            print!(", peer addr: {}\n", peer_addr.to_string());

            let (server_ip, server_port, _thread_count) = get_ip_port_thread_count();
            let client_ip = peer_addr.ip().to_string();
            let client_port = peer_addr.port() as i32;
            let request_allocation_size = get_request_allocation_size();

            let connection = ConnectionInfo {
                client: Address {
                    ip: client_ip.to_string(),
                    port: client_port
                },
                server: Address {
                    ip: server_ip,
                    port: server_port
                },
                request_size: request_allocation_size,
            };



            if let Err(e) = stream.set_read_timeout(Some(Duration::from_secs(30))) {
                eprintln!("failed to set read timeout: {}", e);
            }

            pool.execute(move || {
                let boxed_process = Server::process(stream, connection, app);
                if boxed_process.is_err() {
                    let message = boxed_process.err().unwrap();
                    eprintln!("{}", message);
                }
            });

        }


    }

}

/// Network context for the current connection, passed into every [`Controller`](crate::controller::Controller).
#[derive(Clone)]
pub struct ConnectionInfo {
    /// Client (peer) address.
    pub client: Address,
    /// Server (local) address.
    pub server: Address,
    /// Bytes allocated for reading the request.
    pub request_size: i64
}

/// IP address and port pair.
#[derive(Clone)]
pub struct Address {
    pub ip: String,
    pub port: i32
}

#[cfg(feature = "http2")]
impl Server {
    pub async fn run_tls(
        listener: TcpListener,
        pool: ThreadPool,
        app: impl Application + New + Send + 'static + Copy,
    ) {
        use crate::tls::create_tls_acceptor;
        use crate::h2_handler;

        let cert_path = std::env::var(crate::entry_point::Config::RWS_CONFIG_TLS_CERT_FILE)
            .unwrap_or_default();
        let key_path = std::env::var(crate::entry_point::Config::RWS_CONFIG_TLS_KEY_FILE)
            .unwrap_or_default();

        if cert_path.is_empty() || key_path.is_empty() {
            println!("No TLS certificate configured — serving plain HTTP/1.1.");
            tokio::task::block_in_place(|| Server::run(listener, pool, app));
            return;
        }

        let tls_acceptor = match create_tls_acceptor(&cert_path, &key_path) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("TLS setup failed: {}", e);
                return;
            }
        };

        listener
            .set_nonblocking(true)
            .expect("failed to set TCP listener to non-blocking");
        let tokio_listener = tokio::net::TcpListener::from_std(listener)
            .expect("failed to convert TCP listener to tokio");

        println!("Listening for TLS connections (HTTP/1.1 + HTTP/2)...");

        loop {
            tokio::select! {
                result = tokio_listener.accept() => {
                    match result {
                        Ok((tcp_stream, peer_addr)) => {
                            let acceptor = tls_acceptor.clone();
                            tokio::spawn(async move {
                                match acceptor.accept(tcp_stream).await {
                                    Ok(tls_stream) => {
                                        let protocol = tls_stream
                                            .get_ref()
                                            .1
                                            .alpn_protocol()
                                            .map(|p| p.to_vec());

                                        match protocol.as_deref() {
                                            Some(b"h2") => {
                                                if let Err(e) =
                                                    h2_handler::handle_connection(tls_stream, peer_addr, app)
                                                        .await
                                                {
                                                    eprintln!("H2 connection error: {}", e);
                                                }
                                            }
                                            _ => {
                                                if let Err(e) =
                                                    Server::process_h1_tls(tls_stream, peer_addr, app).await
                                                {
                                                    eprintln!("H1 TLS error: {}", e);
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => eprintln!("TLS handshake failed: {}", e),
                                }
                            });
                        }
                        Err(e) => eprintln!("TCP accept error: {}", e),
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    println!("\nShutting down gracefully.");
                    break;
                }
            }
        }
    }

    async fn process_h1_tls(
        mut stream: tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
        peer_addr: std::net::SocketAddr,
        app: impl Application,
    ) -> Result<(), String> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let (server_ip, server_port, _) = get_ip_port_thread_count();
        let request_allocation_size = get_request_allocation_size();

        let mut buffer = vec![0u8; request_allocation_size as usize];
        if let Err(e) = stream.read(&mut buffer).await {
            let raw = Server::bad_request_response(e.to_string());
            let _ = stream.write_all(&raw).await;
            return Ok(());
        }

        let request = match Request::parse(&buffer) {
            Ok(r) => r,
            Err(message) => {
                let raw = Server::bad_request_response(message);
                let _ = stream.write_all(&raw).await;
                return Ok(());
            }
        };

        let connection = ConnectionInfo {
            client: Address {
                ip: peer_addr.ip().to_string(),
                port: peer_addr.port() as i32,
            },
            server: Address {
                ip: server_ip,
                port: server_port,
            },
            request_size: request_allocation_size,
        };

        let mut response = match app.execute(&request, &connection) {
            Ok(r) => r,
            Err(message) => {
                let raw = Server::bad_request_response(message);
                let _ = stream.write_all(&raw).await;
                return Ok(());
            }
        };

        response.headers.push(Header::get_hsts_header());

        #[cfg(feature = "http3")]
        response.headers.push(Header {
            name: Header::_ALT_SVC.to_string(),
            value: format!("h3=\":{}\"", server_port),
        });
        #[cfg(not(feature = "http3"))]
        response.headers.push(Header {
            name: Header::_ALT_SVC.to_string(),
            value: format!("h2=\":{}\"", server_port),
        });

        let log = Log::combined(&request, &response, &peer_addr);
        println!("{}", log);

        let raw = Response::generate_response(response, request);
        stream
            .write_all(&raw)
            .await
            .map_err(|e| e.to_string())?;
        stream.flush().await.map_err(|e| e.to_string())?;

        Ok(())
    }
}

#[cfg(feature = "http3")]
impl Server {
    pub async fn run_quic(
        app: impl Application + New + Send + 'static + Copy,
    ) {
        use crate::tls::create_quinn_server_config;
        use crate::h3_handler;

        let cert_path = std::env::var(crate::entry_point::Config::RWS_CONFIG_TLS_CERT_FILE)
            .unwrap_or_default();
        let key_path = std::env::var(crate::entry_point::Config::RWS_CONFIG_TLS_KEY_FILE)
            .unwrap_or_default();

        if cert_path.is_empty() || key_path.is_empty() {
            return;
        }

        let server_config = match create_quinn_server_config(&cert_path, &key_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("QUIC TLS setup failed: {}", e);
                return;
            }
        };

        let (server_ip, server_port, _) = get_ip_port_thread_count();
        let bind_addr = format!("{}:{}", server_ip, server_port);
        let addr: std::net::SocketAddr = match bind_addr.parse() {
            Ok(a) => a,
            Err(e) => {
                eprintln!("Invalid QUIC bind address '{}': {}", bind_addr, e);
                return;
            }
        };

        let endpoint = match quinn::Endpoint::server(server_config, addr) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("QUIC endpoint error: {}", e);
                return;
            }
        };

        println!("Listening for QUIC/HTTP3 on UDP {}:{}", server_ip, server_port);

        loop {
            tokio::select! {
                maybe = endpoint.accept() => {
                    match maybe {
                        Some(incoming) => {
                            tokio::spawn(async move {
                                match incoming.await {
                                    Ok(conn) => {
                                        let peer_addr = conn.remote_address();
                                        if let Err(e) = h3_handler::handle_connection(conn, peer_addr, app).await {
                                            eprintln!("H3 connection error: {}", e);
                                        }
                                    }
                                    Err(e) => eprintln!("QUIC connection error: {}", e),
                                }
                            });
                        }
                        None => break,
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    println!("\nShutting down QUIC.");
                    endpoint.close(0u32.into(), b"shutdown");
                    break;
                }
            }
        }
    }
}


