use super::*;
use std::net::UdpSocket;
use std::thread;

// ── build_query ───────────────────────────────────────────────────────────────

#[test]
fn build_query_encodes_header_and_question() {
    let packet = build_query(0x1234, "_http._tcp.example.com");

    assert_eq!([0x12, 0x34], packet[0..2], "query ID");
    assert_eq!([0x01, 0x00], packet[2..4], "flags: recursion desired");
    assert_eq!([0x00, 0x01], packet[4..6], "QDCOUNT");
    assert_eq!([0x00, 0x00], packet[6..8], "ANCOUNT");
    assert_eq!([0x00, 0x00], packet[8..10], "NSCOUNT");
    assert_eq!([0x00, 0x00], packet[10..12], "ARCOUNT");

    // Question name: 5"_http" 4"_tcp" 7"example" 3"com" 0
    let mut expected_name = Vec::new();
    encode_name("_http._tcp.example.com", &mut expected_name);
    let name_end = 12 + expected_name.len();
    assert_eq!(expected_name, packet[12..name_end]);

    let qtype = u16::from_be_bytes([packet[name_end], packet[name_end + 1]]);
    let qclass = u16::from_be_bytes([packet[name_end + 2], packet[name_end + 3]]);
    assert_eq!(SRV_QTYPE, qtype);
    assert_eq!(IN_QCLASS, qclass);
    assert_eq!(packet.len(), name_end + 4);
}

#[test]
fn encode_name_strips_trailing_dot() {
    let mut a = Vec::new();
    encode_name("example.com", &mut a);
    let mut b = Vec::new();
    encode_name("example.com.", &mut b);
    assert_eq!(a, b);
}

// ── read_name ─────────────────────────────────────────────────────────────────

#[test]
fn read_name_without_compression() {
    let mut buf = Vec::new();
    encode_name("host1.example.com", &mut buf);
    buf.push(0xAA); // trailing junk to prove we stop at the right position
    let (name, next) = read_name(&buf, 0).unwrap();
    assert_eq!("host1.example.com", name);
    assert_eq!(buf.len() - 1, next);
}

#[test]
fn read_name_follows_compression_pointer() {
    // Buffer layout: [0..] "example.com" (uncompressed), then a name that's
    // just a pointer back to offset 0.
    let mut buf = Vec::new();
    encode_name("example.com", &mut buf);
    let pointer_pos = buf.len();
    buf.push(0xC0);
    buf.push(0x00); // pointer -> offset 0
    buf.push(0xBB); // trailing junk

    let (name, next) = read_name(&buf, pointer_pos).unwrap();
    assert_eq!("example.com", name);
    // Must resume right after the 2-byte pointer, not after the pointer's target.
    assert_eq!(pointer_pos + 2, next);
}

#[test]
fn read_name_rejects_pointer_loop() {
    // A pointer at offset 0 that points to itself.
    let buf = [0xC0u8, 0x00];
    assert!(read_name(&buf, 0).is_err());
}

#[test]
fn read_name_errors_on_truncated_label() {
    let buf = [5u8, b'a', b'b']; // claims a 5-byte label but only 2 bytes follow
    assert!(read_name(&buf, 0).is_err());
}

// ── parse_response ────────────────────────────────────────────────────────────

/// Builds a synthetic DNS response for one SRV query with the given answers.
fn build_srv_response(id: u16, query_name: &str, answers: &[(u16, u16, u16, &str)]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&id.to_be_bytes());
    buf.extend_from_slice(&0x8180u16.to_be_bytes()); // standard response, no error
    buf.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT
    buf.extend_from_slice(&(answers.len() as u16).to_be_bytes()); // ANCOUNT
    buf.extend_from_slice(&0u16.to_be_bytes());
    buf.extend_from_slice(&0u16.to_be_bytes());

    encode_name(query_name, &mut buf);
    buf.extend_from_slice(&SRV_QTYPE.to_be_bytes());
    buf.extend_from_slice(&IN_QCLASS.to_be_bytes());

    for (priority, weight, port, target) in answers {
        encode_name(query_name, &mut buf); // NAME (repeats the query name)
        buf.extend_from_slice(&SRV_QTYPE.to_be_bytes());
        buf.extend_from_slice(&IN_QCLASS.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes()); // TTL

        let mut rdata = Vec::new();
        rdata.extend_from_slice(&priority.to_be_bytes());
        rdata.extend_from_slice(&weight.to_be_bytes());
        rdata.extend_from_slice(&port.to_be_bytes());
        encode_name(target, &mut rdata);

        buf.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
        buf.extend_from_slice(&rdata);
    }

    buf
}

#[test]
fn parse_response_extracts_srv_records() {
    let response = build_srv_response(
        0xABCD,
        "_http._tcp.example.com",
        &[(10, 5, 8080, "host1.example.com"), (10, 15, 8081, "host2.example.com")],
    );

    let records = parse_response(&response, 0xABCD).unwrap();
    assert_eq!(2, records.len());
    assert_eq!(
        SrvRecord { priority: 10, weight: 5, port: 8080, target: "host1.example.com".to_string() },
        records[0]
    );
    assert_eq!(
        SrvRecord { priority: 10, weight: 15, port: 8081, target: "host2.example.com".to_string() },
        records[1]
    );
}

#[test]
fn parse_response_rejects_mismatched_id() {
    let response = build_srv_response(1, "example.com", &[(1, 1, 80, "a.example.com")]);
    assert!(parse_response(&response, 2).is_err());
}

#[test]
fn parse_response_nxdomain_returns_empty_not_error() {
    let mut buf = Vec::new();
    buf.extend_from_slice(&1u16.to_be_bytes());
    buf.extend_from_slice(&0x8183u16.to_be_bytes()); // rcode 3 = NXDOMAIN
    buf.extend_from_slice(&1u16.to_be_bytes());
    buf.extend_from_slice(&0u16.to_be_bytes());
    buf.extend_from_slice(&0u16.to_be_bytes());
    buf.extend_from_slice(&0u16.to_be_bytes());
    encode_name("example.com", &mut buf);
    buf.extend_from_slice(&SRV_QTYPE.to_be_bytes());
    buf.extend_from_slice(&IN_QCLASS.to_be_bytes());

    let records = parse_response(&buf, 1).unwrap();
    assert!(records.is_empty());
}

#[test]
fn parse_response_errors_on_short_buffer() {
    assert!(parse_response(&[0, 1, 2], 1).is_err());
}

// ── expand_by_weight ──────────────────────────────────────────────────────────

#[test]
fn expand_by_weight_keeps_only_lowest_priority_tier() {
    let records = vec![
        SrvRecord { priority: 10, weight: 1, port: 80, target: "low.example.com".to_string() },
        SrvRecord { priority: 20, weight: 1, port: 81, target: "high.example.com".to_string() },
    ];
    let backends = expand_by_weight(records);
    assert_eq!(vec!["low.example.com:80".to_string()], backends);
}

#[test]
fn expand_by_weight_repeats_proportionally_to_weight() {
    let records = vec![
        SrvRecord { priority: 0, weight: 1, port: 80, target: "a.example.com".to_string() },
        SrvRecord { priority: 0, weight: 3, port: 81, target: "b.example.com".to_string() },
    ];
    let backends = expand_by_weight(records);
    let a_count = backends.iter().filter(|b| *b == "a.example.com:80").count();
    let b_count = backends.iter().filter(|b| *b == "b.example.com:81").count();
    assert_eq!(1, a_count);
    assert_eq!(3, b_count);
}

#[test]
fn expand_by_weight_caps_extreme_weight() {
    let records = vec![SrvRecord { priority: 0, weight: 60000, port: 80, target: "a.example.com".to_string() }];
    let backends = expand_by_weight(records);
    assert_eq!(MAX_WEIGHT_COPIES as usize, backends.len());
}

#[test]
fn expand_by_weight_zero_weight_still_gets_one_copy() {
    let records = vec![SrvRecord { priority: 0, weight: 0, port: 80, target: "a.example.com".to_string() }];
    let backends = expand_by_weight(records);
    assert_eq!(vec!["a.example.com:80".to_string()], backends);
}

#[test]
fn expand_by_weight_empty_input_returns_empty() {
    assert!(expand_by_weight(Vec::new()).is_empty());
}

#[test]
fn expand_by_weight_strips_trailing_dot_from_target() {
    let records = vec![SrvRecord { priority: 0, weight: 1, port: 80, target: "a.example.com.".to_string() }];
    let backends = expand_by_weight(records);
    assert_eq!(vec!["a.example.com:80".to_string()], backends);
}

// ── query (mock UDP resolver) ─────────────────────────────────────────────────

#[test]
fn query_against_mock_resolver_returns_parsed_records() {
    let server = UdpSocket::bind("127.0.0.1:0").expect("bind mock DNS server");
    let server_addr = server.local_addr().unwrap();

    thread::spawn(move || {
        let mut buf = [0u8; 512];
        let (n, client_addr) = match server.recv_from(&mut buf) {
            Ok(v) => v,
            Err(_) => return,
        };
        let id = u16::from_be_bytes([buf[0], buf[1]]);
        let response = build_srv_response(id, "_http._tcp.example.com", &[(10, 5, 8080, "host1.example.com")]);
        let _ = server.send_to(&response, client_addr);
        let _ = n;
    });

    let records = query("_http._tcp.example.com", server_addr, Duration::from_secs(2)).unwrap();
    assert_eq!(1, records.len());
    assert_eq!(8080, records[0].port);
    assert_eq!("host1.example.com", records[0].target);
}

#[test]
fn query_times_out_when_nothing_responds() {
    // Bind a socket to reserve a port, then close it — nothing will answer.
    let addr = {
        let s = UdpSocket::bind("127.0.0.1:0").unwrap();
        s.local_addr().unwrap()
    };
    let result = query("example.com", addr, Duration::from_millis(200));
    assert!(result.is_err());
}
