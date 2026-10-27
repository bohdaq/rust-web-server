//! Certificates and challenges shared between replicas through Redis
//! (`[acme] redis`): one replica orders a certificate while the others wait
//! for it, every replica installs what another one renewed, and a challenge
//! one replica presents can be answered by any of them — the CA's request may
//! reach another replica behind a load balancer.
//!
//! Keys (under `redis_prefix`, default `rws:acme`):
//! `<prefix>:<domains>:cert|key|lock` for a certificate (domains in order,
//! comma-separated), `<prefix>:challenge:<token>` (HTTP-01) and
//! `<prefix>:tls-alpn:<domain>` (TLS-ALPN-01), both expiring after 10
//! minutes. The certificate's private key is stored in Redis: keep Redis on
//! a private network.
//!
//! Redis being unreachable is never fatal: the replica carries on alone with
//! its local files and says so.

use std::sync::OnceLock;
use std::time::Duration;

use crate::entry_point::Config;
use crate::redis_protocol::{RespConn, RespReply};

/// How long a replica may hold the ordering lock.
const LOCK_MS: u64 = 5 * 60 * 1000;
/// Challenge answers expire after this.
const CHALLENGE_MS: u64 = 10 * 60 * 1000;

pub(crate) struct Shared {
    conn: RespConn,
    prefix: String,
    /// Identifies this replica in the lock.
    owner: String,
}

/// The configured store, or `None` (read once).
pub(crate) fn shared() -> Option<&'static Shared> {
    static SHARED: OnceLock<Option<Shared>> = OnceLock::new();
    SHARED
        .get_or_init(|| {
            let addr = super::non_empty_env(Config::RWS_CONFIG_ACME_REDIS)?;
            let password = super::non_empty_env(Config::RWS_CONFIG_ACME_REDIS_PASSWORD);
            let prefix = super::non_empty_env(Config::RWS_CONFIG_ACME_REDIS_PREFIX).unwrap_or_else(|| "rws:acme".to_string());
            let host = std::fs::read_to_string("/proc/sys/kernel/hostname").map(|h| h.trim().to_string()).unwrap_or_default();
            println!("[ACME] Sharing certificates and challenges through Redis at {addr} ({prefix}).");
            Some(Shared { conn: RespConn::new(addr, password), prefix, owner: format!("{host}:{}", std::process::id()) })
        })
        .as_ref()
}

impl Shared {
    fn key(&self, parts: &[&str]) -> String {
        let mut key = self.prefix.clone();
        for part in parts {
            key.push(':');
            key.push_str(part);
        }
        key
    }

    fn certificate_key(&self, domains: &[String], part: &str) -> String {
        self.key(&[&domains.join(","), part])
    }

    fn get(&self, key: &str) -> Option<Vec<u8>> {
        match self.conn.cmd(&[b"GET", key.as_bytes()]) {
            Ok(RespReply::Bulk(value)) => value,
            Ok(_) => None,
            Err(e) => {
                eprintln!("[ACME] Redis GET {key}: {e}");
                None
            }
        }
    }

    fn set(&self, key: &str, value: &[u8], expire_ms: Option<u64>) -> bool {
        let ms = expire_ms.map(|ms| ms.to_string());
        let mut args: Vec<&[u8]> = vec![b"SET", key.as_bytes(), value];
        if let Some(ms) = &ms {
            args.extend_from_slice(&[b"PX", ms.as_bytes()]);
        }
        match self.conn.cmd(&args) {
            Ok(RespReply::Ok) => true,
            Ok(_) => false,
            Err(e) => {
                eprintln!("[ACME] Redis SET {key}: {e}");
                false
            }
        }
    }

    fn del(&self, key: &str) {
        let _ = self.conn.cmd(&[b"DEL", key.as_bytes()]);
    }

    /// The certificate and key another replica stored for `domains`.
    pub(crate) fn certificate(&self, domains: &[String]) -> Option<(Vec<u8>, Vec<u8>)> {
        let cert = self.get(&self.certificate_key(domains, "cert"))?;
        let key = self.get(&self.certificate_key(domains, "key"))?;
        Some((cert, key))
    }

    pub(crate) fn store_certificate(&self, domains: &[String], cert: &[u8], key: &[u8]) {
        // The key first: a replica that sees the new certificate finds its key.
        if !(self.set(&self.certificate_key(domains, "key"), key, None) && self.set(&self.certificate_key(domains, "cert"), cert, None)) {
            eprintln!("[ACME] The new certificate couldn't be stored in Redis; other replicas keep theirs.");
        }
    }

    /// Take the ordering lock for `domains`: `Some(true)` taken, `Some(false)`
    /// another replica holds it, `None` Redis unreachable.
    pub(crate) fn lock(&self, domains: &[String]) -> Option<bool> {
        let ms = LOCK_MS.to_string();
        let key = self.certificate_key(domains, "lock");
        match self.conn.cmd(&[b"SET", key.as_bytes(), self.owner.as_bytes(), b"NX", b"PX", ms.as_bytes()]) {
            Ok(RespReply::Ok) => Some(true),
            Ok(_) => Some(false), // a nil bulk: taken
            Err(e) => {
                eprintln!("[ACME] Redis lock {key}: {e}; ordering without it.");
                None
            }
        }
    }

    /// Release the lock if this replica still holds it.
    pub(crate) fn unlock(&self, domains: &[String]) {
        let key = self.certificate_key(domains, "lock");
        let script = b"if redis.call('get', KEYS[1]) == ARGV[1] then return redis.call('del', KEYS[1]) else return 0 end";
        let _ = self.conn.cmd(&[b"EVAL", script, b"1", key.as_bytes(), self.owner.as_bytes()]);
    }

    pub(crate) fn put_challenge(&self, token: &str, key_authorization: &str) {
        self.set(&self.key(&["challenge", token]), key_authorization.as_bytes(), Some(CHALLENGE_MS));
    }

    pub(crate) fn challenge(&self, token: &str) -> Option<String> {
        self.get(&self.key(&["challenge", token])).and_then(|v| String::from_utf8(v).ok())
    }

    pub(crate) fn remove_challenge(&self, token: &str) {
        self.del(&self.key(&["challenge", token]));
    }

    pub(crate) fn put_tls_alpn(&self, domain: &str, key_authorization: &str) {
        self.set(&self.key(&["tls-alpn", &domain.to_ascii_lowercase()]), key_authorization.as_bytes(), Some(CHALLENGE_MS));
    }

    pub(crate) fn tls_alpn(&self, domain: &str) -> Option<String> {
        self.get(&self.key(&["tls-alpn", &domain.to_ascii_lowercase()])).and_then(|v| String::from_utf8(v).ok())
    }

    pub(crate) fn remove_tls_alpn(&self, domain: &str) {
        self.del(&self.key(&["tls-alpn", &domain.to_ascii_lowercase()]));
    }
}

/// How long a replica that lost the lock waits for the other's certificate.
pub(crate) const WAIT_FOR_OTHER: Duration = Duration::from_millis(LOCK_MS);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::sync::{Arc, Mutex};

    /// A Redis speaking just what this module sends: SET [NX] [PX], GET,
    /// DEL and the unlock script.
    fn fake_redis() -> (String, Arc<Mutex<HashMap<String, Vec<u8>>>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let data: Arc<Mutex<HashMap<String, Vec<u8>>>> = Arc::default();
        let store = Arc::clone(&data);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let store = Arc::clone(&store);
                std::thread::spawn(move || serve(stream, &store));
            }
        });
        (addr, data)
    }

    fn serve(mut stream: std::net::TcpStream, store: &Mutex<HashMap<String, Vec<u8>>>) {
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            let argc: usize = line.trim_end()[1..].parse().unwrap();
            let mut args: Vec<Vec<u8>> = Vec::new();
            for _ in 0..argc {
                let mut len = String::new();
                reader.read_line(&mut len).unwrap();
                let mut arg = vec![0u8; len.trim_end()[1..].parse::<usize>().unwrap() + 2];
                reader.read_exact(&mut arg).unwrap();
                arg.truncate(arg.len() - 2);
                args.push(arg);
            }
            let text = |i: usize| String::from_utf8_lossy(&args[i]).to_string();
            let mut data = store.lock().unwrap();
            let reply: Vec<u8> = match text(0).to_uppercase().as_str() {
                "SET" => {
                    let nx = args.iter().any(|a| a.eq_ignore_ascii_case(b"NX"));
                    if nx && data.contains_key(&text(1)) {
                        b"$-1\r\n".to_vec()
                    } else {
                        data.insert(text(1), args[2].clone());
                        b"+OK\r\n".to_vec()
                    }
                }
                "GET" => match data.get(&text(1)) {
                    Some(v) => [format!("${}\r\n", v.len()).into_bytes(), v.clone(), b"\r\n".to_vec()].concat(),
                    None => b"$-1\r\n".to_vec(),
                },
                "DEL" => format!(":{}\r\n", data.remove(&text(1)).is_some() as u8).into_bytes(),
                "EVAL" => {
                    let held = data.get(&text(3)).is_some_and(|v| v == &args[4]);
                    if held {
                        data.remove(&text(3));
                    }
                    format!(":{}\r\n", held as u8).into_bytes()
                }
                _ => b"-ERR unknown\r\n".to_vec(),
            };
            drop(data);
            if stream.write_all(&reply).is_err() {
                return;
            }
        }
    }

    fn replica(addr: &str, owner: &str) -> Shared {
        Shared { conn: RespConn::new(addr.to_string(), None), prefix: "test:acme".to_string(), owner: owner.to_string() }
    }

    fn domains() -> Vec<String> {
        vec!["example.test".to_string(), "*.example.test".to_string()]
    }

    #[test]
    fn one_replica_holds_the_ordering_lock_at_a_time() {
        let (addr, data) = fake_redis();
        let (a, b) = (replica(&addr, "a"), replica(&addr, "b"));
        assert_eq!(Some(true), a.lock(&domains()));
        assert_eq!(Some(false), b.lock(&domains()));
        b.unlock(&domains()); // not b's: kept
        assert!(data.lock().unwrap().contains_key("test:acme:example.test,*.example.test:lock"));
        a.unlock(&domains());
        assert_eq!(Some(true), b.lock(&domains()));
    }

    #[test]
    fn a_stored_certificate_is_found_by_every_replica() {
        let (addr, _) = fake_redis();
        let (a, b) = (replica(&addr, "a"), replica(&addr, "b"));
        assert_eq!(None, b.certificate(&domains()));
        a.store_certificate(&domains(), b"CERT", b"KEY");
        assert_eq!(Some((b"CERT".to_vec(), b"KEY".to_vec())), b.certificate(&domains()));
    }

    #[test]
    fn challenges_are_shared_and_removed() {
        let (addr, _) = fake_redis();
        let (a, b) = (replica(&addr, "a"), replica(&addr, "b"));
        a.put_challenge("tok", "tok.thumb");
        assert_eq!(Some("tok.thumb".to_string()), b.challenge("tok"));
        a.remove_challenge("tok");
        assert_eq!(None, b.challenge("tok"));
        a.put_tls_alpn("Example.Test", "k.a");
        assert_eq!(Some("k.a".to_string()), b.tls_alpn("example.test"));
    }

    #[test]
    fn an_unreachable_redis_is_not_fatal() {
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().to_string();
        let lonely = replica(&closed, "a");
        assert_eq!(None, lonely.lock(&domains()));
        assert_eq!(None, lonely.certificate(&domains()));
        lonely.store_certificate(&domains(), b"C", b"K");
    }
}
