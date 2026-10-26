//! Entry points for `fuzz/` and the stack-depth tests (not API).

use crate::application::Application;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

/// Decode `data` as a series of header blocks (each prefixed by a one-byte
/// length) with one decoder: errors are fine, panics and hangs are not.
pub fn hpack_decode(data: &[u8]) {
    let mut decoder = super::hpack::Decoder::new(super::hpack::DEFAULT_TABLE_SIZE);
    let mut rest = data;
    while let Some((&len, tail)) = rest.split_first() {
        let len = (len as usize).min(tail.len());
        let (block, tail) = tail.split_at(len);
        let mut fields = 0usize;
        if decoder.decode(block, super::MAX_HEADER_LIST_SIZE as usize, |_, _| fields += 1) == Err(super::hpack::DecodeError::Compression) {
            return; // the connection would end here
        }
        rest = tail;
    }
}

#[derive(Clone)]
struct Echo;

impl Application for Echo {
    fn execute(&self, request: &Request, _connection: &ConnectionInfo) -> Result<Response, String> {
        let mut response = Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "");
        response.content_range_list[0].body = request.body.to_vec();
        Ok(response)
    }

    fn runs_inline(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }
}

/// Run a connection whose client sends the preface, an empty SETTINGS and
/// then `data`, until the server closes it.
pub fn connection(data: &[u8]) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(r) => r,
        Err(_) => return,
    };
    runtime.block_on(async {
        let (mut client, server_io) = tokio::io::duplex(64 * 1024);
        let peer = std::net::SocketAddr::from(([127, 0, 0, 1], 1));
        let server = tokio::spawn(super::handle_connection(server_io, peer, None, Echo));
        let mut input = super::PREFACE.to_vec();
        input.extend_from_slice(&[0, 0, 0, 4, 0, 0, 0, 0, 0]);
        input.extend_from_slice(data);
        let (mut read, mut write) = tokio::io::split(&mut client);
        let writer = async {
            let _ = write.write_all(&input).await;
            let _ = write.shutdown().await;
        };
        let mut out = Vec::new();
        let reader = read.read_to_end(&mut out);
        let _ = tokio::time::timeout(std::time::Duration::from_secs(10), async { tokio::join!(writer, reader) }).await;
        drop(client);
        let _ = tokio::time::timeout(std::time::Duration::from_secs(10), server).await;
    });
}
