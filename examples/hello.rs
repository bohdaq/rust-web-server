//! The smallest rws app: two routes, a path parameter, plain-HTTP serving.
//!
//! ```sh
//! cargo run --example hello
//! curl localhost:7878/hello/world      # Hello, world!
//! ```

use rust_web_server::prelude::*;
use rust_web_server::state::AppWithState;

fn hello(_: &Request, params: &PathParams, _: &ConnectionInfo, _: &()) -> Response {
    let name = params.get("name").unwrap_or("there");
    Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, &format!("Hello, {name}!"))
}

/// The app: everything not routed here (e.g. `/healthz`, static files)
/// falls through to rws's built-ins.
pub fn app() -> AppWithState<()> {
    App::with_state(())
        .get("/", |_, _, _, _| Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "rws is running"))
        .get("/hello/:name", hello)
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
    fn routes_answer() {
        let client = TestClient::new(app());
        assert_eq!("Hello, world!", client.get("/hello/world").send().body_text());
        assert_eq!("rws is running", client.get("/").send().body_text());
        assert_eq!(200, client.get("/healthz").send().status());
    }
}
