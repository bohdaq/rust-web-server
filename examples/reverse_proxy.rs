//! A reverse proxy in code: `/api/*` goes to a backend, everything else is
//! served locally. (For a proxy configured in `rws.config.toml` instead, see
//! the docs' "Config-driven proxy".)
//!
//! ```sh
//! cargo run --example reverse_proxy      # starts its own backend on :9091
//! curl localhost:7878/api/time
//! ```

use std::net::TcpListener;

use rust_web_server::middleware::WithMiddleware;
use rust_web_server::prelude::*;
use rust_web_server::proxy::ReverseProxy;
use rust_web_server::state::AppWithState;

/// The backend: a separate app on its own port.
pub fn backend() -> AppWithState<()> {
    App::with_state(()).get("/api/time", |_, _, _, _| {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, &format!("backend says {secs}"))
    })
}

/// The front app: `/api/*` proxied to `backend_url`, the rest served here.
pub fn front(backend_url: &str) -> WithMiddleware<AppWithState<()>> {
    App::with_state(())
        .get("/", |_, _, _, _| Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "front"))
        .wrap(ReverseProxy::new([backend_url]).path_prefix("/api"))
}

#[tokio::main]
async fn main() {
    let backend_listener = TcpListener::bind("127.0.0.1:9091").unwrap();
    tokio::spawn(Server::run_plain(backend_listener, backend()));
    let (listener, pool) = Server::setup().unwrap();
    Server::run_tls(listener, pool, front("http://127.0.0.1:9091")).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_web_server::test_client::TestClient;

    #[test]
    fn api_requests_reach_the_backend() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            tokio::runtime::Runtime::new().unwrap().block_on(Server::run_plain(listener, backend()));
        });
        let client = TestClient::new(front(&url));
        let mut proxied = String::new();
        for _ in 0..50 {
            proxied = client.get("/api/time").send().body_text().to_string();
            if proxied.starts_with("backend says") {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(proxied.starts_with("backend says"), "{proxied}");
        assert_eq!("front", client.get("/").send().body_text());
    }
}
