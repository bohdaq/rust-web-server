use super::*;
use std::os::unix::net::UnixDatagram;

/// A stand-in for systemd's notification socket.
fn listener(name: &str) -> (UnixDatagram, crate::test_env::TempPath) {
    let path = crate::test_env::TempPath::new(name);
    let _ = std::fs::remove_file(&*path);
    let socket = UnixDatagram::bind(&*path).unwrap();
    socket.set_read_timeout(Some(std::time::Duration::from_secs(2))).unwrap();
    (socket, path)
}

fn next(socket: &UnixDatagram) -> String {
    let mut buf = [0u8; 512];
    let n = socket.recv(&mut buf).expect("a notification");
    String::from_utf8_lossy(&buf[..n]).to_string()
}

#[test]
fn notifications_reach_notify_socket() {
    let _g = crate::test_env::lock();
    std::env::remove_var("NOTIFY_SOCKET");
    assert!(!notify("READY=1"), "nothing without NOTIFY_SOCKET");
    let (socket, path) = listener("rws-notify.sock");
    std::env::set_var("NOTIFY_SOCKET", &*path);
    std::env::remove_var("WATCHDOG_USEC");
    ready("serving 2 listeners");
    assert_eq!("READY=1\nSTATUS=serving 2 listeners", next(&socket));
    reloading();
    let reload = next(&socket);
    assert!(reload.starts_with("RELOADING=1\nMONOTONIC_USEC=") && reload.rsplit('=').next().unwrap().parse::<u64>().unwrap() > 0, "{reload}");
    reloaded();
    assert_eq!("READY=1", next(&socket));
    status("renewing the certificate");
    assert_eq!("STATUS=renewing the certificate", next(&socket));
    stopping();
    assert_eq!("STOPPING=1", next(&socket));
    std::env::remove_var("NOTIFY_SOCKET");
}

#[cfg(target_os = "linux")]
#[test]
fn the_abstract_namespace() {
    use std::os::linux::net::SocketAddrExt;
    let _g = crate::test_env::lock();
    let name = format!("rws-test-{}", std::process::id());
    let addr = std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes()).unwrap();
    let socket = UnixDatagram::bind_addr(&addr).unwrap();
    socket.set_read_timeout(Some(std::time::Duration::from_secs(2))).unwrap();
    std::env::set_var("NOTIFY_SOCKET", format!("@{name}"));
    assert!(notify("STATUS=abstract"));
    assert_eq!("STATUS=abstract", next(&socket));
    std::env::remove_var("NOTIFY_SOCKET");
}

#[test]
fn inherited_sockets_match_by_name_address_and_port() {
    // Sockets the "systemd" of this test passed: built by hand, since fds
    // 3.. are this test process's own.
    let tcp_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let udp_socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let (tcp_addr, udp_addr) = (tcp_listener.local_addr().unwrap(), udp_socket.local_addr().unwrap());
    use std::os::fd::IntoRawFd;
    {
        let mut sockets = inherited().lock().unwrap();
        sockets.push(Inherited { fd: tcp_listener.into_raw_fd(), kind: Kind::Tcp, addr: tcp_addr, name: Some("https".into()) });
        sockets.push(Inherited { fd: udp_socket.into_raw_fd(), kind: Kind::Udp, addr: udp_addr, name: None });
    }
    assert!(activated());
    // By port: rws's 0.0.0.0 against systemd's address.
    let wildcard: SocketAddr = format!("0.0.0.0:{}", tcp_addr.port()).parse().unwrap();
    let first = tcp(wildcard, None).expect("matched by port");
    let second = tcp(wildcard, Some("https")).expect("matched by name");
    assert_eq!(tcp_addr, first.local_addr().unwrap());
    assert_eq!(tcp_addr, second.local_addr().unwrap(), "each call a new copy of the same socket");
    let client = std::net::TcpStream::connect(tcp_addr).unwrap();
    drop(client);
    assert!(first.accept().is_ok(), "the copy accepts");
    assert_eq!(udp_addr, udp(udp_addr, Some("quic")).unwrap().local_addr().unwrap(), "an absent name falls back to the address");
    assert!(udp(tcp_addr, None).is_none() || tcp_addr.port() == udp_addr.port(), "kinds don't mix");
    let other: SocketAddr = "127.0.0.1:1".parse().unwrap();
    assert!(tcp(other, None).is_none());
    inherited().lock().unwrap().clear();
}
