//! Per-core runtimes (`RWS_CONFIG_RUNTIME=per-core`, Unix only): one
//! single-threaded tokio runtime per core, each accepting on its own
//! `SO_REUSEPORT` socket, the way nginx runs one worker per core. A
//! connection stays on the thread the kernel gave it to, so there's no work
//! stealing, no cross-thread wake-ups and no idle workers searching for work.
//!
//! The default (`shared`) is tokio's multi-threaded runtime. `run_tls` and
//! `run_plain` start the per-core accept loops ([`spawn`]) and keep the
//! signals: SIGINT/SIGTERM stop every loop, SIGHUP reloads the config and
//! hands the rebuilt TLS acceptor to every loop. HTTP/3, the redirect
//! listener and ACME stay on the main runtime.
//!
//! Tokio I/O belongs to the runtime that created it, so pooled upstream
//! connections are kept per thread ([`pool_key`]).

use std::borrow::Cow;
use std::cell::Cell;
use std::net::SocketAddr;

use tokio::sync::watch;

/// `RWS_CONFIG_RUNTIME`: `shared` (default) or `per-core`.
pub const RWS_CONFIG_RUNTIME: &str = "RWS_CONFIG_RUNTIME";

thread_local! {
    /// 1-based index of this thread's per-core runtime; 0 elsewhere.
    static SLOT: Cell<usize> = const { Cell::new(0) };
}

/// Whether `RWS_CONFIG_RUNTIME` asks for per-core runtimes (always `false`
/// off Unix, where there is no `SO_REUSEPORT`).
pub fn enabled() -> bool {
    cfg!(unix)
        && std::env::var(RWS_CONFIG_RUNTIME).is_ok_and(|v| v.trim().eq_ignore_ascii_case("per-core"))
}

/// How many per-core runtimes to start: `TOKIO_WORKER_THREADS` when set
/// (the same knob as the shared runtime), otherwise one per available CPU.
pub fn runtime_count() -> usize {
    std::env::var("TOKIO_WORKER_THREADS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1))
}

/// The upstream pool key for `key` on this thread: unchanged on the shared
/// runtime, suffixed with the runtime's index on a per-core one (its
/// connections can only be driven by that runtime).
pub(crate) fn pool_key(key: &str) -> Cow<'_, str> {
    match SLOT.with(|s| s.get()) {
        0 => Cow::Borrowed(key),
        slot => Cow::Owned(format!("{}#{}", key, slot)),
    }
}

/// A listening socket on `addr` that shares the port with the other
/// runtimes' (`SO_REUSEPORT`: the kernel spreads connections across them).
#[cfg(unix)]
pub(crate) fn bind(addr: SocketAddr) -> std::io::Result<tokio::net::TcpListener> {
    let socket = if addr.is_ipv4() { tokio::net::TcpSocket::new_v4()? } else { tokio::net::TcpSocket::new_v6()? };
    socket.set_reuseaddr(true)?;
    socket.set_reuseport(true)?;
    socket.bind(addr)?;
    socket.listen(1024)
}

/// Start `count` threads, each running `accept_loop(listener)` on its own
/// single-threaded runtime until `stop` turns `true`. A thread whose
/// runtime or socket can't be set up reports it and ends; the others keep
/// serving.
#[cfg(unix)]
pub(crate) fn spawn<F, Fut>(
    addr: SocketAddr,
    count: usize,
    stop: watch::Receiver<bool>,
    accept_loop: F,
) -> Vec<std::thread::JoinHandle<()>>
where
    F: Fn(tokio::net::TcpListener, watch::Receiver<bool>) -> Fut + Send + Clone + 'static,
    Fut: std::future::Future<Output = ()>,
{
    let mut handles = Vec::with_capacity(count);
    for slot in 1..=count {
        let stop = stop.clone();
        let accept_loop = accept_loop.clone();
        let spawned = std::thread::Builder::new().name(format!("rws-core-{}", slot)).spawn(move || {
            SLOT.with(|s| s.set(slot));
            let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(r) => r,
                Err(e) => return eprintln!("per-core runtime {}: {}", slot, e),
            };
            runtime.block_on(async move {
                match bind(addr) {
                    Ok(listener) => accept_loop(listener, stop).await,
                    Err(e) => eprintln!("per-core runtime {}: unable to listen on {}: {}", slot, addr, e),
                }
            });
        });
        match spawned {
            Ok(h) => handles.push(h),
            Err(e) => eprintln!("per-core runtime {}: {}", slot, e),
        }
    }
    handles
}

/// Resolves once `stop` is `true` (or its sender is gone).
pub(crate) async fn stopped(stop: &mut watch::Receiver<bool>) {
    let _ = stop.wait_for(|s| *s).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_key_is_shared_off_the_per_core_runtimes() {
        assert!(matches!(pool_key("backend:80"), Cow::Borrowed("backend:80")));
    }

    #[test]
    fn pool_key_is_per_runtime_on_a_per_core_thread() {
        let keys = std::thread::spawn(|| {
            SLOT.with(|s| s.set(3));
            pool_key("backend:80").into_owned()
        })
        .join()
        .unwrap();
        assert_eq!("backend:80#3", keys);
    }

    #[test]
    fn enabled_only_for_per_core() {
        let _g = crate::test_env::lock();
        let before = std::env::var(RWS_CONFIG_RUNTIME).ok();
        std::env::remove_var(RWS_CONFIG_RUNTIME);
        assert!(!enabled());
        std::env::set_var(RWS_CONFIG_RUNTIME, "shared");
        assert!(!enabled());
        std::env::set_var(RWS_CONFIG_RUNTIME, " Per-Core ");
        assert_eq!(cfg!(unix), enabled());
        match before {
            Some(v) => std::env::set_var(RWS_CONFIG_RUNTIME, v),
            None => std::env::remove_var(RWS_CONFIG_RUNTIME),
        }
    }

    #[test]
    fn runtime_count_follows_tokio_worker_threads() {
        let _g = crate::test_env::lock();
        let before = std::env::var("TOKIO_WORKER_THREADS").ok();
        std::env::set_var("TOKIO_WORKER_THREADS", "3");
        assert_eq!(3, runtime_count());
        std::env::set_var("TOKIO_WORKER_THREADS", "zero");
        assert!(runtime_count() >= 1);
        match before {
            Some(v) => std::env::set_var("TOKIO_WORKER_THREADS", v),
            None => std::env::remove_var("TOKIO_WORKER_THREADS"),
        }
    }

    /// Two runtimes share one port; every connection is answered by one of
    /// them, on its own thread, and both stop when told to.
    #[cfg(unix)]
    #[test]
    fn runtimes_share_the_port_and_stop() {
        use std::io::{Read, Write};
        use tokio::io::AsyncWriteExt;

        // A free port: bind, read it, release.
        let addr = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
        let (stop_tx, stop_rx) = watch::channel(false);
        let accept_loop = |listener: tokio::net::TcpListener, mut stop: watch::Receiver<bool>| async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        if let Ok((mut stream, _)) = result {
                            let slot = SLOT.with(|s| s.get());
                            let _ = stream.write_all(slot.to_string().as_bytes()).await;
                        }
                    }
                    _ = stopped(&mut stop) => break,
                }
            }
        };
        let handles = spawn(addr, 2, stop_rx, accept_loop);
        assert_eq!(2, handles.len());

        let mut answered = 0;
        for _ in 0..50 {
            let Ok(mut c) = std::net::TcpStream::connect(addr) else {
                std::thread::sleep(std::time::Duration::from_millis(20)); // not listening yet
                continue;
            };
            let _ = c.write_all(b"x");
            let mut slot = String::new();
            let _ = c.read_to_string(&mut slot);
            if !slot.is_empty() {
                assert!(slot == "1" || slot == "2", "slot {slot}");
                answered += 1;
            }
        }
        assert!(answered > 0);

        stop_tx.send(true).unwrap();
        for h in handles {
            h.join().unwrap();
        }
        assert!(std::net::TcpStream::connect(addr).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn bind_reports_an_address_in_use_without_reuseport() {
        let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = taken.local_addr().unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async { assert!(bind(addr).is_err()) });
    }
}
