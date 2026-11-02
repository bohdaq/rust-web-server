use file_ext::FileExt;
use crate::core::base64::Base64;

#[test]
fn encode_one_char() {
    let data = "M".as_bytes();
    let encoded = Base64::encode(data).unwrap();
    assert_eq!("TQ==", encoded);

    let decoded = Base64::decode(encoded).unwrap();
    assert_eq!("M".as_bytes().to_vec(), decoded);
}


#[test]
fn encode_two_chars() {
    let data = "Ma".as_bytes();
    let encoded = Base64::encode(data).unwrap();
    assert_eq!("TWE=", encoded);

    let decoded = Base64::decode(encoded).unwrap();
    assert_eq!("Ma".as_bytes().to_vec(), decoded);
}

#[test]
fn encode_three_chars() {
    let data = "Man".as_bytes();
    let encoded = Base64::encode(data).unwrap();
    assert_eq!("TWFu", encoded);

    let decoded = Base64::decode(encoded).unwrap();
    assert_eq!("Man".as_bytes().to_vec(), decoded);
}

#[test]
fn basic_text_encode() {
    let data = "Many hands make light work.".as_bytes();
    let encoded = Base64::encode(data).unwrap();
    assert_eq!("TWFueSBoYW5kcyBtYWtlIGxpZ2h0IHdvcmsu", encoded);

    let decoded = Base64::decode(encoded).unwrap();
    assert_eq!(data, decoded);
}

#[test]
fn no_text_encode() {
    let data = "".as_bytes();
    let encoded = Base64::encode(data).unwrap();
    assert_eq!("", encoded);

    let decoded = Base64::decode(encoded).unwrap();
    assert_eq!(data, decoded);
}

//long running test
//#[test]
fn _non_text_encode() {
    let path = FileExt::build_path(&["static", "audio.m4a"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let file_as_bytes : Vec<u8> = FileExt::read_file(absolute_file_path.as_str()).unwrap();
    let data : &[u8] = file_as_bytes.as_ref();

    let encoded = Base64::encode(data).unwrap();


    let path = FileExt::build_path(&["static", "audio.m4a.base64"]);
    let pwd = FileExt::working_directory().unwrap();

    let absolute_file_path = FileExt::build_path(&[pwd.as_str(), path.as_str()]);
    let file_as_bytes : Vec<u8> = FileExt::read_file(absolute_file_path.as_str()).unwrap();
    let expected_data = String::from_utf8(file_as_bytes).unwrap();

    assert_eq!(expected_data, encoded);


    let decoded = Base64::decode(encoded).unwrap();
    assert_eq!(data, decoded);
}
// ── Decoder ─────────────────────────────────────────────────────────────────

use crate::core::base64::{Alphabet, Decoder};

#[test]
fn rfc4648_vectors_round_trip() {
    let vectors = [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("fooba", "Zm9vYmE="), ("foobar", "Zm9vYmFy")];
    for (plain, encoded) in vectors {
        assert_eq!(encoded, Base64::encode(plain.as_bytes()).unwrap());
        assert_eq!(plain.as_bytes(), Decoder::STANDARD.decode(encoded).unwrap());
        assert_eq!(plain.as_bytes(), Base64::decode(encoded.to_string()).unwrap());
        // Padding is optional.
        assert_eq!(plain.as_bytes(), Decoder::STANDARD.decode(encoded.trim_end_matches('=')).unwrap());
    }
}

#[test]
fn encode_matches_the_per_sequence_encoder() {
    let data: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
    for len in 0..40 {
        let slow: String = data[..len].chunks(3).map(|c| Base64::encode_sequence(c).unwrap()).collect();
        assert_eq!(slow, Base64::encode(&data[..len]).unwrap());
        assert_eq!(&data[..len], &Decoder::STANDARD.decode(&slow).unwrap()[..]);
    }
}

#[test]
fn alphabets() {
    // 0xfb 0xff = "+/8" standard, "-_8" URL-safe.
    assert_eq!(vec![0xfb, 0xff], Decoder::STANDARD.decode("+/8").unwrap());
    assert_eq!(vec![0xfb, 0xff], Decoder::URL_SAFE.decode("-_8").unwrap());
    assert_eq!(vec![0xfb, 0xff], Decoder::EITHER.decode("-/8").unwrap());
    assert!(Decoder::STANDARD.decode("-_8").is_err());
    assert!(Decoder::URL_SAFE.decode("+/8").is_err());
    assert_eq!(Decoder::new(Alphabet::UrlSafe), Decoder::URL_SAFE);
}

#[test]
fn whitespace_only_when_asked() {
    assert!(Decoder::STANDARD.decode("Zm9v\nYmFy").is_err());
    assert_eq!(b"foobar".to_vec(), Decoder::STANDARD.skip_whitespace().decode(" Zm9v\r\n\tYmFy \n").unwrap());
}

#[test]
fn padding_rules() {
    // Concatenated padded segments decode as a whole (gRPC-Web text frames).
    assert_eq!(b"ffo".to_vec(), Decoder::STANDARD.decode("Zg==Zm8=").unwrap());
    // Fewer pads than the quantum needs is fine, more is not.
    assert_eq!(b"f".to_vec(), Decoder::STANDARD.decode("Zg=").unwrap());
    assert!(Decoder::STANDARD.decode("Zg===").is_err());
    assert!(Decoder::STANDARD.decode("Zm8==").is_err());
    // Padding after a whole quantum or a single character.
    assert!(Decoder::STANDARD.decode("Zm9v=").is_err());
    assert!(Decoder::STANDARD.decode("Z===").is_err());
    assert!(Decoder::STANDARD.decode("=").is_err());
}

#[test]
fn errors_name_the_position() {
    let e = Decoder::STANDARD.decode("Zm9v*mFy").unwrap_err();
    assert_eq!(4, e.position);
    assert_eq!("not a base64 character at byte 4", e.to_string());
    let e = Decoder::STANDARD.decode("Zm9vY").unwrap_err();
    assert_eq!(5, e.position);
    assert!(Base64::decode("Zm9v!".to_string()).unwrap_err().contains("byte 4"));
    // Non-ASCII input is an error, never a panic.
    assert!(Decoder::STANDARD.decode("Zm9vé").is_err());
}

#[test]
fn skip_invalid_ignores_everything_else() {
    let d = Decoder::EITHER.skip_invalid();
    assert_eq!(b"foobar".to_vec(), d.decode("Zm9v=*YmFy==").unwrap());
    assert_eq!(b"foobar".to_vec(), crate::core::x509::base64_decode_std("Zm9v\nYmFy").unwrap());
}

#[test]
fn long_input_is_linear() {
    // `Base64::decode` used to take minutes on 200 KB (chars().nth per character).
    let data: Vec<u8> = (0..1_000_000u32).map(|i| (i * 31) as u8).collect();
    let encoded = Base64::encode(&data).unwrap();
    let started = std::time::Instant::now();
    assert_eq!(data, Base64::decode(encoded).unwrap());
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "{:?}", started.elapsed());
}

// ── what each former private decoder accepted, now through `Decoder` ────────

#[cfg(feature = "sso")]
#[test]
fn pkce_takes_either_alphabet() {
    // PKCE / JWT: URL-safe or standard, unpadded.
    assert_eq!(vec![0xfb, 0xff], crate::sso::pkce::base64url_decode("-_8").unwrap());
    assert_eq!(vec![0xfb, 0xff], crate::sso::pkce::base64url_decode("+/8=").unwrap());
    assert!(crate::sso::pkce::base64url_decode("a b").is_err());
}

#[test]
fn pem_bodies_decode() {
    // PEM: standard with line breaks.
    let der = crate::core::x509::base64_decode_std("MIIB\r\nAA==").unwrap();
    assert_eq!(vec![0x30, 0x82, 0x01, 0x00], der);
}
