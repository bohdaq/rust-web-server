//! Tower interop: a tonic gRPC service and tower-http layers in an rws app.
//!
//! ```sh
//! cargo run --example tower --features tower
//! curl -i localhost:7878/hello                                  # x-served-by from a tower-http layer
//! grpcurl -plaintext -proto health.proto localhost:7878 grpc.health.v1.Health/Check
//! ```
//!
//! `/grpc.health.v1.Health/` goes to tonic-health's generated service (gRPC
//! over HTTP/2: TLS, or cleartext h2c on the plain listener); everything else
//! to the rws routes, behind tower-http's `SetResponseHeaderLayer`.

use rust_web_server::prelude::*;
use rust_web_server::tower::{mount, TowerLayer};
use tower_http::set_header::SetResponseHeaderLayer;

pub fn app() -> impl rust_web_server::application::Application + Clone + Send + Sync + 'static {
    let routes = App::with_state(())
        .get("/hello", |_, _, _, _| Response::json(&STATUS_CODE_REASON_PHRASE.n200_ok, br#"{"hello":"tower"}"#.to_vec()))
        .wrap(TowerLayer::new(SetResponseHeaderLayer::overriding(
            http::header::HeaderName::from_static("x-served-by"),
            http::HeaderValue::from_static("rws + tower-http"),
        )));
    let (_reporter, health) = tonic_health::server::health_reporter();
    mount("/grpc.health.v1.Health/", health, routes)
}

#[tokio::main]
async fn main() {
    let (listener, pool) = Server::setup().unwrap();
    Server::run_tls(listener, pool, app()).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_web_server::application::Application;
    use rust_web_server::test_client::TestClient;

    #[test]
    fn the_rws_route_gets_the_tower_http_header() {
        let response = TestClient::new(app()).get("/hello").send();
        assert_eq!(200, response.status());
        assert_eq!(Some("rws + tower-http"), response.header("x-served-by"));
    }

    #[test]
    fn the_health_service_answers_serving() {
        use std::io::Read;
        let request = rust_web_server::request::Request {
            method: "POST".into(),
            request_uri: "/grpc.health.v1.Health/Check".into(),
            http_version: "HTTP/2.0".into(),
            headers: vec![
                rust_web_server::header::Header::new("content-type".to_string(), "application/grpc".to_string()),
                rust_web_server::header::Header::new("te".to_string(), "trailers".to_string()),
            ],
            body: vec![0u8, 0, 0, 0, 0].into(),
        };
        let connection = rust_web_server::server::ConnectionInfo {
            client: rust_web_server::server::Address { ip: "127.0.0.1".into(), port: 1 },
            server: rust_web_server::server::Address { ip: "127.0.0.1".into(), port: 7878 },
            request_size: 0,
            sni_hostname: None,
        };
        let mut response = app().execute(&request, &connection).unwrap();
        let mut body = Vec::new();
        response.stream_pipe.take().unwrap().read_to_end(&mut body).unwrap();
        assert_eq!(vec![0, 0, 0, 0, 2, 8, 1], body, "SERVING");
        let trailers = response.trailers.unwrap().get();
        assert!(trailers.iter().any(|h| h.name.eq_ignore_ascii_case("grpc-status") && h.value == "0"), "{trailers:?}");
    }
}
