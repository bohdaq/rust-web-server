//! Per-core runtimes (`RWS_CONFIG_RUNTIME=per-core`, Unix only): one
//! single-threaded tokio runtime per core, each accepting on its own
//! `SO_REUSEPORT` socket, the way nginx runs one worker per core. A
//! connection stays on the thread the kernel gave it to, so there's no work
//! stealing, no cross-thread wake-ups and no idle workers searching for work.
//!
//! The `rws` binary runs per-core unless configured otherwise (since 18.0.0,
//! [`default_to_per_core`]); for an application built on the library the
//! default stays `shared`, tokio's multi-threaded runtime. `run_tls` and
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

/// `RWS_CONFIG_RUNTIME`: `shared` or `per-core` (the `rws` binary's default).
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

/// Make `per-core` the runtime unless `RWS_CONFIG_RUNTIME` is already set —
/// what the `rws` binary does before reading its config file and flags, so
/// `runtime = "shared"` there (or in the environment) still wins. Library
/// applications don't call it: their handlers may keep tokio I/O in
/// process-wide structures, which per-core runtimes don't share.
///
/// A TLS key on a PKCS#11 token keeps the shared runtime (18.25.0): rustls
/// asks for the signature synchronously inside the handshake, and only on
/// the multi-threaded runtime can the token's wait be moved off the worker
/// (`tokio::task::block_in_place`); on a per-core runtime every connection
/// of that core would wait for the token.
pub fn default_to_per_core() {
    if std::env::var_os(RWS_CONFIG_RUNTIME).is_none() {
        if pkcs11_key_configured() {
            println!("  runtime: shared (a TLS key is on a PKCS#11 token; set runtime = \"per-core\" to override)");
            std::env::set_var(RWS_CONFIG_RUNTIME, "shared");
        } else {
            std::env::set_var(RWS_CONFIG_RUNTIME, "per-core");
        }
        crate::server_config::invalidate();
    }
}

/// Whether a `pkcs11:` key is configured, before the config file and flags
/// are read: in the environment, the arguments or the config file's text.
/// A false positive (a comment) only keeps the shared runtime.
fn pkcs11_key_configured() -> bool {
    let named = |v: &str| v.trim_start_matches(['"', '\'', ' ']).starts_with("pkcs11:");
    let in_env = std::env::vars().any(|(k, v)| k.starts_with("RWS_CONFIG_") && k.ends_with("KEY_FILE") && named(&v));
    let in_args = std::env::args().any(|a| a.split_once('=').is_some_and(|(_, v)| named(v)));
    let file = std::env::var("RWS_CONFIG_FILE").ok().filter(|p| !p.trim().is_empty()).unwrap_or_else(|| "rws.config.toml".to_string());
    let in_file = std::fs::read_to_string(file).is_ok_and(|text| mentions_pkcs11_key(&text));
    in_env || in_args || in_file
}

/// A `*key_file = "pkcs11:…"` line in config text.
pub(crate) fn mentions_pkcs11_key(text: &str) -> bool {
    text.lines().any(|line| {
        line.split_once('=').is_some_and(|(k, v)| k.trim().ends_with("key_file") && v.trim().trim_start_matches(['"', '\'']).starts_with("pkcs11:"))
    })
}

/// Startup warning for a PKCS#11 key on per-core runtimes, which the
/// signature wait stalls (`None` when that isn't the case).
pub fn pkcs11_warning() -> Option<&'static str> {
    let key_on_token = std::env::vars().any(|(k, v)| k.starts_with("RWS_CONFIG_") && k.ends_with("KEY_FILE") && v.trim().starts_with("pkcs11:"));
    (enabled() && key_on_token).then_some("[runtime] per-core with a PKCS#11 TLS key: each handshake's signature blocks that core's connections while the token works; runtime = \"shared\" moves the wait off the worker")
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
        // Pushed rather than `format!`ed: this runs for every proxied request.
        slot => {
            let slot = crate::ext::decimal::Decimal::new(slot as u128);
            let mut owned = String::with_capacity(key.len() + 1 + slot.as_str().len());
            owned.push_str(key);
            owned.push('#');
            owned.push_str(slot.as_str());
            Cow::Owned(owned)
        }
    }
}

/// A listening socket on `addr` that shares the port with the other
/// runtimes' (`SO_REUSEPORT`: the kernel spreads connections across them).
#[cfg(unix)]
pub(crate) fn bind(addr: SocketAddr) -> std::io::Result<tokio::net::TcpListener> {
    // Socket activation: every runtime accepts on a copy of systemd's socket
    // (a new SO_REUSEPORT socket couldn't share the port with it).
    if let Some(listener) = crate::systemd::tcp(addr, None) {
        listener.set_nonblocking(true)?;
        return tokio::net::TcpListener::from_std(listener);
    }
    let socket = if addr.is_ipv4() { tokio::net::TcpSocket::new_v4()? } else { tokio::net::TcpSocket::new_v6()? };
    socket.set_reuseaddr(true)?;
    socket.set_reuseport(true)?;
    socket.bind(addr)?;
    socket.listen(1024)
}

/// Start `count` threads, each running `accept_loop(listener)` on its own
/// single-threaded runtime until `stop` turns `true`. Returns once every
/// thread has bound its socket (or failed to), so the port accepts
/// connections again when it returns — the setup listener was closed just
/// before, and a client let in by it in between was reset. A thread whose
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
    if let Some(warning) = pkcs11_warning() {
        eprintln!("{warning}");
    }
    let mut handles = Vec::with_capacity(count);
    let (bound_tx, bound_rx) = std::sync::mpsc::channel::<()>();
    for slot in 1..=count {
        let bound = bound_tx.clone();
        let stop = stop.clone();
        let accept_loop = accept_loop.clone();
        let spawned = std::thread::Builder::new().name(format!("rws-core-{}", slot)).spawn(move || {
            SLOT.with(|s| s.set(slot));
            let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(r) => r,
                Err(e) => return eprintln!("per-core runtime {}: {}", slot, e),
            };
            runtime.block_on(async move {
                let listener = bind(addr);
                drop(bound);
                match listener {
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
    // Every sender dropped: each thread has bound (or given up). Bounded,
    // in case a thread never gets that far.
    drop(bound_tx);
    let _ = bound_rx.recv_timeout(std::time::Duration::from_secs(5));
    handles
}

/// Resolves once `stop` is `true` (or its sender is gone).
pub(crate) async fn stopped(stop: &mut watch::Receiver<bool>) {
    let _ = stop.wait_for(|s| *s).await;
}

#[cfg(test)]
mod tests {

    #[test]
    fn pkcs11_keys_are_spotted_in_config_text() {
        use super::mentions_pkcs11_key;
        assert!(mentions_pkcs11_key("tls_key_file = \"pkcs11:token=rws;object=k?module-path=/x.so\""));
        assert!(mentions_pkcs11_key("[[virtual_host]]\ndomain = \"a\"\nkey_file = 'pkcs11:object=k'"));
        assert!(!mentions_pkcs11_key("tls_key_file = \"/etc/rws/key.pem\""));
        assert!(!mentions_pkcs11_key("comment = \"pkcs11:object=k\""));
        assert!(!mentions_pkcs11_key(""));
    }

    use super::*;

    #[cfg(unix)]
    #[test]
    fn the_port_accepts_connections_when_spawn_returns() {
        let addr = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
        let (stop_tx, stop_rx) = watch::channel(false);
        let handles = spawn(addr, 2, stop_rx, |listener: tokio::net::TcpListener, mut stop: watch::Receiver<bool>| async move {
            tokio::select! {
                _ = async { while listener.accept().await.is_ok() {} } => {}
                _ = stopped(&mut stop) => {}
            }
        });
        assert_eq!(2, handles.len());
        std::net::TcpStream::connect(addr).expect("listening as soon as spawn returns");
        stop_tx.send(true).unwrap();
        for handle in handles {
            handle.join().unwrap();
        }
    }

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
    fn the_binary_defaults_to_per_core_unless_configured() {
        let _g = crate::test_env::lock();
        let before = std::env::var(RWS_CONFIG_RUNTIME).ok();
        std::env::remove_var(RWS_CONFIG_RUNTIME);
        default_to_per_core();
        assert_eq!(Some("per-core".to_string()), std::env::var(RWS_CONFIG_RUNTIME).ok());
        std::env::set_var(RWS_CONFIG_RUNTIME, "shared");
        default_to_per_core();
        assert_eq!(Some("shared".to_string()), std::env::var(RWS_CONFIG_RUNTIME).ok());
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
