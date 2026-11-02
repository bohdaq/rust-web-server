use super::*;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn sha256_matches_fips_180_4() {
    assert_eq!("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", hex(&sha256(b"")));
    assert_eq!("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad", hex(&sha256(b"abc")));
    assert_eq!(
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        hex(&sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"))
    );
    assert_eq!("cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0", hex(&sha256(&vec![b'a'; 1_000_000])));
}

#[test]
fn hmac_sha256_matches_rfc_4231() {
    // Test cases 1, 2 and 6 (a key longer than the block).
    assert_eq!("b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7", hex(&hmac_sha256(&[0x0b; 20], b"Hi There")));
    assert_eq!("5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843", hex(&hmac_sha256(b"Jefe", b"what do ya want for nothing?")));
    assert_eq!(
        "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54",
        hex(&hmac_sha256(&[0xaa; 131], b"Test Using Larger Than Block-Size Key - Hash Key First"))
    );
}

#[test]
fn verify_accepts_only_the_right_mac() {
    let mac = hmac_sha256(b"key", b"message");
    assert!(verify(b"key", b"message", &mac));
    assert!(!verify(b"key", b"messagf", &mac));
    assert!(!verify(b"kez", b"message", &mac));
    assert!(!verify(b"key", b"message", &mac[..31]));
}
