//! `server::http1_async` against a real S3 backend (the facade's `storage`).
#![allow(unused_imports, dead_code)]

use crate::server::http1_async::*;
use crate::application::Application;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn peak_rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status").ok()
        .and_then(|status| status.lines().find(|l| l.starts_with("VmHWM:"))
            .and_then(|l| l.split_whitespace().nth(1)?.parse().ok()))
        .unwrap_or(0)
}

fn peer() -> SocketAddr {
    "127.0.0.1:5555".parse().unwrap()
}

/// #31's exit criterion end to end: a multi-gigabyte HTTP upload, read
/// by the handler as a stream and sent to S3 in parts, in constant
/// memory. Needs an S3-compatible server (`RWS_S3_E2E_*`, as for
/// `storage::s3` tests); run alone with `--ignored --test-threads=1`.
#[cfg(feature = "storage-s3")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn a_multi_gigabyte_upload_streams_to_s3_in_constant_memory() {
    use crate::storage::{S3Config, S3Storage, Storage};
    const TOTAL: usize = 2 * 1024 * 1024 * 1024;
    let env = |name: &str| std::env::var(format!("RWS_S3_E2E_{name}")).expect("set RWS_S3_E2E_*");
    let storage = std::sync::Arc::new(S3Storage::new(S3Config {
        bucket: env("BUCKET"),
        region: "us-east-1".to_string(),
        access_key: env("ACCESS_KEY"),
        secret_key: env("SECRET_KEY"),
        endpoint: env("ENDPOINT"),
    }));

    #[derive(Clone)]
    struct ToS3(std::sync::Arc<S3Storage>);
    impl Application for ToS3 {
        fn execute(&self, request: &Request, _c: &ConnectionInfo) -> Result<Response, String> {
            let stored = self.0.put_stream("rws-e2e/huge.bin", &mut &request.body, "application/octet-stream");
            Ok(Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, &format!("{stored:?}")))
        }
        fn streams_body(&self, _r: &Request, _c: &ConnectionInfo) -> bool {
            true
        }
    }

    let before = peak_rss_kib();
    let (client, server) = tokio::io::duplex(1 << 16);
    let task = tokio::spawn(serve(server, peer(), None, false, ToS3(storage.clone())));
    let (mut read_half, mut write_half) = tokio::io::split(client);
    let writer = tokio::spawn(async move {
        write_half.write_all(format!("PUT /upload HTTP/1.1\r\nHost: x\r\nContent-Length: {TOTAL}\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
        let chunk = vec![b'u'; 1 << 20];
        for _ in 0..TOTAL / chunk.len() {
            write_half.write_all(&chunk).await.unwrap();
        }
    });
    let mut response = Vec::new();
    read_half.read_to_end(&mut response).await.unwrap();
    writer.await.unwrap();
    let response = String::from_utf8_lossy(&response).to_string();
    assert!(response.contains("Ok(\"rws-e2e/huge.bin\")"), "{response}");
    let grown_mib = peak_rss_kib().saturating_sub(before) / 1024;
    assert!(grown_mib < 96, "a 2 GiB upload to S3 grew the peak by {grown_mib} MiB");
    let _ = task.await;
    tokio::task::spawn_blocking(move || storage.delete("rws-e2e/huge.bin").unwrap()).await.unwrap();
}
