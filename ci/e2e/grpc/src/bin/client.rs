//! client <https://rws:port> <ca.pem>: every call type through rws; exits
//! non-zero on the first wrong answer.

use std::time::Duration;

use rws_grpc_e2e::echo::echo_client::EchoClient;
use rws_grpc_e2e::echo::Msg;
use tokio_stream::StreamExt;
use tonic::transport::{Certificate, Channel, ClientTlsConfig};
use tonic::Code;

fn msg(text: &str) -> Msg {
    Msg { text: text.to_string() }
}

fn check(name: &str, ok: bool, detail: String) {
    if ok {
        println!("ok   {name}");
    } else {
        println!("FAIL {name}: {detail}");
        std::process::exit(1);
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (url, ca) = (args.get(1).expect("client <url> <ca.pem>").clone(), args.get(2).expect("ca").clone());
    let tls = ClientTlsConfig::new()
        .ca_certificate(Certificate::from_pem(std::fs::read(&ca).unwrap()))
        .domain_name("localhost");
    let channel = Channel::from_shared(url).unwrap().tls_config(tls).unwrap().timeout(Duration::from_secs(10)).connect().await.unwrap();
    let mut client = EchoClient::new(channel);

    let unary = client.unary(msg("hi")).await.map(|r| r.into_inner().text);
    check("unary", matches!(unary.as_deref(), Ok("hi")), format!("{unary:?}"));

    let mut stream = client.server_stream(msg("s")).await.unwrap().into_inner();
    let mut got = Vec::new();
    while let Some(m) = stream.next().await {
        got.push(m.unwrap().text);
    }
    check("server streaming", got == ["s-0", "s-1", "s-2"], format!("{got:?}"));

    let mut stream = client.server_stream(msg("fail")).await.unwrap().into_inner();
    let first = stream.next().await.map(|m| m.map(|m| m.text));
    let status = stream.next().await;
    let ok = matches!(&first, Some(Ok(t)) if t == "fail-0")
        && matches!(&status, Some(Err(s)) if s.code() == Code::Aborted && s.message() == "stopped");
    check("status after messages", ok, format!("{first:?} then {status:?}"));

    let outbound = tokio_stream::iter(["a", "b", "c"].map(msg));
    let joined = client.client_stream(outbound).await.map(|r| r.into_inner().text);
    check("client streaming", matches!(joined.as_deref(), Ok("a,b,c")), format!("{joined:?}"));

    // Bidirectional: the second message is sent only after the first echo.
    let (tx, rx) = tokio::sync::mpsc::channel(4);
    let mut replies = client.bidi(tokio_stream::wrappers::ReceiverStream::new(rx)).await.unwrap().into_inner();
    tx.send(msg("one")).await.unwrap();
    let first = tokio::time::timeout(Duration::from_secs(5), replies.next()).await;
    check("bidi first echo before the second message", matches!(&first, Ok(Some(Ok(m))) if m.text == "one"), format!("{first:?}"));
    tx.send(msg("two")).await.unwrap();
    drop(tx);
    let second = replies.next().await.map(|m| m.map(|m| m.text));
    let end = replies.next().await;
    check("bidi", matches!(&second, Some(Ok(t)) if t == "two") && end.is_none(), format!("{second:?} then {end:?}"));

    let failed = client.fail(msg("x")).await;
    let ok = matches!(&failed, Err(s) if s.code() == Code::InvalidArgument && s.message() == "boom");
    check("trailers-only error", ok, format!("{failed:?}"));
}
