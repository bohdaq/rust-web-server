//! Minimal hand-rolled RESP v2 client shared by Redis-backed features
//! ([`crate::session::RedisSessionStore`], [`crate::rate_limit::RedisRateLimiter`]).
//!
//! Crate-internal only — not part of the public API. No third-party Redis
//! client dependency.

use std::sync::Mutex;
use std::time::Duration;

/// A minimal RESP v2 connection. Reconnects automatically when the
/// underlying TCP connection is dropped.
pub(crate) struct RespConn {
    addr: String,
    password: Option<String>,
    stream: Mutex<Option<std::net::TcpStream>>,
}

/// A decoded RESP reply.
pub(crate) enum RespReply {
    Ok,
    Int(i64),
    Bulk(Option<Vec<u8>>),
    Error(String),
}

impl RespConn {
    pub(crate) fn new(addr: impl Into<String>, password: Option<String>) -> Self {
        RespConn { addr: addr.into(), password, stream: Mutex::new(None) }
    }

    fn connect(&self) -> std::io::Result<std::net::TcpStream> {
        let stream = std::net::TcpStream::connect(&self.addr)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        Ok(stream)
    }

    /// Send a Redis command (array of byte slices) and return the decoded reply.
    pub(crate) fn cmd(&self, args: &[&[u8]]) -> std::io::Result<RespReply> {
        use std::io::Write;
        let mut guard = self.stream.lock().unwrap();
        // Lazy connect / reconnect
        if guard.is_none() {
            let mut s = self.connect()?;
            if let Some(ref pw) = self.password {
                let auth_frame = resp_array(&[b"AUTH", pw.as_bytes()]);
                s.write_all(&auth_frame)?;
                read_reply(&mut s)?; // consume +OK
            }
            *guard = Some(s);
        }
        let frame = resp_array(args);
        let stream = guard.as_mut().unwrap();
        if stream.write_all(&frame).is_err() {
            // Connection broke — drop and retry once
            *guard = None;
            drop(guard);
            return self.cmd(args);
        }
        match read_reply(stream)? {
            // Surface RESP-level errors (auth failure, wrong type, readonly
            // replica, ...) as a real error instead of letting callers treat
            // them as "not found" / zero / success.
            RespReply::Error(msg) => Err(std::io::Error::new(std::io::ErrorKind::Other, msg)),
            reply => Ok(reply),
        }
    }
}

fn resp_array(args: &[&[u8]]) -> Vec<u8> {
    let mut out = format!("*{}\r\n", args.len()).into_bytes();
    for arg in args {
        out.extend_from_slice(format!("${}\r\n", arg.len()).as_bytes());
        out.extend_from_slice(arg);
        out.extend_from_slice(b"\r\n");
    }
    out
}

fn read_reply(stream: &mut std::net::TcpStream) -> std::io::Result<RespReply> {
    use std::io::{BufRead, BufReader, Read};
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let line = line.trim_end_matches("\r\n");
    match line.chars().next() {
        Some('+') => Ok(RespReply::Ok),
        Some(':') => {
            let n = line[1..].parse::<i64>().unwrap_or(0);
            Ok(RespReply::Int(n))
        }
        Some('-') => Ok(RespReply::Error(line[1..].to_string())),
        Some('$') => {
            let len = line[1..].parse::<i64>().unwrap_or(-1);
            if len < 0 {
                return Ok(RespReply::Bulk(None));
            }
            let mut buf = vec![0u8; len as usize + 2]; // +2 for \r\n
            reader.read_exact(&mut buf)?;
            buf.truncate(len as usize);
            Ok(RespReply::Bulk(Some(buf)))
        }
        _ => Ok(RespReply::Ok), // ignore arrays etc.
    }
}
