use super::*;

#[test]
fn names_encode_lower_case_and_read_back() {
    let mut buf = Vec::new();
    encode_name("_ACME-Challenge.Example.TEST.", &mut buf).unwrap();
    assert_eq!(b"\x0f_acme-challenge\x07example\x04test\x00".to_vec(), buf);
    assert_eq!(("_acme-challenge.example.test".to_string(), buf.len()), read_name(&buf, 0).unwrap());
    let mut root = Vec::new();
    encode_name(".", &mut root).unwrap();
    assert_eq!(vec![0], root);
}

#[test]
fn names_over_the_limits_are_refused() {
    assert!(encode_name(&"a".repeat(64), &mut Vec::new()).is_err());
    let long = vec!["a".repeat(63); 5].join(".");
    assert!(encode_name(&long, &mut Vec::new()).is_err());
}

#[test]
fn a_compression_loop_is_an_error() {
    assert!(read_name(&[0xC0, 0x00], 0).is_err());
}

fn response_with(answers: &[(&str, u16, Vec<u8>)], authority: &[(&str, u16, Vec<u8>)]) -> Vec<u8> {
    let mut buf = vec![0x12, 0x34, 0x84, 0x03, 0, 1];
    buf.extend_from_slice(&(answers.len() as u16).to_be_bytes());
    buf.extend_from_slice(&(authority.len() as u16).to_be_bytes());
    buf.extend_from_slice(&[0, 0]);
    encode_name("x.example.test", &mut buf).unwrap();
    buf.extend_from_slice(&[0, 16, 0, 1]);
    for (name, rtype, rdata) in answers.iter().chain(authority) {
        encode_name(name, &mut buf).unwrap();
        buf.extend_from_slice(&rtype.to_be_bytes());
        buf.extend_from_slice(&[0, 1, 0, 0, 0, 60]);
        buf.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
        buf.extend_from_slice(rdata);
    }
    buf
}

#[test]
fn messages_parse_into_sections() {
    let buf = response_with(&[("x.example.test", TYPE_TXT, txt_rdata(b"value"))], &[("example.test", TYPE_SOA, vec![0; 22])]);
    let message = parse(&buf).unwrap();
    assert_eq!(0x1234, message.id);
    assert_eq!(3, message.rcode());
    assert_eq!(1, message.answers.len());
    assert_eq!(b"value".to_vec(), txt_value(&buf[message.answers[0].rdata.clone()]));
    assert_eq!("example.test", message.authority[0].name);
    assert_eq!((TYPE_SOA, 60), (message.authority[0].rtype, message.authority[0].ttl));
}

#[test]
fn truncated_messages_are_errors() {
    let buf = response_with(&[("x.example.test", TYPE_TXT, txt_rdata(b"value"))], &[]);
    for len in [0, 11, 20, buf.len() - 1] {
        assert!(parse(&buf[..len]).is_err(), "{len}");
    }
}

#[test]
fn long_txt_values_are_split_and_joined() {
    let value = vec![b'v'; 600];
    let rdata = txt_rdata(&value);
    assert_eq!(600 + 3, rdata.len());
    assert_eq!(value, txt_value(&rdata));
    assert_eq!(vec![0], txt_rdata(b""));
}

#[test]
fn queries_carry_the_question() {
    let query = build_query(7, "example.test", TYPE_SOA, false).unwrap();
    assert_eq!(&[0, 7, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0], &query[..12]);
    assert_eq!(&[0, 6, 0, 1], &query[query.len() - 4..]);
}
