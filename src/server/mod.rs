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
        use crate::http::VERSION;

        let request_allocation_size = connection.request_size;
        let client = connection.client.clone();
        let client_addr = SocketAddr::new(IpAddr::from_str(client.ip.as_str()).unwrap(), client.port as u16);

        loop {
            let mut buffer = vec![0; request_allocation_size as usize];
            let boxed_read = stream.read(&mut buffer);
            if boxed_read.is_err() {
                // timeout or client closed — normal end of keep-alive session
                break;
            }
            if boxed_read.unwrap() == 0 {
                break;
            }

            let request = match Request::parse(&buffer) {
                Ok(r) => r,
                Err(message) => {
                    let raw_response = Server::bad_request_response(message.clone());
                    let boxed_stream = stream.write(raw_response.borrow());
                    if boxed_stream.is_ok() { stream.flush().unwrap(); }
                    return Err(message);
                }
            };

            let keep_alive = {
                let conn_hdr = request.get_header(Header::_CONNECTION.to_string());
                match conn_hdr {
                    Some(h) => h.value.to_lowercase() != "close",
                    None => request.http_version == VERSION.http_1_1,
                }
            };

            let mut response = match app.execute(&request, &connection) {
                Ok(r) => r,
                Err(message) => {
                    let raw_response = Server::bad_request_response(message.clone());
                    let boxed_stream = stream.write(raw_response.borrow());
                    if boxed_stream.is_ok() { stream.flush().unwrap(); }
                    return Err(message);
                }
            };

            crate::metrics::record_request();
            crate::compression::apply_gzip(&request, &mut response);

            response.headers.push(Header {
                name: Header::_CONNECTION.to_string(),
                value: if keep_alive { "keep-alive".to_string() } else { "close".to_string() },
            });

            Log::log_access(&request, &response, &client_addr);

            if let Some(ref filepath) = response.stream_file.clone() {
                if let Err(e) = Server::write_chunked_file(&mut stream, response, request, filepath) {
                    return Err(e);
                }
            } else {
                let raw_response = Response::generate_response(response, request);
                if let Err(e) = stream.write(raw_response.borrow()) {
                    return Err(e.to_string());
                }
                stream.flush().unwrap();
            }

            if !keep_alive { break; }
        }

        Ok(())
    }

    /// Streams a file to `stream` using HTTP/1.1 chunked transfer encoding.
    /// The response headers are written first, then the file is read and written in 64 KB chunks.
    pub(crate) fn write_chunked_file(
        stream: &mut impl Write,
        mut response: Response,
        request: Request,
        filepath: &str,
    ) -> Result<(), String> {
        use std::fs::File;
        use std::io::Read as _;

        response.headers.push(Header {
            name: Header::_TRANSFER_ENCODING.to_string(),
            value: "chunked".to_string(),
        });

        // build status line + headers (no body)
        let status = [
            response.http_version.clone(),
            response.status_code.to_string(),
            response.reason_phrase.clone(),
        ].join(SYMBOL.whitespace);

        let mut headers_str = SYMBOL.new_line_carriage_return.to_string();
        for header in &response.headers {
            headers_str.push_str(&header.name);
            headers_str.push_str(Header::NAME_VALUE_SEPARATOR);
            headers_str.push_str(&header.value);
            headers_str.push_str(SYMBOL.new_line_carriage_return);
        }
        let head = format!("{}{}{}", status, headers_str, SYMBOL.new_line_carriage_return);

        stream.write_all(head.as_bytes()).map_err(|e| e.to_string())?;

        if request.method != METHOD.head && request.method != METHOD.options {
            let mut file = File::open(filepath).map_err(|e| e.to_string())?;
            let mut buf = vec![0u8; 65536];
            loop {
                let n = file.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 { break; }
                // chunk header: hex size + CRLF
                stream.write_all(format!("{:x}\r\n", n).as_bytes()).map_err(|e| e.to_string())?;
                stream.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                stream.write_all(b"\r\n").map_err(|e| e.to_string())?;
            }
            // terminal chunk
            stream.write_all(b"0\r\n\r\n").map_err(|e| e.to_string())?;
        }

        stream.flush().map_err(|e| e.to_string())
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
    ///
    /// When built with the `http1` feature, Ctrl+C and SIGTERM stop the accept
    /// loop gracefully: `SERVER_READY` is cleared and the pool drains all
    /// in-flight connections before returning.
    ///
    /// For TLS/HTTP2/HTTP3 use [`Server::run_tls`].
    pub fn run(listener: TcpListener,
               pool: ThreadPool,
               app: impl Application + New + Send + 'static + Copy) {
        #[cfg(feature = "http1")]
        {
            use std::sync::Arc;
            use std::sync::atomic::{AtomicBool, Ordering};

            let shutdown = Arc::new(AtomicBool::new(false));
            let s = shutdown.clone();
            if let Err(e) = ctrlc::set_handler(move || {
                s.store(true, Ordering::SeqCst);
            }) {
                eprintln!("unable to install signal handler: {}", e);
            }
            crate::config_reload::install_sighup_handler();
            if let Err(e) = listener.set_nonblocking(true) {
                eprintln!("unable to set non-blocking listener: {}", e);
            }

            loop {
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }
                if crate::config_reload::RELOAD_REQUESTED
                    .compare_exchange(true, false, Ordering::SeqCst, Ordering::Relaxed)
                    .is_ok()
                {
                    crate::config_reload::reload();
                }
                match listener.accept() {
                    Ok((stream, peer_addr)) => {
                        Server::dispatch_connection(stream, peer_addr, &pool, app);
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => {
                        eprintln!("accept error: {}", e);
                        break;
                    }
                }
            }

            crate::metrics::SERVER_READY.store(false, std::sync::atomic::Ordering::SeqCst);
            println!("Shutting down — waiting for in-flight connections to finish");
            pool.join();
            println!("Server stopped");
        }

        #[cfg(not(feature = "http1"))]
        {
            for boxed_stream in listener.incoming() {
                match boxed_stream {
                    Err(e) => {
                        eprintln!("unable to get TCP stream: {}", e);
                        return;
                    }
                    Ok(stream) => {
                        let peer_addr = match stream.peer_addr() {
                            Ok(a) => a,
                            Err(e) => {
                                eprintln!("unable to read peer addr: {}", e);
                                return;
                            }
                        };
                        Server::dispatch_connection(stream, peer_addr, &pool, app);
                    }
                }
            }
        }
    }

    fn dispatch_connection(
        stream: std::net::TcpStream,
        peer_addr: std::net::SocketAddr,
        pool: &ThreadPool,
        app: impl Application + New + Send + 'static + Copy,
    ) {
        print!("Connection established, ");
        if let Ok(local) = stream.local_addr() {
            print!("local addr: {}", local);
        }
        println!(", peer addr: {}", peer_addr);

        let (server_ip, server_port, _thread_count) = get_ip_port_thread_count();
        let connection = ConnectionInfo {
            client: Address {
                ip: peer_addr.ip().to_string(),
                port: peer_addr.port() as i32,
            },
            server: Address {
                ip: server_ip,
                port: server_port,
            },
            request_size: get_request_allocation_size(),
        };

        if let Err(e) = stream.set_read_timeout(Some(Duration::from_secs(30))) {
            eprintln!("failed to set read timeout: {}", e);
        }

        pool.execute(move || {
            crate::metrics::connection_open();
            let result = Server::process(stream, connection, app);
            crate::metrics::connection_close();
            if let Err(msg) = result {
                crate::metrics::record_error();
                eprintln!("{}", msg);
            }
        });
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

impl ConnectionInfo {
    /// Parse the client address into a [`std::net::SocketAddr`], if the stored
    /// IP and port are valid. Returns `None` if parsing fails.
    pub fn peer_addr(&self) -> Option<std::net::SocketAddr> {
        self.client.to_socket_addr()
    }
}

impl Address {
    /// Parse this address into a [`std::net::SocketAddr`]. Returns `None` if
    /// the IP string or port value cannot be converted.
    pub fn to_socket_addr(&self) -> Option<std::net::SocketAddr> {
        let ip: std::net::IpAddr = self.ip.parse().ok()?;
        let port = u16::try_from(self.port).ok()?;
        Some(std::net::SocketAddr::new(ip, port))
    }
}

/// Resolves when SIGTERM is received on Unix, or never on other platforms.
/// Enables a single `select!` branch to handle both SIGTERM and Ctrl+C.
#[cfg(feature = "http2")]
async fn sigterm() {
    #[cfg(unix)]
    {
        if let Ok(mut s) = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate()
        ) {
            s.recv().await;
        } else {
            std::future::pending::<()>().await
        }
    }
    #[cfg(not(unix))]
    std::future::pending::<()>().await
}

/// Returns a stream that fires on each SIGHUP on Unix; never fires elsewhere.
#[cfg(feature = "http2")]
async fn sighup() {
    #[cfg(unix)]
    {
        if let Ok(mut s) = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::hangup()
        ) {
            s.recv().await;
        } else {
            std::future::pending::<()>().await
        }
    }
    #[cfg(not(unix))]
    std::future::pending::<()>().await
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

        let mut tls_acceptor = match create_tls_acceptor(&cert_path, &key_path) {
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
                    crate::metrics::SERVER_READY.store(false, std::sync::atomic::Ordering::SeqCst);
                    println!("\nShutting down gracefully (SIGINT).");
                    break;
                }
                _ = sigterm() => {
                    crate::metrics::SERVER_READY.store(false, std::sync::atomic::Ordering::SeqCst);
                    println!("\nShutting down gracefully (SIGTERM).");
                    break;
                }
                _ = sighup() => {
                    crate::config_reload::reload();
                    // Reload TLS cert in place — picks up renewed certificates (e.g. from ACME).
                    if let Ok(new_acceptor) = create_tls_acceptor(&cert_path, &key_path) {
                        tls_acceptor = new_acceptor;
                        println!("[TLS] Certificate reloaded from '{}'.", cert_path);
                    }
                }
            }
        }
    }

    /// Binds a plain-HTTP listener on the port in `RWS_CONFIG_HTTP_REDIRECT_PORT` and sends
    /// `301 Moved Permanently` to the HTTPS equivalent of every incoming URL.
    /// Returns immediately if TLS is not configured or the redirect port is not set.
    pub async fn run_redirect() {
        use std::env;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener as TokioListener;

        let cert_path = env::var(crate::entry_point::Config::RWS_CONFIG_TLS_CERT_FILE)
            .unwrap_or_default();
        if cert_path.is_empty() {
            return;
        }

        let redirect_port_str = env::var(crate::entry_point::Config::RWS_CONFIG_HTTP_REDIRECT_PORT)
            .unwrap_or_default();
        if redirect_port_str.is_empty() {
            return;
        }

        let redirect_port: u16 = match redirect_port_str.parse() {
            Ok(p) => p,
            Err(_) => {
                eprintln!("Invalid RWS_CONFIG_HTTP_REDIRECT_PORT: {}", redirect_port_str);
                return;
            }
        };

        let (server_ip, server_port, _) = get_ip_port_thread_count();
        let bind_addr = format!("{}:{}", server_ip, redirect_port);

        let listener = match TokioListener::bind(&bind_addr).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("HTTP redirect listener error on {}: {}", bind_addr, e);
                return;
            }
        };

        println!("HTTP→HTTPS redirect listening on http://{}:{}", server_ip, redirect_port);

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((mut stream, _peer)) => {
                            let https_port = server_port;
                            tokio::spawn(async move {
                                let mut buf = vec![0u8; 4096];
                                let n = match stream.read(&mut buf).await {
                                    Ok(n) => n,
                                    Err(_) => return,
                                };
                                let text = String::from_utf8_lossy(&buf[..n]);

                                let uri = text.lines()
                                    .next()
                                    .and_then(|line| line.split_whitespace().nth(1))
                                    .unwrap_or("/")
                                    .to_string();

                                let host_header = text.lines()
                                    .find(|l| l.to_lowercase().starts_with("host:"))
                                    .map(|l| l[5..].trim().to_string());

                                let location = match host_header {
                                    Some(h) => {
                                        // strip existing port from Host header
                                        let h_no_port = if h.starts_with('[') {
                                            // IPv6: [::1] or [::1]:port
                                            h.find(']')
                                                .map(|i| h[..=i].to_string())
                                                .unwrap_or(h.clone())
                                        } else {
                                            h.rfind(':')
                                                .map(|i| h[..i].to_string())
                                                .unwrap_or(h.clone())
                                        };
                                        if https_port == 443 {
                                            format!("https://{}{}", h_no_port, uri)
                                        } else {
                                            format!("https://{}:{}{}", h_no_port, https_port, uri)
                                        }
                                    }
                                    None => format!("https://localhost:{}{}", https_port, uri),
                                };

                                let response = format!(
                                    "HTTP/1.1 301 Moved Permanently\r\nLocation: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                                    location
                                );
                                let _ = stream.write_all(response.as_bytes()).await;
                            });
                        }
                        Err(e) => eprintln!("HTTP redirect accept error: {}", e),
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    println!("\nShutting down HTTP redirect listener (SIGINT).");
                    break;
                }
                _ = sigterm() => {
                    println!("\nShutting down HTTP redirect listener (SIGTERM).");
                    break;
                }
                _ = sighup() => {
                    crate::config_reload::reload();
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

        crate::metrics::record_request();
        crate::compression::apply_gzip(&request, &mut response);
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

        Log::log_access(&request, &response, &peer_addr);

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
                    crate::metrics::SERVER_READY.store(false, std::sync::atomic::Ordering::SeqCst);
                    println!("\nShutting down QUIC (SIGINT).");
                    endpoint.close(0u32.into(), b"shutdown");
                    break;
                }
                _ = sigterm() => {
                    crate::metrics::SERVER_READY.store(false, std::sync::atomic::Ordering::SeqCst);
                    println!("\nShutting down QUIC (SIGTERM).");
                    endpoint.close(0u32.into(), b"shutdown");
                    break;
                }
                _ = sighup() => {
                    crate::config_reload::reload();
                }
            }
        }
    }
}


