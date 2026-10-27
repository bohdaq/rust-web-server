//! server <addr> [<cert.pem> <key.pem> <client-ca.pem>]: the echo service,
//! cleartext or TLS requiring a client certificate signed by the CA.

use std::pin::Pin;
use std::time::Duration;

use rws_grpc_e2e::echo::echo_server::{Echo, EchoServer};
use rws_grpc_e2e::echo::Msg;
use tokio_stream::{Stream, StreamExt};
use tonic::transport::{Certificate, Identity, Server, ServerTlsConfig};
use tonic::{Request, Response, Status, Streaming};

type MsgStream = Pin<Box<dyn Stream<Item = Result<Msg, Status>> + Send>>;

struct Service;

#[tonic::async_trait]
impl Echo for Service {
    async fn unary(&self, request: Request<Msg>) -> Result<Response<Msg>, Status> {
        Ok(Response::new(request.into_inner()))
    }

    type ServerStreamStream = MsgStream;

    async fn server_stream(&self, request: Request<Msg>) -> Result<Response<MsgStream>, Status> {
        let text = request.into_inner().text;
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        tokio::spawn(async move {
            for i in 0..3 {
                if text == "fail" && i == 1 {
                    let _ = tx.send(Err(Status::aborted("stopped"))).await;
                    return;
                }
                let _ = tx.send(Ok(Msg { text: format!("{text}-{i}") })).await;
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        });
        Ok(Response::new(Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx))))
    }

    async fn client_stream(&self, request: Request<Streaming<Msg>>) -> Result<Response<Msg>, Status> {
        let mut stream = request.into_inner();
        let mut texts = Vec::new();
        while let Some(msg) = stream.next().await {
            texts.push(msg?.text);
        }
        Ok(Response::new(Msg { text: texts.join(",") }))
    }

    type BidiStream = MsgStream;

    async fn bidi(&self, request: Request<Streaming<Msg>>) -> Result<Response<MsgStream>, Status> {
        let stream = request.into_inner();
        Ok(Response::new(Box::pin(stream.map(|msg| msg.map(|m| Msg { text: m.text })))))
    }

    async fn fail(&self, _request: Request<Msg>) -> Result<Response<Msg>, Status> {
        Err(Status::invalid_argument("boom"))
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let addr = args.get(1).expect("server <addr> [cert key client-ca]").parse().unwrap();
    let mut builder = Server::builder();
    if args.len() == 5 {
        let read = |p: &str| std::fs::read(p).unwrap_or_else(|e| panic!("{p}: {e}"));
        let tls = ServerTlsConfig::new()
            .identity(Identity::from_pem(read(&args[2]), read(&args[3])))
            .client_ca_root(Certificate::from_pem(read(&args[4])));
        builder = builder.tls_config(tls).unwrap();
    }
    builder.add_service(EchoServer::new(Service)).serve(addr).await.unwrap();
}
