use super::*;

fn bundle(version: &str) -> Vec<u8> {
    (0..400).map(|i| format!("export function widget{i}(el) {{ el.dataset.v = '{version}'; return el.querySelector('.w{i}'); }}\n")).collect::<String>().into_bytes()
}

fn request(headers: &str) -> Request {
    Request::parse(format!("GET /app.js HTTP/1.1\r\nHost: x\r\n{headers}\r\n").as_bytes()).unwrap()
}

#[test]
fn both_codings_round_trip_and_beat_plain_compression() {
    let (v1, v2) = (bundle("1.0.0"), bundle("1.0.1"));
    for encoding in PREFERENCE.iter().copied().filter(|e| e.available()) {
        let delta = encode(encoding, &v2, &v1, Level::Static).unwrap();
        assert!(delta.starts_with(encoding.magic()), "{encoding:?}");
        assert_eq!(&delta[encoding.magic().len()..encoding.header_len()], &hash(&v1));
        assert_eq!(decode(encoding, &delta, &v1, MAX_SIZE).unwrap(), v2, "{encoding:?}");
        let plain = crate::compression::encode(crate::compression::Encoding::Gzip, &v2, Level::Static).unwrap();
        assert!(delta.len() * 4 < plain.len(), "{encoding:?}: delta {} vs gzip {}", delta.len(), plain.len());
    }
}

#[test]
fn decoding_checks_the_magic_hash_and_size() {
    let (v1, v2) = (bundle("a"), bundle("b"));
    for encoding in PREFERENCE.iter().copied().filter(|e| e.available()) {
        let delta = encode(encoding, &v2, &v1, Level::Dynamic).unwrap();
        assert!(decode(encoding, &delta, b"another dictionary", MAX_SIZE).is_none(), "wrong dictionary");
        assert!(decode(encoding, &delta, &v1, v2.len() - 1).is_none(), "over max_len");
        assert!(decode(encoding, &delta[..encoding.header_len() - 1], &v1, MAX_SIZE).is_none());
        let mut corrupt = delta.clone();
        corrupt[0] ^= 1;
        assert!(decode(encoding, &corrupt, &v1, MAX_SIZE).is_none(), "bad magic");
        let other = if encoding == DictionaryEncoding::Dcz { DictionaryEncoding::Dcb } else { DictionaryEncoding::Dcz };
        assert!(decode(other, &delta, &v1, MAX_SIZE).is_none(), "the other coding's magic");
    }
}

#[test]
fn inputs_over_the_limit_are_not_encoded() {
    let big = vec![b'x'; MAX_SIZE + 1];
    assert!(encode(DictionaryEncoding::Dcz, &big, b"d", Level::Dynamic).is_none());
    assert!(encode(DictionaryEncoding::Dcb, b"body", &big, Level::Dynamic).is_none());
}

#[test]
fn an_empty_dictionary_still_round_trips() {
    for encoding in PREFERENCE.iter().copied().filter(|e| e.available()) {
        let delta = encode(encoding, b"hello hello hello", b"", Level::Dynamic).unwrap();
        assert_eq!(decode(encoding, &delta, b"", 100).unwrap(), b"hello hello hello");
    }
}

#[test]
fn available_dictionary_values() {
    let digest = hash(b"dictionary");
    let encoded = crate::core::base64::Base64::encode(&digest).unwrap();
    assert_eq!(parse_available_dictionary(&format!(":{encoded}:")), Some(digest));
    assert_eq!(parse_available_dictionary(&format!("  :{encoded}:;p=1 ")), Some(digest), "parameters ignored");
    for bad in ["", ":", "::", &encoded, &format!(":{encoded}"), &format!(":{encoded}:x"), ":YWJj:", &format!(":{}:", &encoded[..43].replace('A', "-"))] {
        assert_eq!(parse_available_dictionary(bad), None, "{bad:?}");
    }
    assert_eq!(available_dictionary(&request(&format!("Available-Dictionary: :{encoded}:\r\n"))), Some(digest));
    assert_eq!(available_dictionary(&request("")), None);
}

#[test]
fn negotiation_needs_the_coding_by_name() {
    let pick = |accept: &str| negotiate(&request(&format!("Accept-Encoding: {accept}\r\n")));
    let both = DictionaryEncoding::Dcz.available() && DictionaryEncoding::Dcb.available();
    if both {
        assert_eq!(pick("gzip, br, zstd, dcb, dcz"), Some(DictionaryEncoding::Dcz));
        assert_eq!(pick("dcz;q=0.5, dcb"), Some(DictionaryEncoding::Dcb));
        assert_eq!(pick("dcb"), Some(DictionaryEncoding::Dcb));
    }
    assert_eq!(pick("*"), None, "a wildcard doesn't ask for dictionaries");
    assert_eq!(pick("gzip, br"), None);
    assert_eq!(pick("dcz;q=0, dcb;q=0"), None);
    assert_eq!(negotiate(&request("")), None);
}

#[test]
fn marking_sets_the_coding_and_vary() {
    let mut response = Response::text(crate::response::STATUS_CODE_REASON_PHRASE.n200_ok, "x");
    response.headers.push(Header { name: "Vary".into(), value: "Accept-Encoding".into() });
    mark_encoded(&mut response, DictionaryEncoding::Dcz);
    assert_eq!(response.header("Content-Encoding").unwrap().value, "dcz");
    assert_eq!(response.header("Vary").unwrap().value, "Accept-Encoding, Available-Dictionary");
}

#[test]
fn window_logs_cover_the_input() {
    assert_eq!(window_log(0), 10);
    assert_eq!(window_log(1024), 10);
    assert_eq!(window_log(1025), 11);
    assert_eq!(window_log(2 * MAX_SIZE + 16), 22);
}
