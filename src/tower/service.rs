//! [`TowerService`]: an rws app as a Tower service.

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use bytes::Bytes;

use super::{from_http_request, to_http_response, Body, BoxError};
use crate::application::Application;
use crate::response::Response;
use crate::server::{Address, ConnectionInfo};

/// An rws [`Application`] as a Tower `Service<http::Request<B>>` — to serve
/// it with hyper, or nest it in an axum `Router` (`Router::fallback_service`).
///
/// The request body is read into memory first. The app runs as rws runs it:
/// `execute_async` when it offers one, inline when it says so, otherwise on
/// the blocking pool — so call it from inside a tokio runtime. An app error
/// becomes a `500`; the service itself never fails. `ConnectionInfo` is
/// [`TowerService::connection`]'s (default: unknown peer, port 0), since
/// Tower carries no peer address of its own.
pub struct TowerService<A> {
    app: Arc<A>,
    connection: ConnectionInfo,
}

impl<A> Clone for TowerService<A> {
    fn clone(&self) -> Self {
        TowerService { app: Arc::clone(&self.app), connection: self.connection.clone() }
    }
}

impl<A> TowerService<A> {
    pub fn new(app: A) -> TowerService<A> {
        let unknown = Address { ip: "0.0.0.0".to_string(), port: 0 };
        TowerService { app: Arc::new(app), connection: ConnectionInfo { client: unknown.clone(), server: unknown, request_size: 0, sni_hostname: None, tls: false } }
    }

    /// What handlers see as the connection.
    pub fn connection(mut self, connection: ConnectionInfo) -> Self {
        self.connection = connection;
        self
    }
}

fn status(code: u16, message: &str) -> http::Response<Body> {
    let mut response = http::Response::new(Body::full(message.to_string()));
    *response.status_mut() = http::StatusCode::from_u16(code).unwrap_or(http::StatusCode::INTERNAL_SERVER_ERROR);
    response
}

impl<A, B> tower_service::Service<http::Request<B>> for TowerService<A>
where
    A: Application + Send + Sync + 'static,
    B: http_body::Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<http::Response<Body>, Infallible>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Infallible>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: http::Request<B>) -> Self::Future {
        let (app, connection) = (Arc::clone(&self.app), self.connection.clone());
        Box::pin(async move {
            let request = match from_http_request(request).await {
                Ok(request) => request,
                Err(e) => return Ok(status(400, &format!("unreadable request body: {e}"))),
            };
            let offered = match app.execute_async(&request, &connection) {
                Some(future) => Some(future.await),
                None => None,
            };
            let result: Result<Response, String> = match offered {
                Some(result) => result,
                None if app.runs_inline(&request, &connection) => app.execute(&request, &connection),
                None => tokio::task::spawn_blocking(move || app.execute(&request, &connection))
                    .await
                    .unwrap_or_else(|e| Err(format!("handler panicked: {e}"))),
            };
            Ok(match result {
                Ok(response) => to_http_response(response),
                Err(message) => status(500, &message),
            })
        })
    }
}
