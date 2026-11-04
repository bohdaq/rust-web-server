//! Access-log lines to a syslog server (18.33.0, #102): RFC 5424 messages
//! over UDP, TCP (octet-counting framing, RFC 6587 §3.4.1) or a Unix
//! datagram socket such as `/dev/log`.
//!
//! `RWS_CONFIG_ACCESS_LOG_SYSLOG` = `udp://host:514`, `tcp://host:6514` or
//! `unix:///dev/log`; `RWS_CONFIG_ACCESS_LOG_SYSLOG_FACILITY` = `local0`
//! (default) … `local7`, or another facility name or number. Read once, at
//! the first access-log line. Lines go to syslog **as well as** stdout or
//! the access-log file, in the configured `RWS_CONFIG_LOG_FORMAT` (JSON is
//! what SIEMs parse best), severity `info`, APP-NAME `rws`, MSGID `access`.
//!
//! Nothing is sent from the request path: the access-log sink hands each
//! flushed batch (every 100 ms) to a queue that the thread `rws-syslog`
//! sends. While the server is unreachable batches wait up to
//! [`MAX_QUEUED`] bytes, then are dropped (counted by [`dropped_lines`]);
//! a TCP connection is re-made on the next batch.

use std::collections::VecDeque;
use std::io::Write;
use std::net::{TcpStream, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::Duration;

/// `RWS_CONFIG_ACCESS_LOG_SYSLOG`: where to send access-log lines.
pub const RWS_CONFIG_ACCESS_LOG_SYSLOG: &str = "RWS_CONFIG_ACCESS_LOG_SYSLOG";
/// `RWS_CONFIG_ACCESS_LOG_SYSLOG_FACILITY`: the syslog facility (default `local0`).
pub const RWS_CONFIG_ACCESS_LOG_SYSLOG_FACILITY: &str = "RWS_CONFIG_ACCESS_LOG_SYSLOG_FACILITY";

/// Bytes of unsent lines kept while the server is unreachable.
pub const MAX_QUEUED: usize = 8 << 20;
/// Severity of every access-log message: informational.
const SEVERITY_INFO: u8 = 6;

static DROPPED: AtomicU64 = AtomicU64::new(0);

/// Lines dropped because the queue was full.
pub fn dropped_lines() -> u64 {
    DROPPED.load(Ordering::Relaxed)
}

/// Where messages go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// `udp://host:port`: one datagram per line.
    Udp(String),
    /// `tcp://host:port`: octet-counted frames on one connection.
    Tcp(String),
    /// `unix:///path`: one datagram per line to a Unix socket.
    Unix(std::path::PathBuf),
}

impl Target {
    /// `udp://host:port`, `tcp://host:port` (port 514 when left out) or
    /// `unix:///path`.
    pub fn parse(value: &str) -> Result<Target, String> {
        let value = value.trim();
        let with_port = |rest: &str| -> Result<String, String> {
            let rest = rest.trim_end_matches('/');
            if rest.is_empty() {
                return Err(format!("{value}: no host"));
            }
            // `[::1]:514`, `host:514`, or a host alone.
            let has_port = match rest.rfind(':') {
                Some(i) => rest[i + 1..].parse::<u16>().is_ok() && (!rest.starts_with('[') || rest[..i].ends_with(']')),
                None => false,
            };
            Ok(if has_port { rest.to_string() } else { format!("{rest}:514") })
        };
        if let Some(rest) = value.strip_prefix("udp://") {
            Ok(Target::Udp(with_port(rest)?))
        } else if let Some(rest) = value.strip_prefix("tcp://") {
            Ok(Target::Tcp(with_port(rest)?))
        } else if let Some(path) = value.strip_prefix("unix://") {
            if !path.starts_with('/') {
                return Err(format!("{value}: the socket path must be absolute (unix:///dev/log)"));
            }
            Ok(Target::Unix(path.into()))
        } else {
            Err(format!("{value}: expected udp://host:port, tcp://host:port or unix:///path"))
        }
    }
}

/// A facility name (`kern`, `user`, … `local0`–`local7`) or number 0–23.
pub fn parse_facility(value: &str) -> Result<u8, String> {
    const NAMES: [&str; 24] = [
        "kern", "user", "mail", "daemon", "auth", "syslog", "lpr", "news", "uucp", "cron", "authpriv", "ftp", "ntp", "audit", "alert", "clock",
        "local0", "local1", "local2", "local3", "local4", "local5", "local6", "local7",
    ];
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() {
        return Ok(16);
    }
    if let Ok(n) = value.parse::<u8>() {
        return if n < 24 { Ok(n) } else { Err(format!("syslog facility {n}: must be 0–23")) };
    }
    NAMES.iter().position(|n| *n == value).map(|n| n as u8).ok_or_else(|| format!("unknown syslog facility {value}"))
}

/// Append one RFC 5424 message for `line` to `out`:
/// `<PRI>1 TIMESTAMP HOSTNAME rws PROCID access - LINE`.
pub fn message(out: &mut Vec<u8>, pri: u8, timestamp: &str, hostname: &str, pid: u32, line: &[u8]) {
    let _ = write!(out, "<{pri}>1 {timestamp} {hostname} rws {pid} access - ");
    out.extend_from_slice(line);
}

/// This host's name as RFC 5424 allows it (printable ASCII, no spaces,
/// at most 255 bytes), else `-`.
fn hostname() -> String {
    let name = std::env::var("HOSTNAME")
        .ok()
        .filter(|h| !h.trim().is_empty())
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .unwrap_or_default();
    let name: String = name.trim().chars().filter(|c| c.is_ascii_graphic()).take(255).collect();
    if name.is_empty() { "-".to_string() } else { name }
}

enum Connection {
    None,
    Udp(UdpSocket),
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(std::os::unix::net::UnixDatagram),
}

#[derive(Default)]
struct Queue {
    batches: VecDeque<Vec<u8>>,
    bytes: usize,
}

/// Sends access-log batches to one syslog [`Target`].
pub struct Sender {
    target: Target,
    pri: u8,
    hostname: String,
    queue: Mutex<Queue>,
    wake: Condvar,
    connection: Mutex<Connection>,
}

impl Sender {
    /// A sender for `target` with `facility`; nothing is connected or
    /// started until something is sent.
    pub fn new(target: Target, facility: u8) -> Sender {
        Sender {
            target,
            pri: facility * 8 + SEVERITY_INFO,
            hostname: hostname(),
            queue: Mutex::new(Queue::default()),
            wake: Condvar::new(),
            connection: Mutex::new(Connection::None),
        }
    }

    /// Queue a batch of newline-terminated lines (no network I/O here).
    pub fn enqueue(&self, lines: &[u8]) {
        if lines.is_empty() {
            return;
        }
        let mut queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        if queue.bytes + lines.len() > MAX_QUEUED {
            DROPPED.fetch_add(memchr::memchr_iter(b'\n', lines).count() as u64, Ordering::Relaxed);
            return;
        }
        queue.bytes += lines.len();
        queue.batches.push_back(lines.to_vec());
        drop(queue);
        self.wake.notify_one();
    }

    /// Send everything queued now, on this thread.
    pub fn send_pending(&self) {
        let batches = {
            let mut queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
            queue.bytes = 0;
            std::mem::take(&mut queue.batches)
        };
        if batches.is_empty() {
            return;
        }
        let mut connection = self.connection.lock().unwrap_or_else(|e| e.into_inner());
        let mut timestamp = String::new();
        super::iso8601_now(|now| timestamp.push_str(now));
        let pid = std::process::id();
        let lines = || batches.iter().flat_map(|b| b.split(|c| *c == b'\n')).filter(|l| !l.is_empty());
        let result = match self.target {
            Target::Tcp(_) => {
                let mut frames = Vec::new();
                let mut msg = Vec::new();
                for line in lines() {
                    msg.clear();
                    message(&mut msg, self.pri, &timestamp, &self.hostname, pid, line);
                    let _ = write!(frames, "{} ", msg.len());
                    frames.extend_from_slice(&msg);
                }
                // A connection that died since the last batch fails here;
                // one more try on a new one.
                self.send(&mut connection, &frames).or_else(|_| {
                    *connection = Connection::None;
                    self.send(&mut connection, &frames)
                })
            }
            _ => {
                let mut msg = Vec::new();
                let mut result = Ok(());
                for line in lines() {
                    msg.clear();
                    message(&mut msg, self.pri, &timestamp, &self.hostname, pid, line);
                    if let Err(e) = self.send(&mut connection, &msg) {
                        result = Err(e);
                        *connection = Connection::None;
                    }
                }
                result
            }
        };
        if let Err(e) = result {
            *connection = Connection::None;
            complain(&self.target, &e);
        }
    }

    /// Send one datagram, or bytes on the stream, connecting first if needed.
    fn send(&self, connection: &mut Connection, bytes: &[u8]) -> std::io::Result<()> {
        if matches!(connection, Connection::None) {
            *connection = self.connect()?;
        }
        match connection {
            Connection::None => Ok(()),
            Connection::Udp(socket) => socket.send(bytes).map(|_| ()),
            Connection::Tcp(stream) => stream.write_all(bytes),
            #[cfg(unix)]
            Connection::Unix(socket) => match &self.target {
                Target::Unix(path) => socket.send_to(bytes, path).map(|_| ()),
                _ => Ok(()),
            },
        }
    }

    fn connect(&self) -> std::io::Result<Connection> {
        match &self.target {
            Target::Udp(address) => {
                let remote = std::net::ToSocketAddrs::to_socket_addrs(address.as_str())?
                    .next()
                    .ok_or_else(|| std::io::Error::other(format!("{address}: no address")))?;
                let local = if remote.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" };
                let socket = UdpSocket::bind(local)?;
                socket.connect(remote)?;
                Ok(Connection::Udp(socket))
            }
            Target::Tcp(address) => {
                let remote = std::net::ToSocketAddrs::to_socket_addrs(address.as_str())?
                    .next()
                    .ok_or_else(|| std::io::Error::other(format!("{address}: no address")))?;
                let stream = TcpStream::connect_timeout(&remote, Duration::from_secs(5))?;
                stream.set_write_timeout(Some(Duration::from_secs(10)))?;
                Ok(Connection::Tcp(stream))
            }
            #[cfg(unix)]
            Target::Unix(_) => Ok(Connection::Unix(std::os::unix::net::UnixDatagram::unbound()?)),
            #[cfg(not(unix))]
            Target::Unix(_) => Err(std::io::Error::other("unix:// syslog targets need a Unix system")),
        }
    }

    /// Send queued batches until the process ends (the `rws-syslog` thread).
    fn run(&self) {
        loop {
            {
                let queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
                let _ = self.wake.wait_timeout_while(queue, Duration::from_secs(1), |q| q.batches.is_empty());
            }
            self.send_pending();
        }
    }
}

/// At most one error line a minute.
fn complain(target: &Target, error: &std::io::Error) {
    static LAST: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if last.is_none_or(|at| at.elapsed() >= Duration::from_secs(60)) {
        *last = Some(std::time::Instant::now());
        eprintln!("access log: syslog {target:?}: {error} (further errors within a minute aren't shown)");
    }
}

/// The configured sender, if `RWS_CONFIG_ACCESS_LOG_SYSLOG` is set; its
/// thread starts with the first call.
pub fn sender() -> Option<&'static Sender> {
    static SENDER: OnceLock<Option<Sender>> = OnceLock::new();
    let sender = SENDER.get_or_init(|| {
        let value = std::env::var(RWS_CONFIG_ACCESS_LOG_SYSLOG).unwrap_or_default();
        if value.trim().is_empty() {
            return None;
        }
        let configured = Target::parse(&value).and_then(|target| {
            let facility = parse_facility(&std::env::var(RWS_CONFIG_ACCESS_LOG_SYSLOG_FACILITY).unwrap_or_default())?;
            Ok(Sender::new(target, facility))
        });
        match configured {
            Ok(sender) => Some(sender),
            Err(e) => {
                eprintln!("access log: {RWS_CONFIG_ACCESS_LOG_SYSLOG}: {e}; not sending to syslog");
                None
            }
        }
    });
    let sender = sender.as_ref()?;
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let spawned = std::thread::Builder::new().name("rws-syslog".to_string()).spawn(move || sender.run());
        if spawned.is_err() {
            eprintln!("access log: no syslog thread; lines are sent at exit only");
        }
    });
    Some(sender)
}

/// Send what is queued now, on this thread (at process exit).
pub fn flush_now() {
    if std::env::var(RWS_CONFIG_ACCESS_LOG_SYSLOG).is_ok_and(|v| !v.trim().is_empty()) {
        if let Some(sender) = sender() {
            sender.send_pending();
        }
    }
}

#[cfg(test)]
mod tests;
