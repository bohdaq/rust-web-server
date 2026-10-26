//! Serve the working directory as a static site, with a request id on every
//! response and the global rate limit.
//!
//! ```sh
//! cd my-site && cargo run --manifest-path /path/to/rust-web-server/Cargo.toml --example static_site
//! curl -i localhost:7878/index.html      # x-request-id: ...
//! ```
//!
//! Files are served from the working directory with ETags, ranges,
//! compression (gzip, br, zstd) and `If-None-Match` revalidation.

use rust_web_server::core::New;
use rust_web_server::middleware::{RateLimitLayer, WithMiddleware};
use rust_web_server::prelude::*;
use rust_web_server::request_id::RequestIdLayer;

pub fn app() -> WithMiddleware<App> {
    App::new().wrap(RequestIdLayer::new()).wrap(RateLimitLayer)
}

#[tokio::main]
async fn main() {
    let (listener, pool) = Server::setup().unwrap();
    Server::run_tls(listener, pool, app()).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_web_server::test_client::TestClient;

    #[test]
    fn files_are_served_with_a_request_id() {
        let client = TestClient::new(app());
        // The repository's own index.html (tests run from the crate root).
        let response = client.get("/index.html").send();
        assert_eq!(200, response.status());
        assert!(response.header("x-request-id").is_some());
        assert!(response.header("etag").is_some());
        assert_eq!(404, client.get("/no-such-file.html").send().status());
    }
}
