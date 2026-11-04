use super::*;
use std::io::Read;

#[test]
fn targets_parse_with_a_default_port() {
    assert_eq!(Ok(Target::Udp("logs.example:514".into())), Target::parse("udp://logs.example"));
    assert_eq!(Ok(Target::Udp("10.0.0.1:5514".into())), Target::parse(" udp://10.0.0.1:5514 "));
    assert_eq!(Ok(Target::Tcp("[::1]:6514".into())), Target::parse("tcp://[::1]:6514"));
    assert_eq!(Ok(Target::Tcp("[::1]:514".into())), Target::parse("tcp://[::1]"));
    assert_eq!(Ok(Target::Unix("/dev/log".into())), Target::parse("unix:///dev/log"));
}

#[test]
fn bad_targets_are_refused() {
    for bad in ["", "logs.example:514", "http://logs.example", "udp://", "unix://dev/log"] {
        assert!(Target::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn facilities_by_name_or_number() {
    assert_eq!(Ok(16), parse_facility(""));
    assert_eq!(Ok(16), parse_facility("local0"));
    assert_eq!(Ok(23), parse_facility("LOCAL7"));
    assert_eq!(Ok(4), parse_facility("auth"));
    assert_eq!(Ok(10), parse_facility("10"));
    assert!(parse_facility("24").is_err());
    assert!(parse_facility("local8").is_err());
}

#[test]
fn messages_are_rfc_5424() {
    let mut out = Vec::new();
    message(&mut out, 16 * 8 + 6, "2026-11-04T12:00:00Z", "web-1", 42, br#"{"status":200}"#);
    assert_eq!(r#"<134>1 2026-11-04T12:00:00Z web-1 rws 42 access - {"status":200}"#, String::from_utf8(out).unwrap());
}

/// `<PRI>1 TIMESTAMP HOST rws PID access - ` before `line`.
fn assert_message(message: &str, pri: u8, line: &str) {
    let fields: Vec<&str> = message.splitn(8, ' ').collect();
    assert_eq!(8, fields.len(), "{message}");
    assert_eq!(format!("<{pri}>1"), fields[0], "{message}");
    assert!(fields[1].ends_with('Z') && fields[1].len() == 20, "{message}");
    assert!(!fields[2].is_empty(), "{message}");
    assert_eq!(["rws", &std::process::id().to_string(), "access", "-"], [fields[3], fields[4], fields[5], fields[6]], "{message}");
    assert_eq!(line, fields[7]);
}

#[test]
fn udp_sends_one_datagram_per_line() {
    let listener = UdpSocket::bind("127.0.0.1:0").unwrap();
    listener.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let sender = Sender::new(Target::Udp(listener.local_addr().unwrap().to_string()), 16);
    sender.enqueue(b"{\"a\":1}\n{\"b\":2}\n");
    sender.send_pending();
    let mut buf = [0u8; 2048];
    for line in ["{\"a\":1}", "{\"b\":2}"] {
        let n = listener.recv(&mut buf).unwrap();
        assert_message(std::str::from_utf8(&buf[..n]).unwrap(), 134, line);
    }
    // Nothing queued: nothing sent.
    sender.send_pending();
    listener.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
    assert!(listener.recv(&mut buf).is_err());
}

/// Split octet-counted frames (`LEN SP MSG`).
fn frames(mut bytes: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    while !bytes.is_empty() {
        let space = bytes.iter().position(|b| *b == b' ').expect("a length");
        let len: usize = std::str::from_utf8(&bytes[..space]).unwrap().parse().unwrap();
        out.push(String::from_utf8(bytes[space + 1..space + 1 + len].to_vec()).unwrap());
        bytes = &bytes[space + 1 + len..];
    }
    out
}

#[test]
fn tcp_frames_are_octet_counted_and_the_connection_is_remade() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let sender = Sender::new(Target::Tcp(listener.local_addr().unwrap().to_string()), 4);
    sender.enqueue("line one\nline two with a newline-free ü\n".as_bytes());
    sender.send_pending();
    let (mut first, _) = listener.accept().unwrap();
    first.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut got = Vec::new();
    while frames_complete(&got) < 2 {
        let mut buf = [0u8; 1024];
        let n = first.read(&mut buf).unwrap();
        assert!(n > 0, "closed early");
        got.extend_from_slice(&buf[..n]);
    }
    let got = frames(&got);
    assert_message(&got[0], 4 * 8 + 6, "line one");
    assert_message(&got[1], 38, "line two with a newline-free ü");
    // The server goes away; the next batch is sent on a new connection.
    drop(first);
    std::thread::sleep(Duration::from_millis(50));
    sender.enqueue(b"after\n");
    sender.send_pending(); // may still "succeed" into the dead socket
    std::thread::sleep(Duration::from_millis(100));
    sender.enqueue(b"after again\n");
    sender.send_pending();
    let (mut second, _) = listener.accept().unwrap();
    second.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut got = Vec::new();
    let mut buf = [0u8; 1024];
    while let Ok(n) = second.read(&mut buf) {
        if n == 0 {
            break;
        }
        got.extend_from_slice(&buf[..n]);
        if frames(&got).last().is_some_and(|m| m.ends_with("after again")) {
            break;
        }
    }
    assert!(frames(&got).last().is_some_and(|m| m.ends_with("after again")), "{:?}", String::from_utf8_lossy(&got));
}

/// Complete frames at the start of `bytes`.
fn frames_complete(mut bytes: &[u8]) -> usize {
    let mut n = 0;
    while let Some(space) = bytes.iter().position(|b| *b == b' ') {
        let Ok(len) = std::str::from_utf8(&bytes[..space]).unwrap_or("x").parse::<usize>() else { break };
        if bytes.len() < space + 1 + len {
            break;
        }
        bytes = &bytes[space + 1 + len..];
        n += 1;
    }
    n
}

#[cfg(unix)]
#[test]
fn unix_datagrams_reach_a_dev_log_style_socket() {
    let path = crate::test_env::TempPath::new("syslog.sock");
    let _ = std::fs::remove_file(&*path);
    let listener = std::os::unix::net::UnixDatagram::bind(&*path).unwrap();
    listener.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let sender = Sender::new(Target::Unix(path.to_path_buf()), 16);
    sender.enqueue(b"hello\n");
    sender.send_pending();
    let mut buf = [0u8; 1024];
    let n = listener.recv(&mut buf).unwrap();
    assert_message(std::str::from_utf8(&buf[..n]).unwrap(), 134, "hello");
}

#[test]
fn an_unreachable_server_costs_lines_not_the_caller() {
    // Nothing listens on port 9 of this address: the send fails, quietly.
    let sender = Sender::new(Target::Tcp("127.0.0.1:9".into()), 16);
    sender.enqueue(b"lost\n");
    sender.send_pending();
    // A full queue drops (and counts) instead of growing.
    let before = dropped_lines();
    let big = vec![b'x'; MAX_QUEUED];
    sender.enqueue(&big);
    sender.enqueue(b"one\ntwo\n");
    assert!(dropped_lines() >= before + 2);
}
