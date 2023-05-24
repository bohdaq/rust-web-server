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
fn non_text_encode() {
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