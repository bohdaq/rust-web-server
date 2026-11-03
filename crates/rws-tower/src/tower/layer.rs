//! [`TowerLayer`]: a Tower layer as rws middleware.

use std::future::Future;
use std::pin::Pin;

use bytes::Bytes;
use http_body_util::BodyExt;

use super::{block_on, call, from_http_response, to_http_request, to_http_response, Body, BoxError};
use crate::application::Application;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

/// A Tower [`Layer`](tower_layer::Layer) — tower-http's
/// `SetResponseHeaderLayer`, `ValidateRequestHeaderLayer`, `TimeoutLayer`,
/// `NormalizePathLayer`, … — as rws [`Middleware`]: `app.wrap(TowerLayer::new(layer))`.
///
/// Per request the layer wraps the rest of the stack ([`Next`]); the request
/// goes in as `http::Request<Body>` and the response comes back through the
/// layer as `http::Response<_>`. Streamed responses (files, SSE, proxied
/// streams) stay streams. Request bodies reach the layer read into memory.
/// It runs on a blocking-pool thread, never inline on the event loop.
pub struct TowerLayer<L> {
    layer: L,
}

impl<L> TowerLayer<L> {
    pub fn new(layer: L) -> TowerLayer<L> {
        TowerLayer { layer }
    }
}

/// The rest of the rws stack, as the Tower service a layer wraps.
pub struct Next<'a> {
    app: &'a dyn Application,
    connection: &'a ConnectionInfo,
}

impl<'a, ReqBody> tower_service::Service<http::Request<ReqBody>> for Next<'a>
where
    ReqBody: http_body::Body<Data = Bytes> + 'a,
    ReqBody::Error: Into<BoxError>,
{
    type Response = http::Response<Body>;
    type Error = BoxError;
    type Future = Pin<Box<dyn Future<Output = Result<http::Response<Body>, BoxError>> + 'a>>;

    fn poll_ready(&mut self, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), BoxError>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: http::Request<ReqBody>) -> Self::Future {
        let (app, connection) = (self.app, self.connection);
        Box::pin(async move {
            let (parts, body) = request.into_parts();
            let bytes = body.collect().await.map_err(Into::into)?.to_bytes();
            let request = super::from_http_request(http::Request::from_parts(parts, Body::full(bytes))).await?;
            let response = app.execute(&request, connection).map_err(BoxError::from)?;
            Ok(to_http_response(response))
        })
    }
}

impl<L, ResBody> Middleware for TowerLayer<L>
where
    L: for<'a> tower_layer::Layer<Next<'a>> + Send + Sync,
    for<'a> <L as tower_layer::Layer<Next<'a>>>::Service: tower_service::Service<http::Request<Body>, Response = http::Response<ResBody>>,
    for<'a> <<L as tower_layer::Layer<Next<'a>>>::Service as tower_service::Service<http::Request<Body>>>::Error: Into<BoxError>,
    ResBody: http_body::Body<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
{
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        let service = self.layer.layer(Next { app: next, connection });
        let request = to_http_request(request.clone());
        block_on(async move {
            let response = call(service, request).await?;
            from_http_response(response).await
        })
        .map_err(|e| format!("tower layer: {e}"))
    }
}
