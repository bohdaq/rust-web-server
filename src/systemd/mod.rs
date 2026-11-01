//! systemd integration (18.20.0, #67), without libsystemd.
//!
//! - **Notifications** (`Type=notify`): [`ready`] once rws serves, [`reloading`]
//!   / [`ready`] around a reload, [`stopping`] at shutdown, [`status`] lines,
//!   and `WATCHDOG=1` every half `WatchdogSec` while the runtime runs.
//!   Datagrams to `$NOTIFY_SOCKET` (a path, or `@name` in the abstract
//!   namespace); nothing happens without it.
//! - **Socket activation**: listeners systemd passes (`LISTEN_FDS`,
//!   `LISTEN_PID`, `LISTEN_FDNAMES`) are used instead of binding: [`tcp`] and
//!   [`udp`] return a copy of the inherited socket for an address, matched by
//!   port (and kind) — systemd's `ListenStream=443` is `[::]:443` while rws's
//!   `ip` may say `0.0.0.0`. systemd keeps the sockets across restarts, so
//!   nothing is refused while rws starts again.

use std::net::SocketAddr;
use std::os::fd::{FromRawFd, RawFd};
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Tcp,
    Udp,
}

#[derive(Debug)]
struct Inherited {
    fd: RawFd,
    kind: Kind,
    addr: SocketAddr,
    name: Option<String>,
}

/// The sockets systemd passed, read (and the variables removed, so child
/// processes don't think they're activated) once.
fn inherited() -> &'static Mutex<Vec<Inherited>> {
    static SOCKETS: OnceLock<Mutex<Vec<Inherited>>> = OnceLock::new();
    SOCKETS.get_or_init(|| {
        let found = read_inherited();
        if !found.is_empty() {
            let list: Vec<String> = found.iter().map(|s| format!("{} {:?}{}", s.addr, s.kind, s.name.as_deref().map(|n| format!(" ({n})")).unwrap_or_default())).collect();
            println!("systemd passed {} socket(s): {}", found.len(), list.join(", "));
        }
        Mutex::new(found)
    })
}

fn read_inherited() -> Vec<Inherited> {
    let pid_matches = std::env::var("LISTEN_PID").ok().and_then(|p| p.parse::<u32>().ok()) == Some(std::process::id());
    let count = std::env::var("LISTEN_FDS").ok().and_then(|n| n.parse::<i32>().ok()).unwrap_or(0);
    let names: Vec<String> = std::env::var("LISTEN_FDNAMES").map(|n| n.split(':').map(str::to_string).collect()).unwrap_or_default();
    for var in ["LISTEN_PID", "LISTEN_FDS", "LISTEN_FDNAMES"] {
        // SAFETY of remove_var: read once at startup, before threads that
        // read these exist.
        unsafe { std::env::remove_var(var) };
    }
    if !pid_matches || count <= 0 {
        return Vec::new();
    }
    const FIRST: RawFd = 3; // SD_LISTEN_FDS_START
    let mut out = Vec::new();
    for (i, fd) in (FIRST..FIRST + count.min(64)).enumerate() {
        // SAFETY: fds 3.. are ours by the activation protocol.
        unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) };
        let mut kind: libc::c_int = 0;
        let mut len = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
        // SAFETY: a valid out pointer and length.
        if unsafe { libc::getsockopt(fd, libc::SOL_SOCKET, libc::SO_TYPE, &mut kind as *mut _ as *mut libc::c_void, &mut len) } != 0 {
            continue;
        }
        let kind = match kind {
            libc::SOCK_STREAM => Kind::Tcp,
            libc::SOCK_DGRAM => Kind::Udp,
            _ => continue,
        };
        // The address without taking ownership: borrow through a socket we
        // forget again.
        let addr = match kind {
            Kind::Tcp => {
                // SAFETY: fd is a stream socket; ownership is given back by into_raw_fd.
                let socket = unsafe { std::net::TcpListener::from_raw_fd(fd) };
                let addr = socket.local_addr();
                std::mem::forget(socket);
                addr
            }
            Kind::Udp => {
                // SAFETY: as above, for a datagram socket.
                let socket = unsafe { std::net::UdpSocket::from_raw_fd(fd) };
                let addr = socket.local_addr();
                std::mem::forget(socket);
                addr
            }
        };
        let Ok(addr) = addr else { continue };
        out.push(Inherited { fd, kind, addr, name: names.get(i).cloned().filter(|n| !n.is_empty() && n != "unknown") });
    }
    out
}

/// The inherited socket of `kind` for `addr` (or named `name`): the same
/// port, the exact address preferred.
fn find(kind: Kind, addr: SocketAddr, name: Option<&str>) -> Option<RawFd> {
    let sockets = inherited().lock().unwrap_or_else(|e| e.into_inner());
    let candidates = sockets.iter().filter(|s| s.kind == kind);
    let by_name = name.and_then(|n| candidates.clone().find(|s| s.name.as_deref() == Some(n)));
    by_name
        .or_else(|| candidates.clone().find(|s| s.addr == addr))
        .or_else(|| candidates.clone().find(|s| s.addr.port() == addr.port()))
        .map(|s| s.fd)
}

/// A copy of the TCP listener systemd passed for `addr` (or named `name`),
/// if any. Each call is a new descriptor of the same socket, so several
/// runtimes can accept on it.
pub fn tcp(addr: SocketAddr, name: Option<&str>) -> Option<std::net::TcpListener> {
    let fd = find(Kind::Tcp, addr, name)?;
    // SAFETY: dup of a listening stream socket we hold.
    let copy = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
    (copy >= 0).then(|| unsafe { std::net::TcpListener::from_raw_fd(copy) })
}

/// A copy of the UDP socket systemd passed for `addr` (or named `name`).
pub fn udp(addr: SocketAddr, name: Option<&str>) -> Option<std::net::UdpSocket> {
    let fd = find(Kind::Udp, addr, name)?;
    // SAFETY: dup of a datagram socket we hold.
    let copy = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
    (copy >= 0).then(|| unsafe { std::net::UdpSocket::from_raw_fd(copy) })
}

/// Whether systemd passed any socket.
pub fn activated() -> bool {
    !inherited().lock().unwrap_or_else(|e| e.into_inner()).is_empty()
}

/// Send `message` to `$NOTIFY_SOCKET`; `false` without one (or on error).
pub fn notify(message: &str) -> bool {
    let Ok(target) = std::env::var("NOTIFY_SOCKET") else { return false };
    let Ok(socket) = std::os::unix::net::UnixDatagram::unbound() else { return false };
    let sent = match target.strip_prefix('@') {
        #[cfg(target_os = "linux")]
        Some(name) => {
            use std::os::linux::net::SocketAddrExt;
            std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes()).and_then(|addr| socket.send_to_addr(message.as_bytes(), &addr))
        }
        #[cfg(not(target_os = "linux"))]
        Some(_) => return false,
        None => socket.send_to(message.as_bytes(), &target),
    };
    sent.is_ok()
}

/// rws serves: `READY=1` (once), and the watchdog starts.
pub fn ready(status_line: &str) {
    if notify(&format!("READY=1\nSTATUS={status_line}")) {
        start_watchdog();
    }
}

/// A reload starts (`RELOADING=1` with the monotonic time systemd wants).
pub fn reloading() {
    let mut now = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: a valid out pointer.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) };
    let usec = now.tv_sec as u64 * 1_000_000 + now.tv_nsec as u64 / 1_000;
    notify(&format!("RELOADING=1\nMONOTONIC_USEC={usec}"));
}

/// A reload is done.
pub fn reloaded() {
    notify("READY=1");
}

/// Shutting down.
pub fn stopping() {
    notify("STOPPING=1");
}

/// A status line for `systemctl status`.
pub fn status(line: &str) {
    notify(&format!("STATUS={line}"));
}

/// `WATCHDOG=1` every half `WATCHDOG_USEC`, from the async runtime when
/// there is one (a wedged runtime then stops the pings and systemd restarts
/// rws), else from a thread.
fn start_watchdog() {
    static STARTED: OnceLock<()> = OnceLock::new();
    let Some(usec) = std::env::var("WATCHDOG_USEC").ok().and_then(|u| u.parse::<u64>().ok()).filter(|u| *u > 0) else { return };
    if std::env::var("WATCHDOG_PID").ok().and_then(|p| p.parse::<u32>().ok()).is_some_and(|p| p != std::process::id()) {
        return;
    }
    if STARTED.set(()).is_err() {
        return;
    }
    let every = std::time::Duration::from_micros(usec / 2);
    #[cfg(feature = "http2")]
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(async move {
            loop {
                notify("WATCHDOG=1");
                tokio::time::sleep(every).await;
            }
        });
        return;
    }
    let _ = std::thread::Builder::new().name("rws-watchdog".into()).spawn(move || loop {
        notify("WATCHDOG=1");
        std::thread::sleep(every);
    });
}

#[cfg(test)]
mod tests;
