use super::*;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn key() -> Tsig {
    Tsig { key_name: "rws-key".to_string(), algorithm: "hmac-sha256".to_string(), secret: b"secret-key-1234".to_vec() }
}

/// Built independently (Python, from RFC 2136 §2 and RFC 8945 §4.3.3) and
/// pinned here: the UPDATE adding `_acme-challenge.example.test TXT "value"`
/// to zone example.test, and its HMAC-SHA256 over message + TSIG variables
/// with key `rws-key` / secret `secret-key-1234` at time 1700000000.
const UPDATE: &str = "123428000001000000010000076578616d706c65047465737400000600010f5f61636d652d6368616c6c656e6765076578616d706c65047465737400001000010000003c00060576616c7565";
const MAC: &str = "cbfcea69f5139815d8ac17f6996f5ee113a989dffce85f652e9c1ffe8e8de506";

#[test]
fn the_update_message_is_rfc_2136() {
    let rdata = crate::dns::txt_rdata(b"value");
    let message = update_message(0x1234, "example.test", "_acme-challenge.example.test", &[Change::Add(&rdata)]).unwrap();
    assert_eq!(UPDATE, hex(&message));
}

#[test]
fn the_tsig_record_signs_the_message_as_rfc_8945_says() {
    let rdata = crate::dns::txt_rdata(b"value");
    let mut message = update_message(0x1234, "example.test", "_acme-challenge.example.test", &[Change::Add(&rdata)]).unwrap();
    let unsigned = message.len();
    sign(&mut message, &key(), 1_700_000_000).unwrap();
    assert_eq!(1, u16::from_be_bytes([message[10], message[11]]), "ARCOUNT counts the TSIG record");
    assert!(hex(&message).contains(MAC), "the MAC");
    // The record: owner = key name, TYPE 250, CLASS ANY, TTL 0.
    let parsed = crate::dns::parse(&message).unwrap();
    let tsig = &parsed.additional[0];
    assert_eq!(("rws-key", crate::dns::TYPE_TSIG, crate::dns::CLASS_ANY, 0), (tsig.name.as_str(), tsig.rtype, tsig.class, tsig.ttl));
    assert_eq!(unsigned, message.len() - (tsig.rdata.end - unsigned), "the record is appended after the message");
    assert_eq!(Some(0), tsig_error(&message[tsig.rdata.clone()]));
}

#[test]
fn hmac_sha256_is_the_rfc_4231_function() {
    // RFC 4231 test case 2.
    let tag = aws_lc_rs::hmac::sign(&aws_lc_rs::hmac::Key::new(aws_lc_rs::hmac::HMAC_SHA256, b"Jefe"), b"what do ya want for nothing?");
    assert_eq!("5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843", hex(tag.as_ref()));
}

#[test]
fn deletes_use_class_any_and_none() {
    let rdata = crate::dns::txt_rdata(b"v");
    let message = update_message(1, "example.test", "_acme-challenge.example.test", &[Change::DeleteAll, Change::Delete(&rdata)]).unwrap();
    let parsed = crate::dns::parse(&message);
    // The update section parses as the "authority" (NSCOUNT position).
    let records = parsed.unwrap().authority;
    assert_eq!((crate::dns::CLASS_ANY, 0, 0..0), (records[0].class, records[0].ttl, records[0].rdata.start - records[0].rdata.start..records[0].rdata.len()));
    assert_eq!((crate::dns::CLASS_NONE, 0), (records[1].class, records[1].ttl));
    assert_eq!(b"\x01v", &message[records[1].rdata.clone()]);
}

fn reply(id: u16, rcode: u16, tsig_error_code: Option<u16>) -> Vec<u8> {
    let mut message = Vec::new();
    message.extend_from_slice(&id.to_be_bytes());
    message.extend_from_slice(&(0xA800 | rcode).to_be_bytes());
    message.extend_from_slice(&[0, 1, 0, 0, 0, 0]);
    message.extend_from_slice(&(tsig_error_code.is_some() as u16).to_be_bytes());
    crate::dns::encode_name("example.test", &mut message).unwrap();
    message.extend_from_slice(&[0, 6, 0, 1]);
    if let Some(error) = tsig_error_code {
        crate::dns::encode_name("rws-key", &mut message).unwrap();
        message.extend_from_slice(&[0, 250, 0, 255, 0, 0, 0, 0]);
        let mut rdata = Vec::new();
        crate::dns::encode_name("hmac-sha256", &mut rdata).unwrap();
        rdata.extend_from_slice(&[0, 0, 0, 0, 0, 0, 1, 44, 0, 0]); // time, fudge, MAC size 0
        rdata.extend_from_slice(&id.to_be_bytes());
        rdata.extend_from_slice(&error.to_be_bytes());
        rdata.extend_from_slice(&[0, 0]);
        message.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
        message.extend_from_slice(&rdata);
    }
    message
}

#[test]
fn update_replies_are_checked_and_explained() {
    assert!(check_update_reply(&reply(7, 0, None), 7).is_ok());
    assert!(check_update_reply(&reply(8, 0, None), 7).is_err());
    let refused = check_update_reply(&reply(7, 5, None), 7).unwrap_err();
    assert!(refused.contains("REFUSED"), "{refused}");
    let bad_sig = check_update_reply(&reply(7, 9, Some(16)), 7).unwrap_err();
    assert!(bad_sig.contains("NOTAUTH") && bad_sig.contains("BADSIG"), "{bad_sig}");
    assert!(check_update_reply(&reply(7, 9, Some(18)), 7).unwrap_err().contains("BADTIME"));
}

#[test]
fn record_name_and_value() {
    assert_eq!("_acme-challenge.example.test", record_name("example.test."));
    // RFC 8555 §8.4: base64url(SHA-256), no padding — 43 characters.
    let value = record_value("token.thumbprint");
    assert_eq!(43, value.len());
    assert!(!value.contains('=') && !value.contains('+') && !value.contains('/'));
}

#[test]
fn server_addresses_get_port_53() {
    assert_eq!("ns1.example.test:53", server_addr("ns1.example.test"));
    assert_eq!("127.0.0.1:8053", server_addr("127.0.0.1:8053"));
    assert_eq!("[::1]:53", server_addr("::1"));
    assert_eq!("[::1]:8053", server_addr("[::1]:8053"));
}

#[test]
fn persist_records() {
    assert_eq!("_validation-persist.example.com", persist_record_name("example.com."));
    let account = "https://ca.example/acct/7";
    let value = persist_record_value("ca.example", account, true);
    assert_eq!("ca.example; accounturi=https://ca.example/acct/7; policy=wildcard", value);
    let issuers = vec!["ca.example".to_string()];
    assert!(persist_record_matches(&value, &issuers, account, true, 0));
    assert!(persist_record_matches(&persist_record_value("ca.example", account, false), &issuers, account, false, 0));
    // A wildcard needs policy=wildcard; another CA or account doesn't count.
    assert!(!persist_record_matches(&persist_record_value("ca.example", account, false), &issuers, account, true, 0));
    assert!(!persist_record_matches(&value, &["other.example".to_string()], account, true, 0));
    assert!(!persist_record_matches(&value, &issuers, "https://ca.example/acct/8", true, 0));
    // Tags case-insensitive, spaces allowed, persistUntil honoured.
    assert!(persist_record_matches("ca.example;AccountURI=https://ca.example/acct/7 ; Policy=WILDCARD", &issuers, account, true, 0));
    let until = "ca.example; accounturi=https://ca.example/acct/7; persistUntil=100";
    assert!(persist_record_matches(until, &issuers, account, false, 100));
    assert!(!persist_record_matches(until, &issuers, account, false, 101));
    assert!(!persist_record_matches("ca.example; accounturi", &issuers, account, false, 0));
}
