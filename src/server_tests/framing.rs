//! Tests of rws-server's `server::framing` that read repository fixtures (`static/`,
//! `index.html`): a crate's tests run in the crate's directory.
#![allow(unused_imports, dead_code)]

use crate::header::Header;
use crate::request::Request;

use crate::server::framing::*;

fn request(headers: &[(&str, &str)]) -> Request {
    Request {
        method: "POST".to_string(),
        request_uri: "/".to_string(),
        http_version: "HTTP/1.1".to_string(),
        headers: headers.iter().map(|(n, v)| Header { name: n.to_string().into(), value: v.to_string().into() }).collect(),
        body: Default::default(),
    }
}

fn check(head: &[u8]) -> Result<(), String> {
    let request = Request::parse(head)?;
    validate_head(head, &request)
}

/// The CL/TE smuggling corpus in fuzz/seeds (#8): every ambiguous framing
/// is refused somewhere before a body is read, and the two accepted
/// shapes (bare-LF chunks, a trailer naming Content-Length) end the body
/// where the chunked framing says, leaving the rest as the next request.
#[test]
fn the_smuggling_corpus_is_refused_or_framed_consistently() {
    fn outcome(data: &[u8]) -> Result<Vec<u8>, String> {
        let mut pending = data.to_vec();
        skip_leading_empty_lines(&mut pending);
        let HeadScan::Complete(head_len) = scan_head(&pending) else { return Err("head incomplete".into()) };
        let request = crate::request::Request::parse(&pending[..head_len]).map_err(|e| e.to_string())?;
        validate_head(&pending[..head_len], &request).map_err(|e| format!("{e:?}"))?;
        match body_framing(&request)? {
            BodyFraming::Chunked => {
                let (consumed, done) = ChunkedBody::new(1 << 20).feed(&pending[head_len..]).map_err(|e| format!("{e:?}"))?;
                assert!(done, "chunked body not finished");
                Ok(pending[head_len + consumed..].to_vec())
            }
            other => Err(format!("unexpected framing {other:?}")),
        }
    }
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/fuzz/seeds/http1_framing");
    let seed = |name: &str| std::fs::read(format!("{dir}/{name}")).unwrap();
    for refused in [
        "smuggle_cl_te", "smuggle_te_cl", "smuggle_te_te", "smuggle_te_obfuscated", "smuggle_two_cl",
        "smuggle_cl_list", "smuggle_chunk_ext", "smuggle_chunk_size_overflow", "smuggle_obs_fold",
    ] {
        assert!(outcome(&seed(refused)).is_err(), "{refused} was accepted");
    }
    assert_eq!(b"GET /x HTTP/1.1\r\n\r\n".to_vec(), outcome(&seed("smuggle_bare_lf")).unwrap());
    assert_eq!(b"GET /admin HTTP/1.1\r\n\r\n".to_vec(), outcome(&seed("smuggle_trailer")).unwrap());
}
