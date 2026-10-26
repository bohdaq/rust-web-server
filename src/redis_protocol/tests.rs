use super::{read_reply, RespReply};
use std::io::Cursor;

fn reply(bytes: &[u8]) -> std::io::Result<RespReply> {
    read_reply(&mut Cursor::new(bytes.to_vec()))
}

#[test]
fn replies_are_read() {
    assert!(matches!(reply(b"+OK\r\n").unwrap(), RespReply::Ok));
    assert!(matches!(reply(b":42\r\n").unwrap(), RespReply::Int(42)));
    assert!(matches!(reply(b"-ERR wrong type\r\n").unwrap(), RespReply::Error(m) if m == "ERR wrong type"));
    assert!(matches!(reply(b"$-1\r\n").unwrap(), RespReply::Bulk(None)));
    assert!(matches!(reply(b"$5\r\nhello\r\n").unwrap(), RespReply::Bulk(Some(v)) if v == b"hello"));
    assert!(matches!(reply(b"$0\r\n\r\n").unwrap(), RespReply::Bulk(Some(v)) if v.is_empty()));
}

#[test]
fn a_huge_claimed_bulk_length_is_refused_without_allocating_it() {
    // Used to allocate the claimed length up front (`vec![0; len + 2]`).
    assert!(reply(b"$9223372036854775806\r\n").is_err());
    assert!(reply(b"$536870913\r\n").is_err()); // 512 MiB + 1
}

#[test]
fn truncated_or_closed_replies_are_errors() {
    // A closed connection used to read as a successful "+OK".
    assert!(reply(b"").is_err());
    assert!(reply(b"+OK").is_err());
    assert!(reply(b"$10\r\nshort\r\n").is_err());
    // An endless line is cut off at 64 KiB.
    assert!(reply(&vec![b'+'; 100_000]).is_err());
}

#[test]
fn a_reply_line_with_multibyte_characters_is_not_a_panic() {
    let _ = reply("é\r\n".as_bytes());
    let _ = reply("$é\r\n".as_bytes());
    let _ = reply(":\u{1F600}\r\n".as_bytes());
}
