//! Inbound PROXY protocol, versions 1 (text) and 2 (binary), as HAProxy,
//! AWS NLB, GCP and most L4 load balancers send it: the client's address,
//! prepended to the connection. With `proxy_protocol = true`
//! (`RWS_CONFIG_PROXY_PROTOCOL`) every connection to the main listener must
//! start with one — before the TLS handshake on HTTPS — and its source
//! address becomes the client address rws logs, rate-limits and forwards in
//! `X-Forwarded-For`. A connection without a valid header is closed.
//!
//! The header is read exactly, byte for byte where needed, so what follows it
//! (a TLS ClientHello, an HTTP request) is untouched. `LOCAL` (v2) and
//! `UNKNOWN` (v1) headers — a load balancer's own health checks — keep the
//! peer address. TLVs (v2) are skipped.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

/// The 12-byte version 2 signature.
pub const V2_SIGNATURE: [u8; 12] = [0x0D, 0x0A, 0x0D, 0x0A, 0x00, 0x0D, 0x0A, 0x51, 0x55, 0x49, 0x54, 0x0A];

/// Longest version 1 header, `\r\n` included (the spec's limit).
pub const V1_MAX: usize = 107;

/// Largest version 2 header accepted: 16 bytes plus addresses and TLVs.
pub const V2_MAX: usize = 16 + 1024;

/// Whether the main listener expects PROXY protocol headers. Read once from
/// `RWS_CONFIG_PROXY_PROTOCOL` (default `false`); a change needs a restart.
pub fn enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("RWS_CONFIG_PROXY_PROTOCOL").is_ok_and(|v| v.trim().eq_ignore_ascii_case("true")))
}

/// What the bytes read so far say.
#[derive(Debug, PartialEq, Eq)]
pub enum Parsed {
    /// Read until the buffer holds this many bytes, then parse again.
    Need(usize),
    /// A complete header of `len` bytes. `source` is `None` for `LOCAL` /
    /// `UNKNOWN` / unsupported address families: keep the peer address.
    Header { source: Option<SocketAddr>, len: usize },
    /// Not a PROXY protocol header (or a broken one).
    Invalid(&'static str),
}

/// Parse the start of a connection. Never asks for more than [`V1_MAX`] or
/// [`V2_MAX`] bytes, and for version 1 only one byte at a time past the
/// first 12, so a reader following it never consumes what comes after the
/// header.
pub fn parse(buf: &[u8]) -> Parsed {
    if buf.len() < 12 {
        return Parsed::Need(12);
    }
    if buf[..12] == V2_SIGNATURE {
        return parse_v2(buf);
    }
    if buf.starts_with(b"PROXY ") {
        return parse_v1(buf);
    }
    Parsed::Invalid("no PROXY protocol header")
}

fn parse_v1(buf: &[u8]) -> Parsed {
    let Some(end) = buf.windows(2).position(|w| w == b"\r\n") else {
        return if buf.len() >= V1_MAX { Parsed::Invalid("PROXY v1 header too long") } else { Parsed::Need(buf.len() + 1) };
    };
    let len = end + 2;
    let Ok(line) = std::str::from_utf8(&buf[..end]) else { return Parsed::Invalid("PROXY v1 header isn't text") };
    let mut fields = line.split(' ');
    fields.next(); // "PROXY"
    match fields.next() {
        Some("UNKNOWN") => Parsed::Header { source: None, len },
        Some(family @ ("TCP4" | "TCP6")) => {
            let (Some(src), Some(_dst), Some(sport), Some(_dport), None) =
                (fields.next(), fields.next(), fields.next(), fields.next(), fields.next())
            else {
                return Parsed::Invalid("PROXY v1 header has the wrong number of fields");
            };
            let ip: Option<IpAddr> = match family {
                "TCP4" => src.parse::<Ipv4Addr>().ok().map(IpAddr::V4),
                _ => src.parse::<Ipv6Addr>().ok().map(IpAddr::V6),
            };
            match (ip, sport.parse::<u16>()) {
                (Some(ip), Ok(port)) => Parsed::Header { source: Some(SocketAddr::new(ip, port)), len },
                _ => Parsed::Invalid("PROXY v1 header has a bad address or port"),
            }
        }
        _ => Parsed::Invalid("PROXY v1 header has an unknown protocol"),
    }
}

fn parse_v2(buf: &[u8]) -> Parsed {
    if buf.len() < 16 {
        return Parsed::Need(16);
    }
    let version_command = buf[12];
    if version_command >> 4 != 2 {
        return Parsed::Invalid("PROXY v2 header has an unknown version");
    }
    let len = 16 + u16::from_be_bytes([buf[14], buf[15]]) as usize;
    if len > V2_MAX {
        return Parsed::Invalid("PROXY v2 header too long");
    }
    if buf.len() < len {
        return Parsed::Need(len);
    }
    let body = &buf[16..len];
    match version_command & 0x0F {
        0x0 => return Parsed::Header { source: None, len }, // LOCAL
        0x1 => {}                                            // PROXY
        _ => return Parsed::Invalid("PROXY v2 header has an unknown command"),
    }
    // Address family in the high nibble; only stream transports carry HTTP.
    let source = match buf[13] {
        0x11 if body.len() >= 12 => {
            let ip = Ipv4Addr::new(body[0], body[1], body[2], body[3]);
            Some(SocketAddr::new(IpAddr::V4(ip), u16::from_be_bytes([body[8], body[9]])))
        }
        0x21 if body.len() >= 36 => {
            let mut octets = [0u8; 16];
            octets.copy_from_slice(&body[..16]);
            Some(SocketAddr::new(IpAddr::V6(Ipv6Addr::from(octets)), u16::from_be_bytes([body[32], body[33]])))
        }
        0x11 | 0x21 => return Parsed::Invalid("PROXY v2 header is shorter than its addresses"),
        _ => None, // UNSPEC, UNIX sockets, datagrams: keep the peer address
    };
    Parsed::Header { source, len }
}

/// Read and parse the header at the start of `stream` (async builds).
/// `Ok(None)`: a valid header without an address; keep the peer's.
#[cfg(feature = "http2")]
pub async fn read_async<S: tokio::io::AsyncRead + Unpin>(stream: &mut S) -> std::io::Result<Option<SocketAddr>> {
    use tokio::io::AsyncReadExt;
    let mut buf = Vec::with_capacity(32);
    loop {
        match parse(&buf) {
            Parsed::Need(n) => {
                let start = buf.len();
                buf.resize(n, 0);
                stream.read_exact(&mut buf[start..]).await?;
            }
            Parsed::Header { source, .. } => return Ok(source),
            Parsed::Invalid(why) => return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, why)),
        }
    }
}

/// [`read_async`] for the blocking HTTP/1.1 server.
pub fn read_blocking<S: std::io::Read>(stream: &mut S) -> std::io::Result<Option<SocketAddr>> {
    let mut buf = Vec::with_capacity(32);
    loop {
        match parse(&buf) {
            Parsed::Need(n) => {
                let start = buf.len();
                buf.resize(n, 0);
                stream.read_exact(&mut buf[start..])?;
            }
            Parsed::Header { source, .. } => return Ok(source),
            Parsed::Invalid(why) => return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, why)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v2(command: u8, family: u8, addresses: &[u8]) -> Vec<u8> {
        let mut header = V2_SIGNATURE.to_vec();
        header.push(0x20 | command);
        header.push(family);
        header.extend_from_slice(&(addresses.len() as u16).to_be_bytes());
        header.extend_from_slice(addresses);
        header
    }

    #[test]
    fn version_1_tcp4_tcp6_and_unknown() {
        let header = b"PROXY TCP4 203.0.113.7 10.0.0.1 51234 443\r\n";
        let line = [header.as_slice(), b"GET / HTTP/1.1\r\n"].concat();
        assert_eq!(Parsed::Header { source: Some("203.0.113.7:51234".parse().unwrap()), len: header.len() }, parse(&line));
        let line = b"PROXY TCP6 2001:db8::7 2001:db8::1 51234 443\r\n";
        assert_eq!(Parsed::Header { source: Some("[2001:db8::7]:51234".parse().unwrap()), len: line.len() }, parse(line));
        assert_eq!(Parsed::Header { source: None, len: 15 }, parse(b"PROXY UNKNOWN\r\n\x16\x03\x01"));
    }

    #[test]
    fn version_1_is_read_a_byte_at_a_time_after_twelve() {
        assert_eq!(Parsed::Need(12), parse(b"PROXY"));
        assert_eq!(Parsed::Need(13), parse(b"PROXY TCP4 2"));
    }

    #[test]
    fn version_1_rejects_broken_headers() {
        for bad in [
            &b"PROXY TCP4 203.0.113.7 10.0.0.1 51234\r\n"[..],
            b"PROXY TCP4 2001:db8::1 10.0.0.1 1 2\r\n",
            b"PROXY TCP4 203.0.113.7 10.0.0.1 99999 443\r\n",
            b"PROXY UDP4 203.0.113.7 10.0.0.1 1 2\r\n",
            b"GET / HTTP/1.1\r\nHost: x\r\n\r\n",
        ] {
            assert!(matches!(parse(bad), Parsed::Invalid(_)), "{:?}", String::from_utf8_lossy(bad));
        }
        let long = [b"PROXY ".as_slice(), &[b'1'; V1_MAX]].concat();
        assert!(matches!(parse(&long), Parsed::Invalid(_)));
    }

    #[test]
    fn version_2_inet_inet6_local_and_tlvs() {
        let mut addresses = vec![203, 0, 113, 7, 10, 0, 0, 1];
        addresses.extend_from_slice(&51234u16.to_be_bytes());
        addresses.extend_from_slice(&443u16.to_be_bytes());
        addresses.extend_from_slice(&[0x04, 0x00, 0x01, 0x00]); // a TLV, skipped
        let header = v2(1, 0x11, &addresses);
        assert_eq!(Parsed::Header { source: Some("203.0.113.7:51234".parse().unwrap()), len: header.len() }, parse(&header));

        let mut addresses = "2001:db8::7".parse::<Ipv6Addr>().unwrap().octets().to_vec();
        addresses.extend_from_slice(&"2001:db8::1".parse::<Ipv6Addr>().unwrap().octets());
        addresses.extend_from_slice(&[0xC8, 0x22, 0x01, 0xBB]);
        let header = v2(1, 0x21, &addresses);
        assert_eq!(Parsed::Header { source: Some("[2001:db8::7]:51234".parse().unwrap()), len: 52 }, parse(&header));

        assert_eq!(Parsed::Header { source: None, len: 16 }, parse(&v2(0, 0x00, &[])));
        assert_eq!(Parsed::Need(16), parse(&V2_SIGNATURE));
        assert_eq!(Parsed::Need(28), parse(&v2(1, 0x11, &[0; 12])[..20]));
    }

    #[test]
    fn version_2_rejects_broken_headers() {
        assert!(matches!(parse(&v2(1, 0x11, &[0; 4])), Parsed::Invalid(_)));
        assert!(matches!(parse(&v2(7, 0x11, &[0; 12])), Parsed::Invalid(_)));
        let mut wrong_version = v2(1, 0x11, &[0; 12]);
        wrong_version[12] = 0x11;
        assert!(matches!(parse(&wrong_version), Parsed::Invalid(_)));
        let mut huge = V2_SIGNATURE.to_vec();
        huge.extend_from_slice(&[0x21, 0x11, 0xFF, 0xFF]);
        assert!(matches!(parse(&huge), Parsed::Invalid(_)));
    }

    #[test]
    fn the_readers_stop_at_the_end_of_the_header() {
        let mut input: &[u8] = b"PROXY TCP4 203.0.113.7 10.0.0.1 51234 443\r\nGET /";
        assert_eq!(Some("203.0.113.7:51234".parse().unwrap()), read_blocking(&mut input).unwrap());
        assert_eq!(b"GET /", input);
        let mut input: &[u8] = b"GET / HTTP/1.1\r\n\r\n";
        assert!(read_blocking(&mut input).is_err());
    }

    #[cfg(feature = "http2")]
    #[tokio::test]
    async fn the_async_reader_stops_at_the_end_of_the_header() {
        let mut header = v2(1, 0x11, &[192, 0, 2, 1, 10, 0, 0, 1, 0x30, 0x39, 0x01, 0xBB]);
        header.extend_from_slice(b"\x16\x03\x01");
        let mut input: &[u8] = &header;
        assert_eq!(Some("192.0.2.1:12345".parse().unwrap()), read_async(&mut input).await.unwrap());
        assert_eq!(b"\x16\x03\x01", input);
    }
}
