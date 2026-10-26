//! Large uploads streamed to disk as they arrive: memory stays at a few
//! chunks whatever the file size (opt in per route with `stream_body`).
//!
//! ```sh
//! cargo run --example upload
//! curl -T big.iso localhost:7878/upload/big.iso     # 4700372992 bytes
//! ```

use std::path::{Path, PathBuf};

use rust_web_server::prelude::*;
use rust_web_server::state::AppWithState;

/// Uploads land in `dir`.
pub fn app(dir: PathBuf) -> AppWithState<PathBuf> {
    App::with_state(dir)
        .put("/upload/:name", |request, params, _, dir| {
            let name = params.get("name").unwrap_or_default();
            // A file name, never a path: no separators, no dot-files.
            if name.is_empty() || name.starts_with('.') || name.contains(['/', '\\']) {
                return Response::text(STATUS_CODE_REASON_PHRASE.n400_bad_request, "bad file name");
            }
            let path = Path::new(dir).join(name);
            let mut file = match std::fs::File::create(&path) {
                Ok(file) => file,
                Err(e) => return Response::text(STATUS_CODE_REASON_PHRASE.n500_internal_server_error, &e.to_string()),
            };
            // `into_reader` reads a streamed body as it arrives, or a body
            // already in memory; either way it is copied a chunk at a time.
            match std::io::copy(&mut request.body.clone().into_reader(), &mut file) {
                Ok(bytes) => Response::text(STATUS_CODE_REASON_PHRASE.n201_created, &format!("{bytes} bytes")),
                Err(e) => {
                    let _ = std::fs::remove_file(&path);
                    Response::text(STATUS_CODE_REASON_PHRASE.n400_bad_request, &e.to_string())
                }
            }
        })
        .stream_body("PUT", "/upload/:name")
}

#[tokio::main]
async fn main() {
    let dir = PathBuf::from("uploads");
    std::fs::create_dir_all(&dir).unwrap();
    let (listener, pool) = Server::setup().unwrap();
    Server::run_tls(listener, pool, app(dir)).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_web_server::test_client::TestClient;

    #[test]
    fn an_upload_is_written_to_disk() {
        let dir = std::env::temp_dir().join(format!("rws_upload_example_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let client = TestClient::new(app(dir.clone()));
        let response = client.put("/upload/notes.txt").body_text("hello upload").send();
        assert_eq!(201, response.status(), "{}", response.body_text());
        assert_eq!("12 bytes", response.body_text());
        assert_eq!("hello upload", std::fs::read_to_string(dir.join("notes.txt")).unwrap());
        assert_eq!(400, client.put("/upload/.env").body_text("x").send().status());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
